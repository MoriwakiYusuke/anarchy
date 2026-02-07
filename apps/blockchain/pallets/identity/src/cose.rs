//! COSE (CBOR Object Signing and Encryption) Public Key Parser
//!
//! WebAuthn仕様で使用されるCOSE形式の公開鍵をパースする。
//! ES256 (ECDSA P-256 with SHA-256) のみをサポート。

use sp_std::vec::Vec;

/// COSE公開鍵から抽出された P-256 公開鍵座標
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WebAuthnPublicKey {
    /// X座標（32バイト）
    pub x: [u8; 32],
    /// Y座標（32バイト）
    pub y: [u8; 32],
}

impl WebAuthnPublicKey {
    /// 非圧縮形式のバイト列に変換 (65バイト: 0x04 || x || y)
    pub fn to_uncompressed(&self) -> [u8; 65] {
        let mut result = [0u8; 65];
        result[0] = 0x04;
        result[1..33].copy_from_slice(&self.x);
        result[33..65].copy_from_slice(&self.y);
        result
    }

    /// 非圧縮形式のバイト列からパース (65バイト: 0x04 || x || y)
    pub fn from_uncompressed(bytes: &[u8]) -> Result<Self, CoseError> {
        if bytes.len() != 65 {
            return Err(CoseError::InvalidCoordinateLength);
        }
        if bytes[0] != 0x04 {
            return Err(CoseError::InvalidUncompressedPrefix);
        }
        let mut x = [0u8; 32];
        let mut y = [0u8; 32];
        x.copy_from_slice(&bytes[1..33]);
        y.copy_from_slice(&bytes[33..65]);
        Ok(Self { x, y })
    }
}

/// COSEパースエラー
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoseError {
    /// COSE公開鍵のパースに失敗
    InvalidCoseFormat,
    /// データが短すぎる
    DataTooShort,
    /// サポートされていないキータイプ (EC2以外)
    UnsupportedKeyType,
    /// サポートされていないアルゴリズム (ES256以外)
    UnsupportedAlgorithm,
    /// サポートされていない曲線 (P-256以外)
    UnsupportedCurve,
    /// X座標が不正またはない
    InvalidXCoordinate,
    /// Y座標が不正またはない
    InvalidYCoordinate,
    /// 座標の長さが不正 (32バイトでない)
    InvalidCoordinateLength,
    /// 非圧縮形式のプレフィックスが不正
    InvalidUncompressedPrefix,
    /// 公開鍵が曲線上にない
    PointNotOnCurve,
    /// CBORマップが期待されたが見つからなかった
    ExpectedMap,
    /// CBOR整数のデコードに失敗
    InvalidInteger,
    /// CBORバイト列のデコードに失敗
    InvalidBytes,
}

// CBOR Major Types
const CBOR_UINT: u8 = 0;     // Major type 0: unsigned integer
const CBOR_NEGINT: u8 = 1;   // Major type 1: negative integer
const CBOR_BYTES: u8 = 2;    // Major type 2: byte string
const CBOR_MAP: u8 = 5;      // Major type 5: map

// COSE Key Parameter Labels
const COSE_KTY: i64 = 1;     // Key Type
const COSE_ALG: i64 = 3;     // Algorithm
const COSE_CRV: i64 = -1;    // Curve (EC2)
const COSE_X: i64 = -2;      // X Coordinate (EC2)
const COSE_Y: i64 = -3;      // Y Coordinate (EC2)

// COSE Key Type Values
const KTY_EC2: i64 = 2;      // Elliptic Curve with x, y

// COSE Algorithm Values
const ALG_ES256: i64 = -7;   // ECDSA with SHA-256

// COSE Curve Values
const CRV_P256: i64 = 1;     // P-256 (secp256r1)

/// CBORヘッダをデコードして (major_type, argument) を返す
fn decode_cbor_head(data: &[u8], cursor: &mut usize) -> Result<(u8, u64), CoseError> {
    if *cursor >= data.len() {
        return Err(CoseError::DataTooShort);
    }
    let initial_byte = data[*cursor];
    *cursor += 1;

    let major_type = initial_byte >> 5;
    let additional = initial_byte & 0x1f;

    let argument = match additional {
        0..=23 => additional as u64,
        24 => {
            if *cursor >= data.len() {
                return Err(CoseError::DataTooShort);
            }
            let val = data[*cursor] as u64;
            *cursor += 1;
            val
        }
        25 => {
            if *cursor + 2 > data.len() {
                return Err(CoseError::DataTooShort);
            }
            let val = u16::from_be_bytes([data[*cursor], data[*cursor + 1]]) as u64;
            *cursor += 2;
            val
        }
        26 => {
            if *cursor + 4 > data.len() {
                return Err(CoseError::DataTooShort);
            }
            let val = u32::from_be_bytes([
                data[*cursor],
                data[*cursor + 1],
                data[*cursor + 2],
                data[*cursor + 3],
            ]) as u64;
            *cursor += 4;
            val
        }
        27 => {
            if *cursor + 8 > data.len() {
                return Err(CoseError::DataTooShort);
            }
            let val = u64::from_be_bytes([
                data[*cursor],
                data[*cursor + 1],
                data[*cursor + 2],
                data[*cursor + 3],
                data[*cursor + 4],
                data[*cursor + 5],
                data[*cursor + 6],
                data[*cursor + 7],
            ]);
            *cursor += 8;
            val
        }
        _ => return Err(CoseError::InvalidCoseFormat),
    };

    Ok((major_type, argument))
}

/// CBOR整数 (正または負) をデコード
fn decode_cbor_int(data: &[u8], cursor: &mut usize) -> Result<i64, CoseError> {
    let (major, arg) = decode_cbor_head(data, cursor)?;
    match major {
        CBOR_UINT => Ok(arg as i64),
        CBOR_NEGINT => Ok(-1 - arg as i64),
        _ => Err(CoseError::InvalidInteger),
    }
}

/// CBORバイト列をデコード
fn decode_cbor_bytes(data: &[u8], cursor: &mut usize) -> Result<Vec<u8>, CoseError> {
    let (major, len) = decode_cbor_head(data, cursor)?;
    if major != CBOR_BYTES {
        return Err(CoseError::InvalidBytes);
    }
    let len = len as usize;
    if *cursor + len > data.len() {
        return Err(CoseError::DataTooShort);
    }
    let bytes = data[*cursor..*cursor + len].to_vec();
    *cursor += len;
    Ok(bytes)
}

/// CBOR値をスキップ (未知のラベル用)
fn skip_cbor_value(data: &[u8], cursor: &mut usize) -> Result<(), CoseError> {
    let (major, arg) = decode_cbor_head(data, cursor)?;
    match major {
        CBOR_UINT | CBOR_NEGINT => Ok(()),
        CBOR_BYTES | 3 => {
            // byte string or text string
            let len = arg as usize;
            if *cursor + len > data.len() {
                return Err(CoseError::DataTooShort);
            }
            *cursor += len;
            Ok(())
        }
        4 => {
            // array
            for _ in 0..arg {
                skip_cbor_value(data, cursor)?;
            }
            Ok(())
        }
        CBOR_MAP => {
            // map
            for _ in 0..arg {
                skip_cbor_value(data, cursor)?; // key
                skip_cbor_value(data, cursor)?; // value
            }
            Ok(())
        }
        7 if arg <= 23 => Ok(()), // simple values
        _ => Err(CoseError::InvalidCoseFormat),
    }
}

/// COSE公開鍵をパースして P-256 公開鍵を抽出
///
/// # 入力形式
/// CBOR MAP:
/// ```cbor
/// {
///   1: 2,      // kty: EC2
///   3: -7,     // alg: ES256
///   -1: 1,     // crv: P-256
///   -2: h'...', // x: 32 bytes
///   -3: h'...', // y: 32 bytes
/// }
/// ```
pub fn parse_cose_key(cose_bytes: &[u8]) -> Result<WebAuthnPublicKey, CoseError> {
    let mut cursor = 0;

    // マップヘッダを読み取り
    let (major, map_len) = decode_cbor_head(cose_bytes, &mut cursor)?;
    if major != CBOR_MAP {
        return Err(CoseError::ExpectedMap);
    }

    let mut kty: Option<i64> = None;
    let mut alg: Option<i64> = None;
    let mut crv: Option<i64> = None;
    let mut x: Option<Vec<u8>> = None;
    let mut y: Option<Vec<u8>> = None;

    // マップエントリをパース
    for _ in 0..map_len {
        let label = decode_cbor_int(cose_bytes, &mut cursor)?;

        match label {
            COSE_KTY => kty = Some(decode_cbor_int(cose_bytes, &mut cursor)?),
            COSE_ALG => alg = Some(decode_cbor_int(cose_bytes, &mut cursor)?),
            COSE_CRV => crv = Some(decode_cbor_int(cose_bytes, &mut cursor)?),
            COSE_X => x = Some(decode_cbor_bytes(cose_bytes, &mut cursor)?),
            COSE_Y => y = Some(decode_cbor_bytes(cose_bytes, &mut cursor)?),
            _ => skip_cbor_value(cose_bytes, &mut cursor)?,
        }
    }

    // キータイプ検証: EC2 (2)
    match kty {
        Some(KTY_EC2) => {}
        Some(_) => return Err(CoseError::UnsupportedKeyType),
        None => return Err(CoseError::InvalidCoseFormat),
    }

    // アルゴリズム検証: ES256 (-7)
    match alg {
        Some(ALG_ES256) => {}
        Some(_) => return Err(CoseError::UnsupportedAlgorithm),
        None => return Err(CoseError::InvalidCoseFormat),
    }

    // 曲線検証: P-256 (1)
    match crv {
        Some(CRV_P256) => {}
        Some(_) => return Err(CoseError::UnsupportedCurve),
        None => return Err(CoseError::InvalidCoseFormat),
    }

    // X座標抽出・検証
    let x_bytes = x.ok_or(CoseError::InvalidXCoordinate)?;
    if x_bytes.len() != 32 {
        return Err(CoseError::InvalidCoordinateLength);
    }

    // Y座標抽出・検証
    let y_bytes = y.ok_or(CoseError::InvalidYCoordinate)?;
    if y_bytes.len() != 32 {
        return Err(CoseError::InvalidCoordinateLength);
    }

    let mut x_arr = [0u8; 32];
    let mut y_arr = [0u8; 32];
    x_arr.copy_from_slice(&x_bytes);
    y_arr.copy_from_slice(&y_bytes);

    let public_key = WebAuthnPublicKey { x: x_arr, y: y_arr };

    // 曲線上の点かを検証
    validate_public_key(&public_key)?;

    Ok(public_key)
}

/// 公開鍵が P-256 曲線上の有効な点かを検証
///
/// p256クレートの VerifyingKey を使用して検証
pub fn validate_public_key(public_key: &WebAuthnPublicKey) -> Result<(), CoseError> {
    use p256::ecdsa::VerifyingKey;
    use p256::EncodedPoint;

    // 非圧縮形式で EncodedPoint を構築
    let encoded = EncodedPoint::from_affine_coordinates(
        &public_key.x.into(),
        &public_key.y.into(),
        false, // uncompressed
    );

    // VerifyingKey の構築を試みることで曲線上の点かを検証
    VerifyingKey::from_encoded_point(&encoded).map_err(|_| CoseError::PointNotOnCurve)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // テスト用 COSE 公開鍵 (ES256, P-256)
    // kty: 2 (EC2), alg: -7 (ES256), crv: 1 (P-256)
    // このテストベクトルは WebAuthn 仕様に準拠
    fn valid_cose_key() -> Vec<u8> {
        // CBOR: {1: 2, 3: -7, -1: 1, -2: h'...x...', -3: h'...y...'}
        // 有効な P-256 テストポイント (NIST P-256 curve point)
        let x: [u8; 32] = [
            0x6b, 0x17, 0xd1, 0xf2, 0xe1, 0x2c, 0x42, 0x47, 0xf8, 0xbc, 0xe6, 0xe5, 0x63, 0xa4,
            0x40, 0xf2, 0x77, 0x03, 0x7d, 0x81, 0x2d, 0xeb, 0x33, 0xa0, 0xf4, 0xa1, 0x39, 0x45,
            0xd8, 0x98, 0xc2, 0x96,
        ];
        let y: [u8; 32] = [
            0x4f, 0xe3, 0x42, 0xe2, 0xfe, 0x1a, 0x7f, 0x9b, 0x8e, 0xe7, 0xeb, 0x4a, 0x7c, 0x0f,
            0x9e, 0x16, 0x2b, 0xce, 0x33, 0x57, 0x6b, 0x31, 0x5e, 0xce, 0xcb, 0xb6, 0x40, 0x68,
            0x37, 0xbf, 0x51, 0xf5,
        ];

        // CBOR encoding: A5 01 02 03 26 20 01 21 58 20 [x] 22 58 20 [y]
        let mut cose = Vec::new();
        cose.push(0xa5); // map(5)
        cose.push(0x01); // kty label (1)
        cose.push(0x02); // EC2 value (2)
        cose.push(0x03); // alg label (3)
        cose.push(0x26); // ES256 value (-7 = 0x20 | 6)
        cose.push(0x20); // crv label (-1 = 0x20 | 0)
        cose.push(0x01); // P-256 value (1)
        cose.push(0x21); // x label (-2 = 0x20 | 1)
        cose.push(0x58); // bytes with 1-byte length
        cose.push(0x20); // 32 bytes
        cose.extend_from_slice(&x);
        cose.push(0x22); // y label (-3 = 0x20 | 2)
        cose.push(0x58); // bytes with 1-byte length
        cose.push(0x20); // 32 bytes
        cose.extend_from_slice(&y);
        cose
    }

    #[test]
    fn test_parse_valid_cose_key() {
        let cose = valid_cose_key();
        let result = parse_cose_key(&cose);
        assert!(result.is_ok());

        let pubkey = result.unwrap();
        assert_eq!(pubkey.x[0], 0x6b);
        assert_eq!(pubkey.y[0], 0x4f);
    }

    #[test]
    fn test_parse_invalid_kty() {
        // kty: 1 (OKP instead of EC2)
        let mut cose = Vec::new();
        cose.push(0xa5); // map(5)
        cose.push(0x01); // kty label
        cose.push(0x01); // OKP value (1) - should fail
        cose.push(0x03); // alg label
        cose.push(0x26); // ES256
        cose.push(0x20); // crv label
        cose.push(0x01); // P-256
        cose.push(0x21); // x label
        cose.push(0x58);
        cose.push(0x20);
        cose.extend_from_slice(&[0u8; 32]);
        cose.push(0x22); // y label
        cose.push(0x58);
        cose.push(0x20);
        cose.extend_from_slice(&[0u8; 32]);

        let result = parse_cose_key(&cose);
        assert_eq!(result, Err(CoseError::UnsupportedKeyType));
    }

    #[test]
    fn test_parse_invalid_algorithm() {
        // alg: -257 (RS256 instead of ES256)
        let mut cose = Vec::new();
        cose.push(0xa5); // map(5)
        cose.push(0x01); // kty label
        cose.push(0x02); // EC2
        cose.push(0x03); // alg label
        cose.push(0x39); // negative int, 2-byte value follows
        cose.push(0x01); // high byte
        cose.push(0x00); // low byte -> -257
        cose.push(0x20); // crv label
        cose.push(0x01); // P-256
        cose.push(0x21); // x label
        cose.push(0x58);
        cose.push(0x20);
        cose.extend_from_slice(&[0u8; 32]);
        cose.push(0x22); // y label
        cose.push(0x58);
        cose.push(0x20);
        cose.extend_from_slice(&[0u8; 32]);

        let result = parse_cose_key(&cose);
        assert_eq!(result, Err(CoseError::UnsupportedAlgorithm));
    }

    #[test]
    fn test_parse_invalid_curve() {
        // crv: 2 (P-384 instead of P-256)
        let mut cose = Vec::new();
        cose.push(0xa5); // map(5)
        cose.push(0x01); // kty label
        cose.push(0x02); // EC2
        cose.push(0x03); // alg label
        cose.push(0x26); // ES256
        cose.push(0x20); // crv label
        cose.push(0x02); // P-384 (2) - should fail
        cose.push(0x21); // x label
        cose.push(0x58);
        cose.push(0x20);
        cose.extend_from_slice(&[0u8; 32]);
        cose.push(0x22); // y label
        cose.push(0x58);
        cose.push(0x20);
        cose.extend_from_slice(&[0u8; 32]);

        let result = parse_cose_key(&cose);
        assert_eq!(result, Err(CoseError::UnsupportedCurve));
    }

    #[test]
    fn test_parse_missing_x_coordinate() {
        // x座標がない
        let mut cose = Vec::new();
        cose.push(0xa4); // map(4) - missing x
        cose.push(0x01); // kty label
        cose.push(0x02); // EC2
        cose.push(0x03); // alg label
        cose.push(0x26); // ES256
        cose.push(0x20); // crv label
        cose.push(0x01); // P-256
        cose.push(0x22); // y label only
        cose.push(0x58);
        cose.push(0x20);
        cose.extend_from_slice(&[0u8; 32]);

        let result = parse_cose_key(&cose);
        assert_eq!(result, Err(CoseError::InvalidXCoordinate));
    }

    #[test]
    fn test_parse_invalid_coordinate_length() {
        // x座標が31バイト（不正）
        let mut cose = Vec::new();
        cose.push(0xa5); // map(5)
        cose.push(0x01); // kty label
        cose.push(0x02); // EC2
        cose.push(0x03); // alg label
        cose.push(0x26); // ES256
        cose.push(0x20); // crv label
        cose.push(0x01); // P-256
        cose.push(0x21); // x label
        cose.push(0x58);
        cose.push(0x1f); // 31 bytes - should fail
        cose.extend_from_slice(&[0u8; 31]);
        cose.push(0x22); // y label
        cose.push(0x58);
        cose.push(0x20);
        cose.extend_from_slice(&[0u8; 32]);

        let result = parse_cose_key(&cose);
        assert_eq!(result, Err(CoseError::InvalidCoordinateLength));
    }

    #[test]
    fn test_point_not_on_curve() {
        // 曲線上にない点
        let mut cose = Vec::new();
        cose.push(0xa5); // map(5)
        cose.push(0x01); // kty label
        cose.push(0x02); // EC2
        cose.push(0x03); // alg label
        cose.push(0x26); // ES256
        cose.push(0x20); // crv label
        cose.push(0x01); // P-256
        cose.push(0x21); // x label
        cose.push(0x58);
        cose.push(0x20);
        cose.extend_from_slice(&[1u8; 32]); // Invalid point
        cose.push(0x22); // y label
        cose.push(0x58);
        cose.push(0x20);
        cose.extend_from_slice(&[1u8; 32]); // Invalid point

        let result = parse_cose_key(&cose);
        assert_eq!(result, Err(CoseError::PointNotOnCurve));
    }

    #[test]
    fn test_public_key_to_uncompressed() {
        let pubkey = WebAuthnPublicKey {
            x: [1u8; 32],
            y: [2u8; 32],
        };

        let uncompressed = pubkey.to_uncompressed();

        assert_eq!(uncompressed.len(), 65);
        assert_eq!(uncompressed[0], 0x04);
        assert_eq!(&uncompressed[1..33], &[1u8; 32]);
        assert_eq!(&uncompressed[33..65], &[2u8; 32]);
    }

    #[test]
    fn test_public_key_from_uncompressed() {
        let mut bytes = [0u8; 65];
        bytes[0] = 0x04;
        bytes[1..33].copy_from_slice(&[1u8; 32]);
        bytes[33..65].copy_from_slice(&[2u8; 32]);

        let result = WebAuthnPublicKey::from_uncompressed(&bytes);
        assert!(result.is_ok());

        let pubkey = result.unwrap();
        assert_eq!(pubkey.x, [1u8; 32]);
        assert_eq!(pubkey.y, [2u8; 32]);
    }

    #[test]
    fn test_public_key_from_uncompressed_invalid_prefix() {
        let mut bytes = [0u8; 65];
        bytes[0] = 0x02; // compressed prefix - should fail

        let result = WebAuthnPublicKey::from_uncompressed(&bytes);
        assert_eq!(result, Err(CoseError::InvalidUncompressedPrefix));
    }

    #[test]
    fn test_public_key_from_uncompressed_invalid_length() {
        let bytes = [0u8; 64]; // too short

        let result = WebAuthnPublicKey::from_uncompressed(&bytes);
        assert_eq!(result, Err(CoseError::InvalidCoordinateLength));
    }
}
