//! PreRuntime digest に miner の AccountId を埋め込み、runtime 側で抽出するための
//! `FindAuthor` 実装。Engine ID は `sp_consensus_pow::POW_ENGINE_ID` (= `b"pow_"`)。
//! sc_consensus_pow のマイニングワーカが PreRuntime ダイジェストに使う engine ID と
//! 一致させる必要がある。chain ローカルな別 ID (例: b"ANRC") を使うと runtime 側で
//! author が抽出できず block reward が常にスキップされる。

use frame_support::traits::FindAuthor;
use parity_scale_codec::Decode;
use sp_runtime::AccountId32;
use sp_runtime::ConsensusEngineId;

pub use sp_consensus_pow::POW_ENGINE_ID;

pub struct PowAuthor;

impl FindAuthor<AccountId32> for PowAuthor {
    fn find_author<'a, I>(digests: I) -> Option<AccountId32>
    where
        I: 'a + IntoIterator<Item = (ConsensusEngineId, &'a [u8])>,
    {
        for (id, mut data) in digests {
            if id == POW_ENGINE_ID {
                if let Ok(a) = AccountId32::decode(&mut data) {
                    return Some(a);
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parity_scale_codec::Encode;

    #[test]
    fn extracts_account_id_from_pre_runtime_digest() {
        let acc = AccountId32::from([1u8; 32]);
        let bytes = acc.encode();
        let result = PowAuthor::find_author(vec![(POW_ENGINE_ID, bytes.as_slice())]);
        assert_eq!(result, Some(acc));
    }

    #[test]
    fn returns_none_for_unknown_engine_id() {
        let acc = AccountId32::from([1u8; 32]);
        let bytes = acc.encode();
        // sp_consensus_pow::POW_ENGINE_ID 以外 (例: 旧 b"ANRC" や b"aura") は無視される
        let result = PowAuthor::find_author(vec![(*b"ANRC", bytes.as_slice())]);
        assert_eq!(result, None);
    }

    #[test]
    fn returns_none_for_garbled_payload() {
        let result = PowAuthor::find_author(vec![(POW_ENGINE_ID, &[0u8; 5][..])]);
        assert_eq!(result, None);
    }
}
