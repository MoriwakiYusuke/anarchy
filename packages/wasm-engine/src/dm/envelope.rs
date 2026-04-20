//! DM envelope SCALE codec + W6 inner signed-hash helper.
//!
//! data-model.md §2.1 / contracts/wasm-engine-dm-api.md §W6 に準拠。

use parity_scale_codec::{Decode, Encode};
use wasm_bindgen::prelude::*;

use super::super::stealth::hash::blake2b_256;

/// DM protocol version (MVP = 1)。data-model.md §1.2 と同値。
pub const DM_PROTOCOL_VERSION: u8 = 1;

/// 復号後にパースされる DM ciphertext の内側。
///
/// `#[derive(Encode, Decode)]` による SCALE エンコーディングは送受信双方で
/// ビット一致が必要なので、フィールド順序・型は絶対に変更しない。v2 を
/// 導入する場合は別型 `DmEnvelopeV2` を並行定義する。
#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq)]
pub struct DmEnvelope {
    /// プロトコルバージョン (MVP = 1)。
    pub version: u8,
    /// 送信者メインアカウント (AccountId32 の raw 32 byte)。
    pub sender_account: [u8; 32],
    /// 送信時刻 (送信者ローカルの UNIX ms)。表示用、整合性検証には使わない。
    pub timestamp_ms: u64,
    /// 本文 (FR-017 により opaque bytes)。
    pub body: Vec<u8>,
    /// Sr25519 署名 (sender のメインアカウント秘密鍵による、W6 inner hash 対象)。
    pub signature: [u8; 64],
}

/// W6. `dm_compute_inner_signed_hash`
///
/// `sender_signature` (W1 の引数) が署名するハッシュを決定論的に計算する純関数。
/// 実装ドリフト防止のため送受信双方でこの関数のみを使う。
///
/// payload = 0x01 || sender_main_account || recipient_stealth || eph_pub
///           || timestamp_ms.to_le_bytes() || blake2b_256(body)
/// return blake2b_256(payload)
#[wasm_bindgen]
pub fn dm_compute_inner_signed_hash(
    sender_main_account: &[u8],
    recipient_stealth: &[u8],
    ephemeral_pubkey: &[u8],
    timestamp_ms: u64,
    body: &[u8],
) -> Result<Vec<u8>, JsError> {
    if sender_main_account.len() != 32 {
        return Err(JsError::new(
            "dm_compute_inner_signed_hash: sender_main_account must be 32 bytes",
        ));
    }
    if recipient_stealth.len() != 32 {
        return Err(JsError::new(
            "dm_compute_inner_signed_hash: recipient_stealth must be 32 bytes",
        ));
    }
    if ephemeral_pubkey.len() != 32 {
        return Err(JsError::new(
            "dm_compute_inner_signed_hash: ephemeral_pubkey must be 32 bytes",
        ));
    }

    Ok(compute_inner_signed_hash(
        sender_main_account,
        recipient_stealth,
        ephemeral_pubkey,
        timestamp_ms,
        body,
    )
    .to_vec())
}

/// 内部用 (非 wasm_bindgen) の純関数版。W1 / W2 の内部計算も必ずこれ経由で行う。
pub fn compute_inner_signed_hash(
    sender_main_account: &[u8],
    recipient_stealth: &[u8],
    ephemeral_pubkey: &[u8],
    timestamp_ms: u64,
    body: &[u8],
) -> [u8; 32] {
    let body_hash = blake2b_256(body);
    let mut payload = Vec::with_capacity(1 + 32 + 32 + 32 + 8 + 32);
    payload.push(DM_PROTOCOL_VERSION);
    payload.extend_from_slice(sender_main_account);
    payload.extend_from_slice(recipient_stealth);
    payload.extend_from_slice(ephemeral_pubkey);
    payload.extend_from_slice(&timestamp_ms.to_le_bytes());
    payload.extend_from_slice(&body_hash);
    blake2b_256(&payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_scale_roundtrip() {
        let envelope = DmEnvelope {
            version: DM_PROTOCOL_VERSION,
            sender_account: [0x11; 32],
            timestamp_ms: 1_714_000_000_000,
            body: b"hello, dm".to_vec(),
            signature: [0x22; 64],
        };
        let encoded = envelope.encode();
        let decoded = DmEnvelope::decode(&mut &encoded[..]).expect("decode ok");
        assert_eq!(decoded, envelope);
    }

    #[test]
    fn inner_hash_is_deterministic() {
        let sender = [0x01; 32];
        let recipient = [0x02; 32];
        let eph = [0x03; 32];
        let body = b"plaintext";
        let h1 = compute_inner_signed_hash(&sender, &recipient, &eph, 42, body);
        let h2 = compute_inner_signed_hash(&sender, &recipient, &eph, 42, body);
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 32);
    }

    #[test]
    fn inner_hash_changes_on_single_bit_flip() {
        let sender = [0x01; 32];
        let recipient = [0x02; 32];
        let eph = [0x03; 32];
        let body = b"plaintext";
        let baseline = compute_inner_signed_hash(&sender, &recipient, &eph, 42, body);

        let mut body_flip = body.to_vec();
        body_flip[0] ^= 0x01;
        assert_ne!(
            baseline,
            compute_inner_signed_hash(&sender, &recipient, &eph, 42, &body_flip)
        );

        assert_ne!(
            baseline,
            compute_inner_signed_hash(&sender, &recipient, &eph, 43, body)
        );

        let mut eph_flip = eph;
        eph_flip[0] ^= 0x01;
        assert_ne!(
            baseline,
            compute_inner_signed_hash(&sender, &recipient, &eph_flip, 42, body)
        );
    }

    // `dm_compute_inner_signed_hash` の length-guard は wasm ターゲット限定でのみ
    // JsError を構築できる。ホストテストでは `compute_inner_signed_hash` の
    // ドロップイン呼出しで挙動を確認し、wasm 境界側は Phase 3 の wasm-pack
    // テスト (T029–T031) で検証する。
}
