//! DM 暗号化パイプライン (ECDH + HKDF + AES-GCM + stealth 導出)。
//!
//! Phase 3 で W1 (`dm_encrypt_and_pad`) / W3 (`dm_generate_sender_stealth`)
//! / W4 (`dm_fragment_ciphertext`) を実装。W5 (`dm_derive_recipient_stealth`)
//! と HKDF ヘルパは Phase 2 から継続。

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use curve25519_dalek::constants::ED25519_BASEPOINT_TABLE;
use curve25519_dalek::{edwards::CompressedEdwardsY, Scalar};
use hkdf::Hkdf;
use parity_scale_codec::Encode;
use rand_core::{OsRng, RngCore};
use schnorrkel::{ExpansionMode, MiniSecretKey};
use sha2::Sha256;
use wasm_bindgen::prelude::*;
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret};
use zeroize::Zeroizing;

use super::envelope::{DmEnvelope, DM_PROTOCOL_VERSION};
use super::padding::{pad_iso7816_4, select_padding_bucket, AES_GCM_TAG_LEN};
use super::super::merkle;
use super::super::stealth::hash::blake2b_256;

/// HKDF-SHA256 salt (プロトコル識別子)。data-model.md §2.3 参照。
pub const HKDF_SALT: &[u8] = b"anarchy-dm-v1";

/// HKDF 出力長 (32 byte AES-256 key + 12 byte AES-GCM nonce)。
pub const HKDF_OKM_LEN: usize = 44;

/// `hkdf_okm(shared, salt, info) -> [u8; 44]`
///
/// W1 / W2 双方で使う HKDF-SHA256 Extract+Expand。key (32B) || nonce (12B) を
/// 一度の expand で出力する。
pub fn hkdf_okm(shared: &[u8; 32], salt: &[u8], info: &[u8]) -> [u8; HKDF_OKM_LEN] {
    let hk = Hkdf::<Sha256>::new(Some(salt), shared);
    let mut okm = [0u8; HKDF_OKM_LEN];
    hk.expand(info, &mut okm)
        .expect("HKDF-SHA256 expand to 44 bytes is within RFC 5869 limit");
    okm
}

/// W5 の wasm binding の戻り値。
#[wasm_bindgen]
pub struct DmStealthDerivation {
    stealth_pubkey: [u8; 32],
    ephemeral_pubkey: [u8; 32],
    shared_secret: [u8; 32],
}

#[wasm_bindgen]
impl DmStealthDerivation {
    /// 受信者 stealth 公開鍵 (Ed25519 compressed, 32B)。
    #[wasm_bindgen(getter)]
    pub fn stealth_pubkey(&self) -> Vec<u8> {
        self.stealth_pubkey.to_vec()
    }

    /// ephemeral 公開鍵 (X25519, 32B)。
    #[wasm_bindgen(getter)]
    pub fn ephemeral_pubkey(&self) -> Vec<u8> {
        self.ephemeral_pubkey.to_vec()
    }

    /// X25519 DH 結果 (HKDF ikm, 32B)。
    #[wasm_bindgen(getter)]
    pub fn shared_secret(&self) -> Vec<u8> {
        self.shared_secret.to_vec()
    }
}

/// W5. `dm_derive_recipient_stealth`
///
/// 送信者が「受信メタアドレス + ephemeral_priv」から受信者 stealth pubkey と
/// shared secret を一括導出する。既存 `stealth::address::derive_stealth_address`
/// は内部で ephemeral を生成してしまうため、DM では `eph_priv` を再利用して
/// HKDF 入力を揃える必要があり、本関数を別途用意する。
///
/// 呼出側は `ephemeral_priv` を呼出後にゼロクリアすること (FR-021)。
#[wasm_bindgen]
pub fn dm_derive_recipient_stealth(
    recipient_scan_pub: &[u8],
    recipient_spend_pub: &[u8],
    ephemeral_priv: &[u8],
) -> Result<DmStealthDerivation, JsError> {
    if recipient_scan_pub.len() != 32 {
        return Err(JsError::new(
            "dm_derive_recipient_stealth: recipient_scan_pub must be 32 bytes",
        ));
    }
    if recipient_spend_pub.len() != 32 {
        return Err(JsError::new(
            "dm_derive_recipient_stealth: recipient_spend_pub must be 32 bytes",
        ));
    }
    if ephemeral_priv.len() != 32 {
        return Err(JsError::new(
            "dm_derive_recipient_stealth: ephemeral_priv must be 32 bytes",
        ));
    }

    let mut eph_priv_bytes = [0u8; 32];
    eph_priv_bytes.copy_from_slice(ephemeral_priv);
    let eph_secret = StaticSecret::from(eph_priv_bytes);
    let eph_pub = X25519PublicKey::from(&eph_secret);

    let mut scan_pub_bytes = [0u8; 32];
    scan_pub_bytes.copy_from_slice(recipient_scan_pub);
    let scan_pub = X25519PublicKey::from(scan_pub_bytes);

    let shared = eph_secret.diffie_hellman(&scan_pub);
    let h = blake2b_256(shared.as_bytes());
    let h_scalar = Scalar::from_bytes_mod_order(h);

    let mut spend_pub_bytes = [0u8; 32];
    spend_pub_bytes.copy_from_slice(recipient_spend_pub);
    let spend_point = CompressedEdwardsY(spend_pub_bytes)
        .decompress()
        .ok_or_else(|| {
            JsError::new(
                "dm_derive_recipient_stealth: recipient_spend_pub is not a valid Edwards point",
            )
        })?;

    let h_g = ED25519_BASEPOINT_TABLE * &h_scalar;
    let stealth_point = spend_point + h_g;
    let stealth_pubkey: [u8; 32] = stealth_point.compress().to_bytes();

    let mut shared_bytes = [0u8; 32];
    shared_bytes.copy_from_slice(shared.as_bytes());

    Ok(DmStealthDerivation {
        stealth_pubkey,
        ephemeral_pubkey: *eph_pub.as_bytes(),
        shared_secret: shared_bytes,
    })
}

// =============================================================================
// W1. dm_encrypt_and_pad
// =============================================================================

/// W1 の出力。on-chain `DmDispatch` の構築に必要なフィールドを保持する。
#[wasm_bindgen]
pub struct DmEncryptedOutput {
    ciphertext: Vec<u8>,
    ephemeral_pubkey: [u8; 32],
    recipient_stealth: [u8; 32],
    padding_bucket: u32,
}

#[wasm_bindgen]
impl DmEncryptedOutput {
    /// 暗号化済み envelope。長さは `padding_bucket` と必ず一致。
    #[wasm_bindgen(getter)]
    pub fn ciphertext(&self) -> Vec<u8> {
        self.ciphertext.clone()
    }
    /// X25519 ephemeral pubkey (32B)。
    #[wasm_bindgen(getter)]
    pub fn ephemeral_pubkey(&self) -> Vec<u8> {
        self.ephemeral_pubkey.to_vec()
    }
    /// 受信者 stealth pubkey (Ed25519 compressed, 32B)。
    #[wasm_bindgen(getter)]
    pub fn recipient_stealth(&self) -> Vec<u8> {
        self.recipient_stealth.to_vec()
    }
    /// 採用されたバケット値 (1024 / 4096 / 16384 / 65536 / 262144)。
    #[wasm_bindgen(getter)]
    pub fn padding_bucket(&self) -> u32 {
        self.padding_bucket
    }
}

/// AES-GCM AAD: `recipient_stealth || eph_pub || (padded_len as u32 BE) || version`。
fn build_aad(recipient_stealth: &[u8; 32], eph_pub: &[u8; 32], padded_len: u32) -> Vec<u8> {
    let mut aad = Vec::with_capacity(32 + 32 + 4 + 1);
    aad.extend_from_slice(recipient_stealth);
    aad.extend_from_slice(eph_pub);
    aad.extend_from_slice(&padded_len.to_be_bytes());
    aad.push(DM_PROTOCOL_VERSION);
    aad
}

/// W1. `dm_encrypt_and_pad`
///
/// **Contract deviation (T029 注記)**: 公開仕様の step 1「W1 内部で eph_priv を
/// 生成」を、Constitution II により外部署名フローと整合させるため `ephemeral_priv`
/// を引数で受け取る形に変更している。呼出側は `getrandom`/`crypto.getRandomValues`
/// で 32B を生成し、`dm_compute_inner_signed_hash` の対象となる `eph_pub` を W5
/// 経由で確定したうえで、ウォレット signer で `sender_signature` を作成し、最後に
/// 本関数を呼ぶ。`ephemeral_priv` は本関数内で `Zeroizing` により drop 時に消去
/// されるが、JS 側でも当該バッファを使用後にゼロクリアすること (FR-021)。
#[wasm_bindgen]
pub fn dm_encrypt_and_pad(
    recipient_scan_pub: &[u8],
    recipient_spend_pub: &[u8],
    sender_main_account: &[u8],
    sender_signature: &[u8],
    body: &[u8],
    timestamp_ms: u64,
    ephemeral_priv: &[u8],
) -> Result<DmEncryptedOutput, JsError> {
    if recipient_scan_pub.len() != 32 {
        return Err(JsError::new("invalid scan pub length"));
    }
    if recipient_spend_pub.len() != 32 {
        return Err(JsError::new("invalid spend pub length"));
    }
    if sender_main_account.len() != 32 {
        return Err(JsError::new("invalid sender account length"));
    }
    if sender_signature.len() != 64 {
        return Err(JsError::new("invalid signature length"));
    }
    if ephemeral_priv.len() != 32 {
        return Err(JsError::new("invalid ephemeral priv length"));
    }

    // ステルス導出 (eph_pub / recipient_stealth / shared)。Edwards 検証もここ。
    let derived = dm_derive_recipient_stealth(
        recipient_scan_pub,
        recipient_spend_pub,
        ephemeral_priv,
    )?;
    let eph_pub_vec = derived.ephemeral_pubkey;
    let stealth_vec = derived.stealth_pubkey;
    let shared_secret = Zeroizing::new(derived.shared_secret);

    // ephemeral_priv のローカルコピー (drop 時にゼロクリア)。本関数の制御下を
    // 出ない。呼出側のスライスは Rust では mutate できないので JS 側に責務がある。
    let mut eph_priv_copy = Zeroizing::new([0u8; 32]);
    eph_priv_copy.copy_from_slice(ephemeral_priv);

    // 送信者 envelope を構築 (W6 の sig hash 対象に対応)。
    let mut sender_acct = [0u8; 32];
    sender_acct.copy_from_slice(sender_main_account);
    let mut signature = [0u8; 64];
    signature.copy_from_slice(sender_signature);

    let envelope = DmEnvelope {
        version: DM_PROTOCOL_VERSION,
        sender_account: sender_acct,
        timestamp_ms,
        body: body.to_vec(),
        signature,
    };
    let encoded = envelope.encode();

    // バケット選択 — encoded.len() + ISO 7816 terminator (1B) を AES-GCM tag
    // 付きで収める最小バケット。padded plaintext は `bucket - tag` バイト。
    let bucket = select_padding_bucket(encoded.len() + 1)
        .ok_or_else(|| JsError::new("body too large"))?;
    let padded_len = bucket - AES_GCM_TAG_LEN;
    let padded = pad_iso7816_4(&encoded, padded_len)
        .ok_or_else(|| JsError::new("padding overflow: encoded body >= bucket - tag"))?;

    // HKDF-SHA256: info = recipient_stealth || eph_pub.
    let mut info = Vec::with_capacity(64);
    info.extend_from_slice(&stealth_vec);
    info.extend_from_slice(&eph_pub_vec);
    let okm = Zeroizing::new(hkdf_okm(&shared_secret, HKDF_SALT, &info));

    let key = Key::<Aes256Gcm>::from_slice(&okm[..32]);
    let nonce = Nonce::from_slice(&okm[32..44]);
    let cipher = Aes256Gcm::new(key);

    let aad = build_aad(&stealth_vec, &eph_pub_vec, padded_len as u32);
    let ciphertext = cipher
        .encrypt(
            nonce,
            Payload {
                msg: &padded,
                aad: &aad,
            },
        )
        .map_err(|_| JsError::new("aes-gcm encrypt failed"))?;

    debug_assert_eq!(
        ciphertext.len(),
        bucket,
        "ciphertext length must equal bucket"
    );

    Ok(DmEncryptedOutput {
        ciphertext,
        ephemeral_pubkey: eph_pub_vec,
        recipient_stealth: stealth_vec,
        padding_bucket: bucket as u32,
    })
}

// =============================================================================
// W3. dm_generate_sender_stealth
// =============================================================================

/// W3 の出力。`secret_seed` は `send_dm` 1 tx の署名にのみ使い、JS 側で即時
/// ゼロクリアする (Constitution II 限定例外)。
#[wasm_bindgen]
pub struct DmSenderStealth {
    account_id: [u8; 32],
    secret_seed: [u8; 32],
}

#[wasm_bindgen]
impl DmSenderStealth {
    /// Sr25519 公開鍵 (= AccountId32 raw bytes, 32B)。
    #[wasm_bindgen(getter)]
    pub fn account_id(&self) -> Vec<u8> {
        self.account_id.to_vec()
    }
    /// Sr25519 seed (32B)。**JS 側で 1 回使ったら即ゼロクリアすること**。
    #[wasm_bindgen(getter)]
    pub fn secret_seed(&self) -> Vec<u8> {
        self.secret_seed.to_vec()
    }
}

/// W3. `dm_generate_sender_stealth`
///
/// 32B の fresh entropy → MiniSecretKey → 公開鍵展開 (Ed25519 mode) を行い、
/// 公開鍵 (account_id) と seed の組を返す。`MiniSecretKey` / `SecretKey` は
/// 関数スコープ終了時に drop されるが、`secret_seed` は WASM→JS 越境のため
/// 必要悪として残る (CT-1)。
#[wasm_bindgen]
pub fn dm_generate_sender_stealth() -> DmSenderStealth {
    let mut seed = [0u8; 32];
    OsRng.fill_bytes(&mut seed);

    let mini = MiniSecretKey::from_bytes(&seed)
        .expect("MiniSecretKey accepts any 32B input");
    let kp = mini.expand_to_keypair(ExpansionMode::Ed25519);
    let account_id = kp.public.to_bytes();

    DmSenderStealth {
        account_id,
        secret_seed: seed,
    }
}

// =============================================================================
// W4. dm_fragment_ciphertext
// =============================================================================

/// W4 の出力。Merkle root と各フラグメント (raw bytes) + index 毎の Merkle proof。
///
/// proof は chain-node の `storage_uploadFragment` RPC が `verify_merkle_proof` で
/// 検証する。`rs_merkle::MerkleProof::to_bytes()` 形式 (兄弟ノードハッシュ列)。
#[wasm_bindgen]
pub struct DmFragmentedOutput {
    merkle_root: [u8; 32],
    fragments: Vec<Vec<u8>>,
    merkle_proofs: Vec<Vec<u8>>,
}

#[wasm_bindgen]
impl DmFragmentedOutput {
    /// Merkle root (Blake2b-256, 32B)。on-chain `DmContentRef.root` 用。
    #[wasm_bindgen(getter)]
    pub fn merkle_root(&self) -> Vec<u8> {
        self.merkle_root.to_vec()
    }
    /// フラグメント数。
    #[wasm_bindgen(getter)]
    pub fn fragment_count(&self) -> usize {
        self.fragments.len()
    }
    /// `idx` 番目のフラグメント (raw bytes)。
    pub fn fragment(&self, idx: usize) -> Option<Vec<u8>> {
        self.fragments.get(idx).cloned()
    }
    /// `idx` 番目のフラグメントに対応する Merkle proof (rs_merkle to_bytes 形式)。
    /// chain-node の `storage_uploadFragment` の `proof` フィールドにそのまま渡す。
    pub fn proof(&self, idx: usize) -> Option<Vec<u8>> {
        self.merkle_proofs.get(idx).cloned()
    }
}

/// W4. `dm_fragment_ciphertext`
///
/// ciphertext を `n` 等分し (`k` バイトずつ chunk するわけではなく、長さは
/// 既存 post パイプラインと同じく `ceil(len/n)`)、Merkle 木を Blake2b で構築する。
/// k はバリデータ側で stealth k-of-n の閾値検証に使うパラメータで、ここではメタ
/// 情報として呼出側に返す責務はない (引数で受け取り content_ref に伝搬される)。
///
/// 真の SSS/Reed-Solomon 分散は post パイプラインで `kzg::hybrid::hybrid_split`
/// が担う; DM では「同じ ciphertext を n 個に区切って merkle 化する」ラッパに
/// 留め、storage-node 側の格納モデルと整合させる。
#[wasm_bindgen]
pub fn dm_fragment_ciphertext(
    ciphertext: &[u8],
    k: u32,
    n: u32,
) -> Result<DmFragmentedOutput, JsError> {
    if n == 0 {
        return Err(JsError::new("n must be > 0"));
    }
    if k == 0 || k > n {
        return Err(JsError::new("require 0 < k <= n"));
    }
    if ciphertext.is_empty() {
        return Err(JsError::new("ciphertext must not be empty"));
    }

    let n_us = n as usize;
    let chunk = ciphertext.len().div_ceil(n_us);
    let mut fragments: Vec<Vec<u8>> = Vec::with_capacity(n_us);
    for i in 0..n_us {
        let start = i * chunk;
        let end = ((i + 1) * chunk).min(ciphertext.len());
        if start >= ciphertext.len() {
            // n がデータ長より大きい場合は空フラグメントになるのを避ける。
            break;
        }
        fragments.push(ciphertext[start..end].to_vec());
    }

    let frag_refs: Vec<&[u8]> = fragments.iter().map(|f| f.as_slice()).collect();
    let merkle = merkle::merkle_build_internal(&frag_refs)
        .map_err(|e| JsError::new(&format!("merkle build failed: {}", e)))?;

    let mut merkle_proofs = Vec::with_capacity(fragments.len());
    for i in 0..fragments.len() {
        let proof = merkle
            .generate_proof(i)
            .map_err(|e| JsError::new(&format!("merkle proof failed: {}", e)))?;
        merkle_proofs.push(proof);
    }

    Ok(DmFragmentedOutput {
        merkle_root: merkle.root,
        fragments,
        merkle_proofs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hkdf_okm_length_and_determinism() {
        let shared = [0x11u8; 32];
        let info = b"info-field";
        let a = hkdf_okm(&shared, HKDF_SALT, info);
        let b = hkdf_okm(&shared, HKDF_SALT, info);
        assert_eq!(a.len(), HKDF_OKM_LEN);
        assert_eq!(a, b);
    }

    #[test]
    fn hkdf_okm_diverges_on_input_change() {
        let shared = [0x11u8; 32];
        let base = hkdf_okm(&shared, HKDF_SALT, b"info");

        let mut shared2 = shared;
        shared2[0] ^= 0x01;
        assert_ne!(base, hkdf_okm(&shared2, HKDF_SALT, b"info"));

        assert_ne!(base, hkdf_okm(&shared, HKDF_SALT, b"info2"));
        assert_ne!(base, hkdf_okm(&shared, b"different-salt", b"info"));
    }

    #[test]
    fn derive_recipient_stealth_matches_existing_scan_math() {
        // 受信者の鍵材料 (本テストでは秘密鍵も含めて固定し、両側の計算をクロスチェック)。
        let recipient_scan_priv = [0x07u8; 32];
        let recipient_scan_sec = StaticSecret::from(recipient_scan_priv);
        let recipient_scan_pub = X25519PublicKey::from(&recipient_scan_sec);

        // spend 鍵はテスト用に ed25519 の basepoint を scalar 倍して得る
        // (実運用では stealth keypair 生成ロジックが作るもの)。
        let spend_scalar = Scalar::from_bytes_mod_order([0x03u8; 32]);
        let spend_point = ED25519_BASEPOINT_TABLE * &spend_scalar;
        let recipient_spend_pub = spend_point.compress().to_bytes();

        // 送信者側の ephemeral。
        let eph_priv = [0x42u8; 32];

        let derived = dm_derive_recipient_stealth(
            recipient_scan_pub.as_bytes(),
            &recipient_spend_pub,
            &eph_priv,
        )
        .expect("derive ok");

        // 受信者側の走査計算 (既存 stealth::scan の式を手で再現)。
        let eph_secret = StaticSecret::from(eph_priv);
        let eph_pub_check = X25519PublicKey::from(&eph_secret);
        assert_eq!(&derived.ephemeral_pubkey, eph_pub_check.as_bytes());

        let shared_recv = recipient_scan_sec.diffie_hellman(&eph_pub_check);
        assert_eq!(&derived.shared_secret, shared_recv.as_bytes());

        let h = blake2b_256(shared_recv.as_bytes());
        let h_scalar = Scalar::from_bytes_mod_order(h);
        let h_g = ED25519_BASEPOINT_TABLE * &h_scalar;
        let expected_stealth = (spend_point + h_g).compress().to_bytes();
        assert_eq!(derived.stealth_pubkey, expected_stealth);
    }

    // 長さ不正 / 無効 Edwards 点のエラー分岐は JsError 構築を伴うため、ホスト上の
    // `cargo test` では検証不能 (wasm-bindgen が wasm-only の import を呼ぶ)。
    // これらの分岐は Phase 3 の `wasm-pack test` (T029–T031) でカバーする。
}
