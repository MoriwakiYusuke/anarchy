//! Type definitions for the stealth pallet

use frame_support::pallet_prelude::*;
use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;

/// ステルス送金時に記録されるエフェメラル公開鍵エントリ
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct EphemeralKeyEntry<AccountId> {
    /// エフェメラル公開鍵 (X25519 public key, 32 bytes)
    pub ephemeral_pubkey: [u8; 32],

    /// ステルスアドレス (送金先)
    pub stealth_address: AccountId,
}
