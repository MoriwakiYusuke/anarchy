# Contract: `pallet-messaging` Extrinsics & Runtime API

**Feature**: 019-direct-messages
**Target crate**: `apps/blockchain/pallets/messaging`
**Phase**: 1 (Design)

`pallet-messaging` がオンチェーンに公開する唯一のインターフェース。SCALE 形式は必ず `types.rs` の `#[derive(Encode, Decode)]` で生成されたバイト表現と一致させること。

---

## E1. `publish_dm_key`

**Purpose**: 呼び出し元アカウントが DM を受信するためのメタアドレスを公開・更新する。

```rust
#[pallet::call_index(0)]
#[pallet::weight(T::WeightInfo::publish_dm_key())]
pub fn publish_dm_key(
    origin: OriginFor<T>,
    meta_address: DmMetaAddress,
) -> DispatchResult
```

**Preconditions**:
- `origin` は signed (メインアカウント)。
- `meta_address.scan_pub` と `meta_address.spend_pub` がいずれも非ゼロ。

**Postconditions**:
- `DmReceptionKeys::<T>::insert(caller, meta_address)`。
- Event `DmKeyPublished { account }` を発火。

**Errors**:
- `Error::InvalidMetaAddress`: scan_pub または spend_pub がゼロ。

**Test acceptance (TDD)**:
1. `meta_address` が有効 → `DmReceptionKeys::<T>::get(caller)` が一致、イベント発火。
2. 既存公開鍵がある状態で再呼出 → 上書き、イベント発火。
3. `scan_pub = [0u8; 32]` → `InvalidMetaAddress` で失敗、storage 変更なし。

---

## E2. `revoke_dm_key`

**Purpose**: 呼び出し元アカウントの DM メタアドレスを取り消す。以降、送信者クライアントはこの相手に DM を暗号化できない (`ReceptionKeyNotPublished` で弾かれる)。

```rust
#[pallet::call_index(1)]
#[pallet::weight(T::WeightInfo::revoke_dm_key())]
pub fn revoke_dm_key(origin: OriginFor<T>) -> DispatchResult
```

**Preconditions**:
- `origin` は signed。
- `DmReceptionKeys::<T>::contains_key(caller)` が真。

**Postconditions**:
- `DmReceptionKeys::<T>::remove(caller)`。
- Event `DmKeyRevoked { account }` を発火。

**Errors**:
- `Error::ReceptionKeyNotPublished`: 呼び出し元がまだ鍵を公開していない。

**Test acceptance**:
1. 公開済み → 削除成功、イベント発火、`contains_key` が偽。
2. 未公開で呼出 → `ReceptionKeyNotPublished` で失敗、storage 変更なし。

---

## E3. `send_dm`

**Purpose**: 送信者ステルスアカウントから DM のコンテンツ参照をチェーンに書き込む。

```rust
#[pallet::call_index(2)]
#[pallet::weight(T::WeightInfo::send_dm(*ciphertext_len as u32))]
pub fn send_dm(
    origin: OriginFor<T>,
    recipient_stealth: T::AccountId,
    ephemeral_pubkey: [u8; 32],
    merkle_root: [u8; 32],
    k: u32,
    n: u32,
    ciphertext_len: u64,
) -> DispatchResult
```

**M2 対応 (protocol_version 引数の削除)**: 将来のプロトコル拡張は新 extrinsic (`call_index = 3, 4, ...`) で対応する (Interface Stability 節参照) ため、現行 `send_dm` は常に v1 を表す。引数で version を持つと runtime API / events / weight の全てでバージョン分岐が必要になり保守コストが上がる。代わりに `protocol_version: u8 = 1` を `DmContentRef` 内の定数として扱い、必要なら v2 導入時に新 extrinsic `send_dm_v2` を作る。

**Preconditions**:
- `origin` は signed (想定: 送信者 stealth account。ただし pallet 側で「stealth account かどうか」を識別する手段は持たない — これは観測不可能であるべきプロパティに合致)。
- `k > 0 && k <= n && n <= 255`。
- `ciphertext_len ∈ {1_024, 4_096, 16_384, 65_536, 262_144}` (R4 で 256B は不採用)。
- `ephemeral_pubkey != [0u8; 32]`。
- `!DmMessagesByRoot::<T>::contains_key(merkle_root)`（重複送信防止）。
- 呼び出し元アカウントの残高が `DmBaseCost + DmByteCost * ciphertext_len` 以上。
- 当該ブロックの `DmDispatchesByBlock` エントリ数が `MaxDispatchesPerBlock` 未満。

**Postconditions (副作用順序)**:
1. コスト = `DmBaseCost + DmByteCost * ciphertext_len` を呼び出し元 (sender_stealth) から徴収。内部的には `Currency::withdraw(..., ExistenceRequirement::AllowDeath)` で一括引き落とし → 以下のルールで pool 流入と burn に振り分ける。
2. コストの 80% を `T::StoragePool` へ流入、10% を StealthReward pool へ還流 (R3 / FR-005)、10% を永久 burn (`NegativeImbalance::drop`)。
3. `NextMessageId` をインクリメント。
4. `DmDispatch` を `DmDispatchesByBlock::<T>::append(current_block, ...)`。
5. `DmMessagesByRoot::<T>::insert(merkle_root, message_id)` (重複防止インデックス)。
6. Event `DmDispatched { message_id, block_number, recipient_stealth, ephemeral_pubkey, content_hash: merkle_root }` を発火。

**Errors**:
- `Error::InvalidKNParameters`
- `Error::InvalidPaddingBucket`
- `Error::InvalidMetaAddress` (ephemeral が all-zero)
- `Error::DuplicateContent`
- `Error::TooManyDispatchesInBlock`
- `Error::InsufficientStealthBalance`

**Test acceptance**:
1. 正常系: 全バケット値 (1K, 4K, 16K, 64K, 256K) それぞれで成功、対応する `DmDispatched` イベントとバーン額・pool 残高を検証。
2. `k=0` や `k>n` → `InvalidKNParameters`。
3. `ciphertext_len = 500` (バケット外) → `InvalidPaddingBucket`。
4. 同じ `merkle_root` で 2 回目呼出 → 2 回目は `DuplicateContent`。
5. 同一ブロックで 256 件 + 1 件目 → 257 件目が `TooManyDispatchesInBlock`。
6. 残高不足 → `InsufficientStealthBalance`, pool 残高は不変。
7. `origin = Unsigned` → `BadOrigin`。
8. (将来の protocol_version 分岐は本 extrinsic では行わず、新 extrinsic `send_dm_v2` 追加時に当該 extrinsic のテストで検証する。M2 参照。)

---

## RA. Runtime API: `DmScanApi`

**Purpose**: フロントエンドスキャナが効率的に `DmDispatchesByBlock` を取得するためのランタイム API（ブロックごとの直接アクセスを避けるため）。

```rust
sp_api::decl_runtime_apis! {
    pub trait DmScanApi {
        /// 指定ブロックの DM 発行エントリを取得
        fn dispatches_at(block_number: BlockNumber) -> Vec<DmDispatch<AccountId>>;

        /// 指定アカウントの DM メタアドレスを取得
        fn reception_key(account: AccountId) -> Option<DmMetaAddress>;

        /// pagination 用: from_block..=to_block の dispatches を一括取得 (上限あり)
        fn dispatches_range(
            from_block: BlockNumber,
            to_block: BlockNumber,
        ) -> Vec<(BlockNumber, Vec<DmDispatch<AccountId>>)>;
    }
}
```

**Constraints**:
- `dispatches_range` は `to_block - from_block <= 1_024` を強制 (過剰スキャン防止)。範囲外は空配列を返す。

**Test acceptance**:
1. `dispatches_at` 未使用ブロック → 空 Vec。
2. `dispatches_at` 3 件発行後 → 3 件取得、順序保持。
3. `reception_key` 公開済 → `Some(_)`、未公開 → `None`。
4. `dispatches_range` 境界 (2000 ブロック指定) → 空 Vec を返す (過剰スキャン防止が働く)。

---

## Interface Stability

- `protocol_version` フィールドを保つ限り、将来新しい暗号スイートを入れる余地がある。
- 既存 extrinsic のシグネチャは MVP 後も破壊的変更しない方針。新機能は新 extrinsic を `call_index` 3, 4, ... に追加する。

---

## Dependencies (Runtime 側)

`construct_runtime!` への追加例:

```rust
Messaging: pallet_messaging,
```

```rust
impl pallet_messaging::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    /// 既存 pallet-stealth / pallet-post と同じ `Currency` トレイト経由でトークンを扱う。
    /// fungible API への移行はプロジェクト全体で検討済みでないため、本 pallet も
    /// 既存パターン (`frame_support::traits::Currency`) を踏襲する。
    type Currency = Balances;
    type StoragePool = Storage;  // pallet-storage の reward pool インターフェース
    type MaxDispatchesPerBlock = ConstU32<256>;
    type DmBaseCost = ConstU128<{ 1 * MORAL }>;            // 1 MORAL
    type DmByteCost = ConstU128<{ 50_000_000_000 }>;       // 0.05 MORAL / byte (12 decimals)
    type MaxDmCiphertextLen = ConstU64<262_144>;
    type BurnRatio = BurnRatio;
    type StealthRewardRatio = StealthRewardRatio;
    type WeightInfo = pallet_messaging::weights::SubstrateWeight<Runtime>;
}
```

`Config` トレイト側の型シグネチャも対応させる:

```rust
#[pallet::config]
pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
    type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    /// Currency trait (既存 pallet-stealth と同様)。
    type Currency: frame_support::traits::Currency<Self::AccountId>
        + frame_support::traits::fungible::Mutate<Self::AccountId>;
    type StoragePool: StorageInterface<Self::AccountId, BalanceOf<Self>>;

    #[pallet::constant] type MaxDispatchesPerBlock: Get<u32>;
    #[pallet::constant] type DmBaseCost: Get<BalanceOf<Self>>;
    #[pallet::constant] type DmByteCost: Get<BalanceOf<Self>>;
    #[pallet::constant] type MaxDmCiphertextLen: Get<u64>;
    #[pallet::constant] type BurnRatio: Get<Permill>;
    #[pallet::constant] type StealthRewardRatio: Get<Permill>;

    type WeightInfo: WeightInfo;
}

pub type BalanceOf<T> = <<T as Config>::Currency as
    frame_support::traits::Currency<<T as frame_system::Config>::AccountId>>::Balance;
```

**注**: `Currency` トレイトは古いインターフェースだが、既存 `pallet-stealth`/`pallet-post`/`pallet-storage`/`pallet-reaction` が全て `Currency` で統一されているため整合を優先する。将来プロジェクト全体で `fungible::Mutate` へ移行するタイミングで同時に切り替える。

(MORAL = 10^12 定数は既存のものを再利用)
