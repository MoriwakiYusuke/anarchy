---
name: backend-patterns
description: Anarchy L1 のバックエンド = Substrate pallet (FRAME) + Rust storage node の設計・実装パターン。新規 pallet 追加、extrinsic 設計、Storage/Event/Error 定義、tight coupling (pallet_balances / pallet_storage / pallet_reaction との連携)、Runtime API 宣言、weight/benchmarking、runtime 合成時に使用する。
---

# Backend Patterns — Anarchy Substrate Pallets

Anarchy の「バックエンド」は Substrate ベースの L1 blockchain と、それを支える off-chain Rust storage node。**従来型 Web サーバ (Express / Supabase / Prisma) は一切存在しない**。このスキルは FRAME pallet 実装の骨格と、Anarchy 固有の tight coupling / trait 分離パターンを扱う。

## Pallet ディレクトリ規約

```
apps/blockchain/pallets/<name>/
├── Cargo.toml
└── src/
    ├── lib.rs             # #[frame_support::pallet] 本体 + Config/Storage/Event/Error/Call
    ├── types.rs           # Encode/Decode 対象の公開型 (BoundedVec / MaxEncodedLen 必須)
    ├── weights.rs         # WeightInfo trait + 実測 or stub 実装
    ├── mock.rs            # #[cfg(test)] test runtime
    ├── tests.rs           # #[cfg(test)] ユニットテスト
    └── benchmarking.rs    # #[cfg(feature = "runtime-benchmarks")]
```

参考: [pallets/messaging/](apps/blockchain/pallets/messaging/)、[pallets/post/](apps/blockchain/pallets/post/)

## 基本スケルトン

```rust
#![cfg_attr(not(feature = "std"), no_std)]          // no_std 必須 (wasm32v1-none target)

pub use pallet::*;
pub use types::*;
pub use weights::WeightInfo;

mod types;
pub mod weights;

#[cfg(test)] mod mock;
#[cfg(test)] mod tests;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::{pallet_prelude::*, traits::fungible::{Inspect, Mutate}};
    use frame_system::pallet_prelude::*;

    pub type BalanceOf<T> = <<T as Config>::NativeToken as Inspect<
        <T as frame_system::Config>::AccountId,
    >>::Balance;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        type NativeToken: Inspect<Self::AccountId> + Mutate<Self::AccountId>;
        #[pallet::constant] type SomeConst: Get<BalanceOf<Self>>;
        type WeightInfo: WeightInfo;
    }

    #[pallet::storage]
    pub type Items<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, SomeValue>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> { ItemAdded { who: T::AccountId } }

    #[pallet::error]
    pub enum Error<T> { AlreadyExists, Overflow }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::add_item())]
        pub fn add_item(origin: OriginFor<T>, value: SomeValue) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(!Items::<T>::contains_key(&who), Error::<T>::AlreadyExists);
            Items::<T>::insert(&who, value);
            Self::deposit_event(Event::ItemAdded { who });
            Ok(())
        }
    }
}
```

**非妥協ルール**:
- `#![cfg_attr(not(feature = "std"), no_std)]` を省略しない (runtime build が `wasm32v1-none` で落ちる)
- 全カスタム型に `Encode, Decode, TypeInfo, MaxEncodedLen` (Storage に入れる場合) を必ず derive
- `sp_std::vec::Vec` / `BoundedVec` を使う (`std::vec::Vec` は no_std 環境で不可)
- Extrinsic は `call_index` を明示 (後方互換のため番号を入れ替えない)

## Extrinsic 設計パターン

### 1. validate → charge → mutate → emit の順序

[pallets/messaging/src/lib.rs:187-260](apps/blockchain/pallets/messaging/src/lib.rs#L187-L260) の `send_dm` が典型例:

```rust
pub fn send_dm(origin, /* ... */) -> DispatchResult {
    let who = ensure_signed(origin)?;

    // 1. 純粋バリデーション (fail fast、storage read 最小)
    ensure!(k > 0 && k <= n && n <= 255, Error::<T>::InvalidKNParameters);
    ensure!(DM_PADDING_BUCKETS.contains(&ciphertext_len), Error::<T>::InvalidPaddingBucket);

    // 2. Storage read による状態チェック
    ensure!(!DmMessagesByRoot::<T>::contains_key(merkle_root), Error::<T>::DuplicateContent);

    // 3. 料金計算 (overflow チェック必須)
    let cost = T::DmBaseCost::get()
        .checked_add(&byte_cost)
        .ok_or(Error::<T>::CostCalculationOverflow)?;

    // 4. トークン burn / transfer (失敗するなら早めに)
    T::NativeToken::burn_from(&who, cost, /* ... */)?;

    // 5. Storage mutate
    DmMessagesByRoot::<T>::insert(merkle_root, message_id);

    // 6. Event 発火 (最後)
    Self::deposit_event(Event::DmDispatched { /* ... */ });
    Ok(())
}
```

重要: `ensure!` を使うとエラー時に自動で全 storage 変更が revert される。**`ensure!` の後に任意の mutation が許される**。mutate してから ensure! するとキャッシュされた書き込みが残る可能性があるため順序を守る。

### 2. コスト計算は常に `checked_*`

Anarchy は $moral 12 decimals (1_000_000_000_000 units/token)。掛け算がオーバーフローしやすいため:

```rust
let byte_cost = T::DmByteCost::get()
    .checked_mul(&(ciphertext_len as u128).saturated_into())
    .ok_or(Error::<T>::CostCalculationOverflow)?;
```

## tight coupling (pallet 間依存)

Anarchy は複数 pallet が強く結合しているため、trait を pallet 側で宣言 → runtime で具象実装を配線する方式を徹底する。

### パターン A: Config の tight bound (balances に密結合)

pallet-post は `pallet_balances::Config` に tight coupling していない。代わりに `fungible` trait abstraction を使う:

```rust
pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
    type NativeToken: Inspect<Self::AccountId> + Mutate<Self::AccountId>;
}
```

### パターン B: trait を自分で定義して他 pallet に公開

[pallets/storage/](apps/blockchain/pallets/storage/) は `StorageInterface` を公開し、[pallet-post](apps/blockchain/pallets/post/src/lib.rs#L101-L102) がそれを Config 経由で依存する:

```rust
// pallet-storage 側
pub trait StorageInterface<AccountId, BlockNumber> {
    fn do_register_fragment(/* ... */) -> DispatchResult;
    fn do_register_kzg_fragment(/* ... */) -> DispatchResult;
    fn do_deposit_to_reward_pool(amount: u128);
}

// pallet-post 側 (Config で要求)
type Storage: pallet_storage::StorageInterface<Self::AccountId, BlockNumberFor<Self>>;
```

### パターン C: 2 pallet 以上で共有する interface は単一 pallet が所有

- reward pool への流入は `pallet-reaction::ReactionInterface::do_deposit_to_reaction_pool`
- ステルス報酬プールへの流入は `pallet-messaging::StealthRewardInterface::do_deposit_to_stealth_reward_pool`

**新 pallet 追加時のルール**: 他 pallet から呼ばれる API が必要なら、自 pallet 内で trait を定義し `impl for ()` で no-op 実装を一緒に置く (テストや未配線 runtime で動く)。

```rust
pub trait StealthRewardInterface {
    fn do_deposit_to_stealth_reward_pool(amount: u128);
}
impl StealthRewardInterface for () {
    fn do_deposit_to_stealth_reward_pool(_amount: u128) {}
}
```

## Storage 設計

### Map キーハッシャの選び方

| ハッシャ | 用途 |
|---|---|
| `Blake2_128Concat` | ユーザー由来のキー (AccountId, user-submitted hash)。安全かつ enumerable |
| `Twox64Concat` | enumerate 必要で内部生成キー (post_id, message_id) |
| `Identity` | キーが既に hash であることが保証される場合のみ |

**ユーザー由来入力には絶対 `Identity` を使わない** (storage-bloat 攻撃が可能)。

### BoundedVec でサイズ境界を必ず宣言

```rust
#[pallet::storage]
pub type DmDispatchesByBlock<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    BlockNumberFor<T>,
    BoundedVec<DmDispatch<T::AccountId>, T::MaxDispatchesPerBlock>,
    ValueQuery,
>;
```

`Vec` を Storage に直接入れると実行時 weight が予測不能になり benchmarking も通らない。必ず `BoundedVec<_, MaxN>` + `MaxN` は Config constant に。

## Runtime API 宣言

フロントエンド (PAPI) からの効率的クエリのために runtime API を定義する。[pallets/messaging/src/lib.rs:44-59](apps/blockchain/pallets/messaging/src/lib.rs#L44-L59):

```rust
sp_api::decl_runtime_apis! {
    pub trait DmScanApi<AccountId: parity_scale_codec::Codec> {
        fn dispatches_at(block_number: u32) -> sp_std::vec::Vec<DmDispatch<AccountId>>;
        fn reception_key(account: AccountId) -> Option<DmMetaAddress>;
        fn dispatches_range(from_block: u32, to_block: u32)
            -> sp_std::vec::Vec<(u32, sp_std::vec::Vec<DmDispatch<AccountId>>)>;
    }
}
```

Runtime 側の実装は [runtime/src/lib.rs の `impl_runtime_apis!`](apps/blockchain/runtime/src/lib.rs) に追記。**範囲クエリには必ず上限ガードを入れる** (例: `to_block - from_block > 1024` で空返却)、さもないと full-node が DoS される。

## Runtime 合成 (runtime/src/lib.rs)

新規 pallet 追加時:

1. `Cargo.toml` に依存追加 (`default-features = false` + `std` feature に含める)
2. `impl pallet_<name>::Config for Runtime { /* 型配線 */ }` を追記
3. `construct_runtime!` マクロに `<Name>: pallet_<name>` を追加 (pallet index は **絶対に入れ替えない**)
4. runtime API を追加する場合は `impl_runtime_apis!` に追記
5. **`spec_version` を bump** (`runtime_version!` マクロ内)。forget すると新 extrinsic が decode できない

## Benchmarking / Weights

- プレースホルダ (stub) は `weights.rs` に `0.into()` / `Weight::zero()` で置き、`MaxEncodedLen` 影響を受ける extrinsic には最低でも read/write の概算を入れる
- 本番用には `cargo build --release --features=runtime-benchmarks` + `frame-omni-bencher` 実行で実測値を生成
- weight 式は extrinsic 引数ベース: `#[pallet::weight(T::WeightInfo::send_dm(*ciphertext_len as u32))]`

## Storage Node (apps/storage-node/)

別 Cargo workspace。blockchain node とは独立したバイナリ。
- HTTP JSON-RPC on `:3030` (axum)
- libp2p P2P transport (Tor over onion option あり)
- 起動時に blockchain node へ自己登録 (sr25519 keypair で X-Chain-Auth 署名、[apps/blockchain/node/src/rpc/storage.rs](apps/blockchain/node/src/rpc/storage.rs))

Storage node に pallet 概念は無いが、チェーン側の `pallet-storage` が期待するデータ形式 (fragment_id, KZG commitment) に合わせて保存する。

## よくある失敗

| 症状 | 原因 |
|---|---|
| `error[E0425]: cannot find type Vec` | `use sp_std::vec::Vec;` 不足 (no_std 環境で `std::vec::Vec` は使えない) |
| Runtime wasm build が膨張 | `debug_assert!` / `log::debug!` を runtime code に入れている |
| `spec_version` 忘れで古い client が decode 失敗 | 新 extrinsic 追加時は必ず bump |
| `BoundedVec` insertion 失敗 | `try_mutate` + `try_push` を使い、溢れをエラーにマップ |
| tight coupling の循環依存 | 共有 trait は片方の pallet (呼ばれる側) で定義して他方が使う。Config に直接 `pallet_X::Config` を bound しない |

## 参考実装

| やりたいこと | 参照 pallet |
|---|---|
| fungible token 消費 + 報酬プール配分 | [pallet-post](apps/blockchain/pallets/post/src/lib.rs) |
| PoW 検証 + 動的難易度調整 | [pallet-reaction](apps/blockchain/pallets/reaction/) |
| off-chain storage commitment + KZG 証明 | [pallet-storage](apps/blockchain/pallets/storage/) |
| ephemeral key + stealth 導出 | [pallet-stealth](apps/blockchain/pallets/stealth/) |
| Runtime API + 範囲クエリガード | [pallet-messaging](apps/blockchain/pallets/messaging/src/lib.rs#L44-L59) |
| PoW client-side + on-chain validation | [pallet-faucet](apps/blockchain/pallets/faucet/) |
