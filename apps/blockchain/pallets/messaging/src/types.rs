//! On-chain type definitions for `pallet-messaging`.
//!
//! 詳細は [`specs/019-direct-messages/data-model.md`] §1.1–1.3 を参照。

use frame_support::pallet_prelude::*;
use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;

/// Protocol version constant.
///
/// 将来の暗号スイート変更は新 extrinsic (`send_dm_v2` + `DmContentRefV2`) で導入する
/// ため、v1 の間は本定数が引数や storage 内部に現れる必要はない (contracts/pallet-
/// messaging-extrinsics.md §M2 Interface Stability)。
pub const DM_PROTOCOL_VERSION: u8 = 1;

/// 受信者が `publish_dm_key` でチェーンに公開するメタアドレス。
///
/// 既存 `pallet-stealth` / `packages/wasm-engine/src/stealth/types.rs` の
/// `spend_pubkey` / `view_pubkey` と**バイト同値**。命名差 (scan_pub/spend_pub)
/// は Substrate 側の pallet naming convention に合わせたもので、概念的な対応関係
/// はドキュメント (data-model.md §1.1) に記載。
#[derive(
    Clone, Encode, Decode, DecodeWithMemTracking, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug,
)]
pub struct DmMetaAddress {
    /// X25519 scan public key (ECDH 用、受信者スキャン鍵の公開部)。
    pub scan_pub: [u8; 32],
    /// Ed25519 compressed Edwards point。`stealth_pubkey = K_spend + H(shared)*G` の起点。
    pub spend_pub: [u8; 32],
}

/// 分散ストレージに保存された DM 暗号文の参照。
///
/// `pallet-post` の `PostContent` と構造が似るが、用途分離のため独立型。
/// 将来 `DmContentRefV2` を並行定義する場合も本型は変更しない。
#[derive(
    Clone, Encode, Decode, DecodeWithMemTracking, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug,
)]
pub struct DmContentRef {
    /// MerkleRoot (Blake2b-256)。
    pub root: [u8; 32],
    /// 復元に必要な最小断片数 (k > 0)。
    pub k: u32,
    /// 総断片数 (k <= n <= 255)。
    pub n: u32,
    /// 暗号文サイズ (パディング後。FR-026 のバケット値のいずれか)。
    pub ciphertext_len: u64,
}

/// `send_dm` 成立時にブロック単位で積み上げる dispatch エントリ。
///
/// 受信者スキャナはこれを `DmDispatchesByBlock` から取得して自分宛を検出する。
#[derive(
    Clone, Encode, Decode, DecodeWithMemTracking, PartialEq, Eq, TypeInfo, MaxEncodedLen, RuntimeDebug,
)]
pub struct DmDispatch<AccountId> {
    /// 受信者 stealth address。送信者が `ephemeral_priv × recipient_scan_pub` から導出。
    pub recipient_stealth: AccountId,
    /// 送信者が生成した ephemeral 公開鍵 (X25519, 32B)。
    pub ephemeral_pubkey: [u8; 32],
    /// 分散ストレージ参照 (MerkleRoot + k/n + ciphertext_len)。
    pub content: DmContentRef,
}
