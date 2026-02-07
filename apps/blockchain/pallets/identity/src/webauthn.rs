//! WebAuthn Signature Verification Module
//!
//! WebAuthn仕様に準拠した署名検証を実装する。
//! - authenticatorData の解析
//! - clientDataJSON の解析
//! - ECDSA P-256 署名検証
//! - WYSIWYS (What You See Is What You Sign) チャレンジ検証

extern crate alloc;

use crate::cose::WebAuthnPublicKey;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;
use p256::ecdsa::{signature::Verifier, Signature, VerifyingKey};
use p256::EncodedPoint;
use sha2::{Digest, Sha256};

/// WebAuthn検証エラー
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WebAuthnError {
    /// authenticatorData が短すぎる (最低37バイト必要)
    AuthenticatorDataTooShort,
    /// rpIdHash が一致しない
    RpIdHashMismatch,
    /// userPresent フラグが立っていない
    UserNotPresent,
    /// clientDataJSON のパースに失敗
    InvalidClientDataJson,
    /// clientData の type が不正 ("webauthn.get" ではない)
    InvalidClientDataType,
    /// challenge が一致しない
    ChallengeMismatch,
    /// 署名フォーマットが不正
    InvalidSignatureFormat,
    /// 署名検証に失敗
    SignatureVerificationFailed,
    /// WYSIWYS チャレンジが不正
    InvalidWysiwysChallenge,
    /// 公開鍵が不正
    InvalidPublicKey,
    /// Base64デコードに失敗
    Base64DecodeFailed,
}

/// authenticatorData の flags フィールド
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthenticatorFlags {
    /// User Present (UP) - ビット0
    pub user_present: bool,
    /// User Verified (UV) - ビット2
    pub user_verified: bool,
    /// Attested credential data included (AT) - ビット6
    pub attested_credential_data: bool,
    /// Extension data included (ED) - ビット7
    pub extension_data: bool,
}

impl AuthenticatorFlags {
    /// フラグバイトからパース
    pub fn from_byte(flags: u8) -> Self {
        Self {
            user_present: (flags & 0x01) != 0,
            user_verified: (flags & 0x04) != 0,
            attested_credential_data: (flags & 0x40) != 0,
            extension_data: (flags & 0x80) != 0,
        }
    }
}

/// パース済み authenticatorData
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthenticatorData {
    /// rpIdHash (32 bytes) - SHA-256(rpId)
    pub rp_id_hash: [u8; 32],
    /// flags (1 byte)
    pub flags: AuthenticatorFlags,
    /// 署名カウンタ (4 bytes, big-endian)
    pub sign_count: u32,
    /// 生の authenticatorData (署名検証に使用)
    pub raw: Vec<u8>,
}

/// clientDataJSON の type フィールド
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClientDataType {
    /// "webauthn.create" - 登録時
    Create,
    /// "webauthn.get" - 認証時
    Get,
}

/// パース済み clientDataJSON
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientData {
    /// type: "webauthn.get" or "webauthn.create"
    pub type_: ClientDataType,
    /// challenge: base64url デコード済み
    pub challenge: Vec<u8>,
    /// origin: リクエスト元のオリジン
    pub origin: Vec<u8>,
}

/// WYSIWYS チャレンジのプレフィックス
pub const WYSIWYS_PREFIX: &[u8] = b"anarchy:post:";

/// 署名を正規化 (DER形式 → raw形式)
///
/// WebAuthn署名はDER形式で返されることが多いが、
/// p256クレートは64バイトのraw形式を期待する。
/// 両形式を自動検出して対応する。
pub fn normalize_signature(signature: &[u8]) -> Result<[u8; 64], WebAuthnError> {
    // 64バイトならそのままraw形式として使用
    if signature.len() == 64 {
        let mut raw = [0u8; 64];
        raw.copy_from_slice(signature);
        return Ok(raw);
    }

    // DER形式をパース
    // Format: 0x30 <total_len> 0x02 <r_len> <r> 0x02 <s_len> <s>
    if signature.len() < 8 || signature[0] != 0x30 {
        return Err(WebAuthnError::InvalidSignatureFormat);
    }

    let mut pos = 2; // skip 0x30 and length byte

    // R 値をパース
    if signature[pos] != 0x02 {
        return Err(WebAuthnError::InvalidSignatureFormat);
    }
    pos += 1;

    let r_len = signature[pos] as usize;
    pos += 1;

    if pos + r_len > signature.len() {
        return Err(WebAuthnError::InvalidSignatureFormat);
    }

    let r_bytes = &signature[pos..pos + r_len];
    pos += r_len;

    // S 値をパース
    if pos >= signature.len() || signature[pos] != 0x02 {
        return Err(WebAuthnError::InvalidSignatureFormat);
    }
    pos += 1;

    if pos >= signature.len() {
        return Err(WebAuthnError::InvalidSignatureFormat);
    }

    let s_len = signature[pos] as usize;
    pos += 1;

    if pos + s_len > signature.len() {
        return Err(WebAuthnError::InvalidSignatureFormat);
    }

    let s_bytes = &signature[pos..pos + s_len];

    // 32バイトに正規化 (先頭の0x00パディングを除去、または左側を0埋め)
    let mut raw = [0u8; 64];
    copy_integer_to_fixed(&mut raw[0..32], r_bytes)?;
    copy_integer_to_fixed(&mut raw[32..64], s_bytes)?;

    Ok(raw)
}

/// DER整数を固定長バイト配列にコピー
fn copy_integer_to_fixed(dest: &mut [u8], src: &[u8]) -> Result<(), WebAuthnError> {
    let dest_len = dest.len();

    // 先頭の0x00パディングをスキップ (DER整数の正の数表現)
    let src = if !src.is_empty() && src[0] == 0x00 && src.len() > 1 {
        &src[1..]
    } else {
        src
    };

    if src.len() > dest_len {
        return Err(WebAuthnError::InvalidSignatureFormat);
    }

    // 右寄せでコピー (左側は0で埋まっている)
    let offset = dest_len - src.len();
    dest[offset..].copy_from_slice(src);

    Ok(())
}

/// authenticatorData をパース
///
/// # 形式
/// - rpIdHash: 32 bytes
/// - flags: 1 byte
/// - signCount: 4 bytes (big-endian)
/// - (optional) attestedCredentialData
/// - (optional) extensions
pub fn parse_authenticator_data(data: &[u8]) -> Result<AuthenticatorData, WebAuthnError> {
    // 最低37バイト必要
    if data.len() < 37 {
        return Err(WebAuthnError::AuthenticatorDataTooShort);
    }

    let mut rp_id_hash = [0u8; 32];
    rp_id_hash.copy_from_slice(&data[0..32]);

    let flags = AuthenticatorFlags::from_byte(data[32]);

    let sign_count = u32::from_be_bytes([data[33], data[34], data[35], data[36]]);

    Ok(AuthenticatorData {
        rp_id_hash,
        flags,
        sign_count,
        raw: data.to_vec(),
    })
}

/// clientDataJSON をパース (簡易JSONパーサー)
///
/// 完全なJSONパーサーではなく、WebAuthn仕様で必要な
/// フィールドのみを抽出する最小実装。
pub fn parse_client_data_json(json: &[u8]) -> Result<ClientData, WebAuthnError> {
    // UTF-8文字列として解釈
    let json_str = core::str::from_utf8(json).map_err(|_| WebAuthnError::InvalidClientDataJson)?;

    // "type" フィールドを抽出
    let type_ = extract_json_string(json_str, "type")
        .ok_or(WebAuthnError::InvalidClientDataJson)?;

    let type_ = match type_.as_str() {
        "webauthn.get" => ClientDataType::Get,
        "webauthn.create" => ClientDataType::Create,
        _ => return Err(WebAuthnError::InvalidClientDataType),
    };

    // "challenge" フィールドを抽出 (base64url)
    let challenge_b64 = extract_json_string(json_str, "challenge")
        .ok_or(WebAuthnError::InvalidClientDataJson)?;

    let challenge = base64url_decode(&challenge_b64)?;

    // "origin" フィールドを抽出
    let origin = extract_json_string(json_str, "origin")
        .ok_or(WebAuthnError::InvalidClientDataJson)?;

    Ok(ClientData {
        type_,
        challenge,
        origin: origin.into_bytes(),
    })
}

/// JSON文字列から指定キーの文字列値を抽出 (簡易実装)
fn extract_json_string(json: &str, key: &str) -> Option<String> {
    // "key":" または "key": " のパターンを探す
    let pattern = alloc::format!("\"{}\"", key);
    let key_pos = json.find(&pattern)?;
    let after_key = &json[key_pos + pattern.len()..];

    // : を探す
    let colon_pos = after_key.find(':')?;
    let after_colon = &after_key[colon_pos + 1..];

    // 先頭の空白をスキップ
    let trimmed = after_colon.trim_start();

    // " で始まることを確認
    if !trimmed.starts_with('"') {
        return None;
    }

    // 閉じ " を探す (エスケープは考慮しない簡易実装)
    let value_start = 1;
    let remaining = &trimmed[value_start..];
    let value_end = remaining.find('"')?;

    Some(remaining[..value_end].into())
}

/// Base64URL デコード (パディングなし対応)
fn base64url_decode(input: &str) -> Result<Vec<u8>, WebAuthnError> {
    // パディングを追加
    let padding = (4 - input.len() % 4) % 4;
    let mut padded = input.to_string();
    for _ in 0..padding {
        padded.push('=');
    }

    // URL-safe文字を標準Base64に変換
    let standard: String = padded
        .chars()
        .map(|c| match c {
            '-' => '+',
            '_' => '/',
            other => other,
        })
        .collect();

    base64_decode(&standard)
}

/// 標準 Base64 デコード (最小実装)
fn base64_decode(input: &str) -> Result<Vec<u8>, WebAuthnError> {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    fn decode_char(c: u8) -> Result<u8, WebAuthnError> {
        if c == b'=' {
            return Ok(0);
        }
        ALPHABET
            .iter()
            .position(|&x| x == c)
            .map(|p| p as u8)
            .ok_or(WebAuthnError::Base64DecodeFailed)
    }

    let bytes = input.as_bytes();
    if bytes.len() % 4 != 0 {
        return Err(WebAuthnError::Base64DecodeFailed);
    }

    let mut result = Vec::new();

    for chunk in bytes.chunks(4) {
        let a = decode_char(chunk[0])?;
        let b = decode_char(chunk[1])?;
        let c = decode_char(chunk[2])?;
        let d = decode_char(chunk[3])?;

        result.push((a << 2) | (b >> 4));
        if chunk[2] != b'=' {
            result.push((b << 4) | (c >> 2));
        }
        if chunk[3] != b'=' {
            result.push((c << 6) | d);
        }
    }

    Ok(result)
}

/// WebAuthn署名を検証
///
/// # 署名対象メッセージ
/// `SHA256(authenticatorData || SHA256(clientDataJSON))`
///
/// # 引数
/// - `public_key`: 検証に使用するP-256公開鍵
/// - `authenticator_data`: 生のauthenticatorData
/// - `client_data_json`: 生のclientDataJSON (UTF-8)
/// - `signature`: DER形式またはraw形式の署名
pub fn verify_signature(
    public_key: &WebAuthnPublicKey,
    authenticator_data: &[u8],
    client_data_json: &[u8],
    signature: &[u8],
) -> Result<(), WebAuthnError> {
    // 1. 署名を正規化
    let sig_bytes = normalize_signature(signature)?;
    let sig = Signature::from_slice(&sig_bytes)
        .map_err(|_| WebAuthnError::InvalidSignatureFormat)?;

    // 2. 公開鍵を構築
    let encoded_point = EncodedPoint::from_affine_coordinates(
        &public_key.x.into(),
        &public_key.y.into(),
        false, // uncompressed
    );
    let verifying_key = VerifyingKey::from_encoded_point(&encoded_point)
        .map_err(|_| WebAuthnError::InvalidPublicKey)?;

    // 3. 署名対象メッセージを構築
    // message = SHA256(authenticatorData || SHA256(clientDataJSON))
    let client_data_hash = Sha256::digest(client_data_json);
    let mut message = Vec::with_capacity(authenticator_data.len() + 32);
    message.extend_from_slice(authenticator_data);
    message.extend_from_slice(&client_data_hash);
    let message_hash = Sha256::digest(&message);

    // 4. 署名を検証
    verifying_key
        .verify(&message_hash, &sig)
        .map_err(|_| WebAuthnError::SignatureVerificationFailed)
}

/// WYSIWYS チャレンジを検証
///
/// チャレンジ形式: PREFIX || content_hash || timestamp
///
/// # 引数
/// - `challenge`: clientDataJSONから抽出したchallenge
/// - `content_hash`: 投稿内容のSHA-256ハッシュ
pub fn verify_wysiwys_challenge(
    challenge: &[u8],
    content_hash: &[u8; 32],
) -> Result<(), WebAuthnError> {
    // 最小長チェック: prefix(13) + content_hash(32) + timestamp(8) = 53
    if challenge.len() < WYSIWYS_PREFIX.len() + 32 + 8 {
        return Err(WebAuthnError::InvalidWysiwysChallenge);
    }

    // プレフィックスチェック
    if !challenge.starts_with(WYSIWYS_PREFIX) {
        return Err(WebAuthnError::InvalidWysiwysChallenge);
    }

    // content_hash チェック
    let hash_start = WYSIWYS_PREFIX.len();
    let hash_end = hash_start + 32;
    if &challenge[hash_start..hash_end] != content_hash {
        return Err(WebAuthnError::ChallengeMismatch);
    }

    // タイムスタンプは存在チェックのみ (範囲検証は呼び出し側で行う)
    // timestamp は challenge[hash_end..hash_end+8] に格納される

    Ok(())
}

/// rpIdHash を検証
pub fn verify_rp_id_hash(
    authenticator_data: &AuthenticatorData,
    expected_rp_id: &[u8],
) -> Result<(), WebAuthnError> {
    let expected_hash = Sha256::digest(expected_rp_id);
    if authenticator_data.rp_id_hash != expected_hash.as_slice() {
        return Err(WebAuthnError::RpIdHashMismatch);
    }
    Ok(())
}

/// userPresent フラグを検証
pub fn verify_user_present(authenticator_data: &AuthenticatorData) -> Result<(), WebAuthnError> {
    if !authenticator_data.flags.user_present {
        return Err(WebAuthnError::UserNotPresent);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_signature_raw_format() {
        // 64バイトのraw形式
        let raw = [1u8; 64];
        let result = normalize_signature(&raw);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), raw);
    }

    #[test]
    fn test_normalize_signature_der_format() {
        // DER形式の署名 (簡略化したテストケース)
        // 0x30 <len> 0x02 <r_len> <r> 0x02 <s_len> <s>
        let mut der = Vec::new();
        der.push(0x30); // SEQUENCE
        der.push(0x44); // total length = 68

        der.push(0x02); // INTEGER (r)
        der.push(0x20); // r length = 32
        der.extend_from_slice(&[0xabu8; 32]); // r value

        der.push(0x02); // INTEGER (s)
        der.push(0x20); // s length = 32
        der.extend_from_slice(&[0xcdu8; 32]); // s value

        let result = normalize_signature(&der);
        assert!(result.is_ok());

        let raw = result.unwrap();
        assert_eq!(&raw[0..32], &[0xabu8; 32]);
        assert_eq!(&raw[32..64], &[0xcdu8; 32]);
    }

    #[test]
    fn test_normalize_signature_der_with_padding() {
        // DER形式で先頭に0x00パディングがある場合
        let mut der = Vec::new();
        der.push(0x30); // SEQUENCE
        der.push(0x45); // total length = 69

        der.push(0x02); // INTEGER (r)
        der.push(0x21); // r length = 33 (with padding)
        der.push(0x00); // padding
        der.extend_from_slice(&[0xffu8; 32]); // r value (high bit set)

        der.push(0x02); // INTEGER (s)
        der.push(0x20); // s length = 32
        der.extend_from_slice(&[0x01u8; 32]); // s value

        let result = normalize_signature(&der);
        assert!(result.is_ok());

        let raw = result.unwrap();
        assert_eq!(&raw[0..32], &[0xffu8; 32]); // padding removed
        assert_eq!(&raw[32..64], &[0x01u8; 32]);
    }

    #[test]
    fn test_normalize_signature_invalid() {
        // 不正な署名
        let invalid = [0x00, 0x01, 0x02];
        let result = normalize_signature(&invalid);
        assert_eq!(result, Err(WebAuthnError::InvalidSignatureFormat));
    }

    #[test]
    fn test_parse_authenticator_data_valid() {
        let mut data = Vec::new();
        data.extend_from_slice(&[0xaa; 32]); // rpIdHash
        data.push(0x05); // flags: UP=1, UV=1
        data.extend_from_slice(&[0x00, 0x00, 0x00, 0x2a]); // signCount = 42

        let result = parse_authenticator_data(&data);
        assert!(result.is_ok());

        let auth_data = result.unwrap();
        assert_eq!(auth_data.rp_id_hash, [0xaa; 32]);
        assert!(auth_data.flags.user_present);
        assert!(auth_data.flags.user_verified);
        assert!(!auth_data.flags.attested_credential_data);
        assert!(!auth_data.flags.extension_data);
        assert_eq!(auth_data.sign_count, 42);
    }

    #[test]
    fn test_parse_authenticator_data_too_short() {
        let data = [0u8; 36]; // 36 bytes, need at least 37
        let result = parse_authenticator_data(&data);
        assert_eq!(result, Err(WebAuthnError::AuthenticatorDataTooShort));
    }

    #[test]
    fn test_parse_client_data_json_valid() {
        let json = br#"{"type":"webauthn.get","challenge":"dGVzdA","origin":"https://example.com"}"#;

        let result = parse_client_data_json(json);
        assert!(result.is_ok());

        let client_data = result.unwrap();
        assert_eq!(client_data.type_, ClientDataType::Get);
        assert_eq!(client_data.challenge, b"test"); // base64url("test") = "dGVzdA"
        assert_eq!(client_data.origin, b"https://example.com");
    }

    #[test]
    fn test_parse_client_data_json_create_type() {
        let json = br#"{"type":"webauthn.create","challenge":"YWJj","origin":"https://example.com"}"#;

        let result = parse_client_data_json(json);
        assert!(result.is_ok());

        let client_data = result.unwrap();
        assert_eq!(client_data.type_, ClientDataType::Create);
    }

    #[test]
    fn test_parse_client_data_json_invalid_type() {
        let json = br#"{"type":"invalid","challenge":"dGVzdA","origin":"https://example.com"}"#;

        let result = parse_client_data_json(json);
        assert_eq!(result, Err(WebAuthnError::InvalidClientDataType));
    }

    #[test]
    fn test_base64url_decode() {
        // Standard test vectors
        assert_eq!(base64url_decode("").unwrap(), Vec::<u8>::new());
        assert_eq!(base64url_decode("Zg").unwrap(), b"f".to_vec());
        assert_eq!(base64url_decode("Zm8").unwrap(), b"fo".to_vec());
        assert_eq!(base64url_decode("Zm9v").unwrap(), b"foo".to_vec());
        assert_eq!(base64url_decode("Zm9vYg").unwrap(), b"foob".to_vec());
        assert_eq!(base64url_decode("Zm9vYmE").unwrap(), b"fooba".to_vec());
        assert_eq!(base64url_decode("Zm9vYmFy").unwrap(), b"foobar".to_vec());

        // URL-safe characters
        assert_eq!(base64url_decode("PDw_Pz4-").unwrap(), b"<<??>>".to_vec());
    }

    #[test]
    fn test_verify_wysiwys_challenge_valid() {
        let content_hash = [0x42u8; 32];
        let timestamp = [0x00u8; 8];

        let mut challenge = Vec::new();
        challenge.extend_from_slice(WYSIWYS_PREFIX);
        challenge.extend_from_slice(&content_hash);
        challenge.extend_from_slice(&timestamp);

        let result = verify_wysiwys_challenge(&challenge, &content_hash);
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_wysiwys_challenge_wrong_hash() {
        let content_hash = [0x42u8; 32];
        let wrong_hash = [0x00u8; 32];
        let timestamp = [0x00u8; 8];

        let mut challenge = Vec::new();
        challenge.extend_from_slice(WYSIWYS_PREFIX);
        challenge.extend_from_slice(&wrong_hash);
        challenge.extend_from_slice(&timestamp);

        let result = verify_wysiwys_challenge(&challenge, &content_hash);
        assert_eq!(result, Err(WebAuthnError::ChallengeMismatch));
    }

    #[test]
    fn test_verify_wysiwys_challenge_wrong_prefix() {
        let content_hash = [0x42u8; 32];
        let timestamp = [0x00u8; 8];

        let mut challenge = Vec::new();
        challenge.extend_from_slice(b"wrong:prefix:");
        challenge.extend_from_slice(&content_hash);
        challenge.extend_from_slice(&timestamp);

        let result = verify_wysiwys_challenge(&challenge, &content_hash);
        assert_eq!(result, Err(WebAuthnError::InvalidWysiwysChallenge));
    }

    #[test]
    fn test_verify_wysiwys_challenge_too_short() {
        let content_hash = [0x42u8; 32];
        let challenge = b"too_short";

        let result = verify_wysiwys_challenge(challenge, &content_hash);
        assert_eq!(result, Err(WebAuthnError::InvalidWysiwysChallenge));
    }

    #[test]
    fn test_verify_rp_id_hash_valid() {
        let rp_id = b"example.com";
        let expected_hash = Sha256::digest(rp_id);

        let mut rp_id_hash = [0u8; 32];
        rp_id_hash.copy_from_slice(&expected_hash);

        let auth_data = AuthenticatorData {
            rp_id_hash,
            flags: AuthenticatorFlags::from_byte(0x01),
            sign_count: 0,
            raw: Vec::new(),
        };

        let result = verify_rp_id_hash(&auth_data, rp_id);
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_rp_id_hash_mismatch() {
        let auth_data = AuthenticatorData {
            rp_id_hash: [0u8; 32],
            flags: AuthenticatorFlags::from_byte(0x01),
            sign_count: 0,
            raw: Vec::new(),
        };

        let result = verify_rp_id_hash(&auth_data, b"example.com");
        assert_eq!(result, Err(WebAuthnError::RpIdHashMismatch));
    }

    #[test]
    fn test_verify_user_present_true() {
        let auth_data = AuthenticatorData {
            rp_id_hash: [0u8; 32],
            flags: AuthenticatorFlags::from_byte(0x01), // UP = true
            sign_count: 0,
            raw: Vec::new(),
        };

        let result = verify_user_present(&auth_data);
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_user_present_false() {
        let auth_data = AuthenticatorData {
            rp_id_hash: [0u8; 32],
            flags: AuthenticatorFlags::from_byte(0x00), // UP = false
            sign_count: 0,
            raw: Vec::new(),
        };

        let result = verify_user_present(&auth_data);
        assert_eq!(result, Err(WebAuthnError::UserNotPresent));
    }

    #[test]
    fn test_authenticator_flags_all_bits() {
        // All relevant bits set: UP=1, UV=4, AT=64, ED=128
        let flags = AuthenticatorFlags::from_byte(0xc5); // 11000101

        assert!(flags.user_present);
        assert!(flags.user_verified);
        assert!(flags.attested_credential_data);
        assert!(flags.extension_data);
    }

    #[test]
    fn test_authenticator_flags_none() {
        let flags = AuthenticatorFlags::from_byte(0x00);

        assert!(!flags.user_present);
        assert!(!flags.user_verified);
        assert!(!flags.attested_credential_data);
        assert!(!flags.extension_data);
    }
}
