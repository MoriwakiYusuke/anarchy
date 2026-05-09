//! # Economic Params Pallet (TSTS F5)
//!
//! TSTS 経済モデルの主要パラメータを on-chain で governance-tunable にする pallet。
//!
//! ## 設計方針
//!
//! Substrate 標準の `pallet-parameters` は強力だが大規模 refactor を要求するため、
//! Anarchy ローンチに必要な「市場圧でリアルタイム調整したい paramater 群」に絞った
//! 独立 pallet を用意する。残りの ConstU* は governance 不要 (低リスク) として据え置く。
//!
//! ## 対象パラメータ
//!
//! 1. **Post 配分比率** (`post_storage_share` / `post_reaction_share` / `post_burn_share`)
//! 2. **DM 配分比率** (`dm_storage_share` / `dm_stealth_share` / `dm_burn_share`)
//! 3. **Block reward 配分比率** (`miner_share` / `storage_share` / `reaction_share`)
//! 4. **EIP-1559 base fee の min/max** (混雑期に調整したい)
//! 5. **Reactor lock 額** (Sybil 攻撃検知時に瞬間的に上げたい)
//! 6. **Storage bond per GB** (mainnet 価格次第で調整)
//! 7. **Slash rate per fail** (攻撃モデル変動に対応)
//!
//! ## 認可モデル
//!
//! `set_*` extrinsic は `T::GovernanceOrigin: EnsureOrigin` で gating する。
//! mainnet 初期は `EnsureRoot` (sudo) → multisig (`pallet_collective`) → token-weighted
//! referenda の段階的移行を想定する。
//!
//! ## consume 側 (storage / reaction / etc.)
//!
//! 既存 pallet は `Config::PostStorageSharePermill: Get<Permill>` 形で受け取っているので、
//! ここから値を fetch する `EconomicParams::post_storage_share()` を `impl Get<Permill>`
//! として `runtime/src/lib.rs` で adapt する。

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod tests;

use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;

/// TSTS F9: 経済メトリクスのスナップショット (node 側 Prometheus exporter で取得).
///
/// runtime から chain client / node 側へ提供する型。各 pallet の主要 storage を
/// 1 回の Runtime API 呼び出しで一括取得できるようにする (storage_prefix 計算より
/// 型安全)。
#[derive(Clone, Encode, Decode, TypeInfo, Debug, PartialEq, Eq, Default)]
pub struct EconomicSnapshot {
    pub storage_pool: u128,
    pub reaction_pool: u128,
    pub stealth_pool: u128,
    pub base_fee: u128,
    pub total_active_bond: u128,
    pub faucet_minted: u128,
    pub total_issuance: u128,
    pub gas_used_this_block: u32,
}

sp_api::decl_runtime_apis! {
    /// TSTS F9: 経済メトリクスを Substrate Prometheus 経路に流すための Runtime API.
    ///
    /// node service が起動時にこの API を呼び出して、Substrate 標準の prometheus_registry
    /// に Gauge を register + 定期 polling で更新する。外部 exporter (F3) と冗長だが、
    /// node 単体で監視可能になる利点がある。
    pub trait EconomicMetricsApi {
        /// 直近 best block の経済メトリクス snapshot を返す.
        fn snapshot() -> EconomicSnapshot;
    }
}

#[frame_support::pallet]
pub mod pallet {
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use sp_runtime::Permill;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        /// パラメータ変更を許可される origin (例: EnsureRoot, Council majority).
        type GovernanceOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        // === デフォルト値 (genesis 直後の値) ===
        // 既存 ConstU* と同じ値を入れることで「F5 を導入したら挙動が変わる」副作用を回避する。

        #[pallet::constant]
        type DefaultPostStorageSharePermill: Get<Permill>;
        #[pallet::constant]
        type DefaultPostReactionSharePermill: Get<Permill>;
        #[pallet::constant]
        type DefaultDmStorageSharePermill: Get<Permill>;
        #[pallet::constant]
        type DefaultDmStealthSharePermill: Get<Permill>;
        #[pallet::constant]
        type DefaultMinerSharePermill: Get<Permill>;
        #[pallet::constant]
        type DefaultStorageSharePermill: Get<Permill>;
        #[pallet::constant]
        type DefaultReactionSharePermill: Get<Permill>;
        #[pallet::constant]
        type DefaultReactorLockMin: Get<u128>;
        #[pallet::constant]
        type DefaultBondPerGB: Get<u128>;
        #[pallet::constant]
        type DefaultSlashRatePerFailPpm: Get<u32>;
        #[pallet::constant]
        type DefaultBaseFeeMin: Get<u128>;
        #[pallet::constant]
        type DefaultBaseFeeMax: Get<u128>;
    }

    // ─── Storage ──────────────────────────────────────────────────────────
    // governance で書き換えられた "現在値" を保持する。`None` (未設定) なら Default* が使われる。

    #[pallet::storage]
    #[pallet::getter(fn post_storage_share)]
    pub type PostStorageSharePermill<T: Config> = StorageValue<_, Permill, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn post_reaction_share)]
    pub type PostReactionSharePermill<T: Config> = StorageValue<_, Permill, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn dm_storage_share)]
    pub type DmStorageSharePermill<T: Config> = StorageValue<_, Permill, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn dm_stealth_share)]
    pub type DmStealthSharePermill<T: Config> = StorageValue<_, Permill, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn miner_share)]
    pub type MinerSharePermill<T: Config> = StorageValue<_, Permill, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn storage_share)]
    pub type StorageSharePermill<T: Config> = StorageValue<_, Permill, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn reaction_share)]
    pub type ReactionSharePermill<T: Config> = StorageValue<_, Permill, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn reactor_lock_min)]
    pub type ReactorLockMin<T: Config> = StorageValue<_, u128, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn bond_per_gb)]
    pub type BondPerGB<T: Config> = StorageValue<_, u128, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn slash_rate_per_fail_ppm)]
    pub type SlashRatePerFailPpm<T: Config> = StorageValue<_, u32, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn base_fee_min)]
    pub type BaseFeeMin<T: Config> = StorageValue<_, u128, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn base_fee_max)]
    pub type BaseFeeMax<T: Config> = StorageValue<_, u128, OptionQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        ParameterUpdated { key: ParameterKey, old_value: Option<u128>, new_value: u128 },
    }

    /// パラメータの種類 (event ログ用).
    #[derive(Clone, Copy, Encode, Decode, parity_scale_codec::DecodeWithMemTracking, TypeInfo, MaxEncodedLen, RuntimeDebug, PartialEq, Eq)]
    pub enum ParameterKey {
        PostStorageShare,
        PostReactionShare,
        DmStorageShare,
        DmStealthShare,
        MinerShare,
        StorageShare,
        ReactionShare,
        ReactorLockMin,
        BondPerGB,
        SlashRatePerFailPpm,
        BaseFeeMin,
        BaseFeeMax,
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Permill > 100% は無効
        InvalidPermill,
        /// 配分比率の合計が 100% を超える
        SharesSumExceedsHundred,
        /// BaseFeeMin > BaseFeeMax は無効
        InvertedBaseFeeRange,
    }

    // ─── Calls ────────────────────────────────────────────────────────────
    // 各パラメータ独立の set_* extrinsic. 同時に複数変更したい場合は batch 推奨.

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// post 分配比率の storage 行きシェアを変更.
        #[pallet::call_index(0)]
        #[pallet::weight(T::DbWeight::get().reads_writes(1, 1))]
        pub fn set_post_storage_share(origin: OriginFor<T>, new: Permill) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            let old = PostStorageSharePermill::<T>::get().map(|p| p.deconstruct() as u128);
            PostStorageSharePermill::<T>::put(new);
            Self::deposit_event(Event::ParameterUpdated {
                key: ParameterKey::PostStorageShare,
                old_value: old,
                new_value: new.deconstruct() as u128,
            });
            Ok(())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(T::DbWeight::get().reads_writes(1, 1))]
        pub fn set_post_reaction_share(origin: OriginFor<T>, new: Permill) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            let old = PostReactionSharePermill::<T>::get().map(|p| p.deconstruct() as u128);
            PostReactionSharePermill::<T>::put(new);
            Self::deposit_event(Event::ParameterUpdated {
                key: ParameterKey::PostReactionShare,
                old_value: old,
                new_value: new.deconstruct() as u128,
            });
            Ok(())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(T::DbWeight::get().reads_writes(1, 1))]
        pub fn set_dm_storage_share(origin: OriginFor<T>, new: Permill) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            let old = DmStorageSharePermill::<T>::get().map(|p| p.deconstruct() as u128);
            DmStorageSharePermill::<T>::put(new);
            Self::deposit_event(Event::ParameterUpdated {
                key: ParameterKey::DmStorageShare,
                old_value: old,
                new_value: new.deconstruct() as u128,
            });
            Ok(())
        }

        #[pallet::call_index(3)]
        #[pallet::weight(T::DbWeight::get().reads_writes(1, 1))]
        pub fn set_dm_stealth_share(origin: OriginFor<T>, new: Permill) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            let old = DmStealthSharePermill::<T>::get().map(|p| p.deconstruct() as u128);
            DmStealthSharePermill::<T>::put(new);
            Self::deposit_event(Event::ParameterUpdated {
                key: ParameterKey::DmStealthShare,
                old_value: old,
                new_value: new.deconstruct() as u128,
            });
            Ok(())
        }

        #[pallet::call_index(4)]
        #[pallet::weight(T::DbWeight::get().reads_writes(1, 1))]
        pub fn set_block_reward_shares(
            origin: OriginFor<T>,
            miner: Permill,
            storage: Permill,
            reaction: Permill,
        ) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            let sum = (miner.deconstruct() as u64)
                + (storage.deconstruct() as u64)
                + (reaction.deconstruct() as u64);
            ensure!(sum <= 1_000_000, Error::<T>::SharesSumExceedsHundred);
            MinerSharePermill::<T>::put(miner);
            StorageSharePermill::<T>::put(storage);
            ReactionSharePermill::<T>::put(reaction);
            Self::deposit_event(Event::ParameterUpdated {
                key: ParameterKey::MinerShare,
                old_value: None,
                new_value: miner.deconstruct() as u128,
            });
            Ok(())
        }

        #[pallet::call_index(5)]
        #[pallet::weight(T::DbWeight::get().reads_writes(1, 1))]
        pub fn set_reactor_lock_min(origin: OriginFor<T>, new: u128) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            let old = ReactorLockMin::<T>::get();
            ReactorLockMin::<T>::put(new);
            Self::deposit_event(Event::ParameterUpdated {
                key: ParameterKey::ReactorLockMin,
                old_value: old,
                new_value: new,
            });
            Ok(())
        }

        #[pallet::call_index(6)]
        #[pallet::weight(T::DbWeight::get().reads_writes(1, 1))]
        pub fn set_bond_per_gb(origin: OriginFor<T>, new: u128) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            let old = BondPerGB::<T>::get();
            BondPerGB::<T>::put(new);
            Self::deposit_event(Event::ParameterUpdated {
                key: ParameterKey::BondPerGB,
                old_value: old,
                new_value: new,
            });
            Ok(())
        }

        #[pallet::call_index(7)]
        #[pallet::weight(T::DbWeight::get().reads_writes(1, 1))]
        pub fn set_slash_rate_per_fail_ppm(origin: OriginFor<T>, new: u32) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            let old = SlashRatePerFailPpm::<T>::get().map(|v| v as u128);
            SlashRatePerFailPpm::<T>::put(new);
            Self::deposit_event(Event::ParameterUpdated {
                key: ParameterKey::SlashRatePerFailPpm,
                old_value: old,
                new_value: new as u128,
            });
            Ok(())
        }

        #[pallet::call_index(8)]
        #[pallet::weight(T::DbWeight::get().reads_writes(1, 1))]
        pub fn set_base_fee_range(
            origin: OriginFor<T>,
            min: u128,
            max: u128,
        ) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            ensure!(min <= max, Error::<T>::InvertedBaseFeeRange);
            BaseFeeMin::<T>::put(min);
            BaseFeeMax::<T>::put(max);
            Self::deposit_event(Event::ParameterUpdated {
                key: ParameterKey::BaseFeeMin,
                old_value: None,
                new_value: min,
            });
            Ok(())
        }
    }

    // ─── Public getters with default fallback ──────────────────────────────
    // 既存 pallet が `Get<Permill>` で受け取っている部分にここから拾わせる。
    // runtime adapter で `Get<Permill>` impl にして接続する。

    impl<T: Config> Pallet<T> {
        pub fn effective_post_storage_share() -> Permill {
            PostStorageSharePermill::<T>::get()
                .unwrap_or_else(|| T::DefaultPostStorageSharePermill::get())
        }
        pub fn effective_post_reaction_share() -> Permill {
            PostReactionSharePermill::<T>::get()
                .unwrap_or_else(|| T::DefaultPostReactionSharePermill::get())
        }
        pub fn effective_dm_storage_share() -> Permill {
            DmStorageSharePermill::<T>::get()
                .unwrap_or_else(|| T::DefaultDmStorageSharePermill::get())
        }
        pub fn effective_dm_stealth_share() -> Permill {
            DmStealthSharePermill::<T>::get()
                .unwrap_or_else(|| T::DefaultDmStealthSharePermill::get())
        }
        pub fn effective_miner_share() -> Permill {
            MinerSharePermill::<T>::get()
                .unwrap_or_else(|| T::DefaultMinerSharePermill::get())
        }
        pub fn effective_storage_share() -> Permill {
            StorageSharePermill::<T>::get()
                .unwrap_or_else(|| T::DefaultStorageSharePermill::get())
        }
        pub fn effective_reaction_share() -> Permill {
            ReactionSharePermill::<T>::get()
                .unwrap_or_else(|| T::DefaultReactionSharePermill::get())
        }
        pub fn effective_reactor_lock_min() -> u128 {
            ReactorLockMin::<T>::get().unwrap_or_else(|| T::DefaultReactorLockMin::get())
        }
        pub fn effective_bond_per_gb() -> u128 {
            BondPerGB::<T>::get().unwrap_or_else(|| T::DefaultBondPerGB::get())
        }
        pub fn effective_slash_rate_per_fail_ppm() -> u32 {
            SlashRatePerFailPpm::<T>::get()
                .unwrap_or_else(|| T::DefaultSlashRatePerFailPpm::get())
        }
        pub fn effective_base_fee_min() -> u128 {
            BaseFeeMin::<T>::get().unwrap_or_else(|| T::DefaultBaseFeeMin::get())
        }
        pub fn effective_base_fee_max() -> u128 {
            BaseFeeMax::<T>::get().unwrap_or_else(|| T::DefaultBaseFeeMax::get())
        }
    }
}
