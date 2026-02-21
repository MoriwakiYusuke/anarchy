# Storage Pallet API Contract

**Pallet**: `pallet-storage`  
**Version**: 2.0.0 (KZG-VSS upgrade)  
**Date**: 2026-02-16

## Extrinsics

### prove_holding_kzg

保持証明を提出し、オンチェーンでKZG検証を実行する。

```rust
#[pallet::call_index(10)]
#[pallet::weight(T::WeightInfo::prove_holding_kzg())]
pub fn prove_holding_kzg(
    origin: OriginFor<T>,
    content_hash: H256,
    share_index: u8,
    share_value: BoundedVec<u8, ConstU32<32>>,
    proof: BoundedVec<u8, ConstU32<48>>,
) -> DispatchResult;
```

**Parameters**:
| Name | Type | Description |
|------|------|-------------|
| `content_hash` | `H256` | 対象コンテンツのハッシュ |
| `share_index` | `u8` | シェアインデックス (1..=n) |
| `share_value` | `[u8; 32]` | シェア値（BLS12-381スカラー） |
| `proof` | `[u8; 48]` | KZG opening proof |

**Success Response**:
- `HoldingProved { content_hash, holder, share_index }` イベント発行
- `ProofRecords` ストレージ更新
- ペンディング報酬を加算

**Error Responses**:
| Error | Code | Description |
|-------|------|-------------|
| `FragmentNotFound` | 1001 | 指定されたcontent_hashが存在しない |
| `InvalidShareIndex` | 1002 | share_indexが範囲外 |
| `InvalidKzgProof` | 1003 | KZG検証に失敗 |
| `ChallengeNotPending` | 1004 | チャレンジが発行されていない |
| `ChallengeExpired` | 1005 | チャレンジ期限切れ |

---

### claim_reward

累積した報酬をクレーム（引き出し）する。

```rust
#[pallet::call_index(11)]
#[pallet::weight(T::WeightInfo::claim_reward())]
pub fn claim_reward(
    origin: OriginFor<T>,
) -> DispatchResult;
```

**Parameters**: なし（呼び出し元アカウントの報酬をクレーム）

**Success Response**:
- `RewardClaimed { holder, amount }` イベント発行
- 呼び出し元の `pending_reward` を0にリセット
- `RewardPoolBalance` から減算
- 呼び出し元のバランスに加算

**Error Responses**:
| Error | Code | Description |
|-------|------|-------------|
| `NoPendingReward` | 1101 | クレーム可能な報酬がない |
| `InsufficientRewardPool` | 1102 | 報酬プール残高不足 |

---

### issue_challenge (Root only)

保持証明のチャレンジを発行する。通常はOff-chain Workerから呼び出し。

```rust
#[pallet::call_index(12)]
#[pallet::weight(T::WeightInfo::issue_challenge())]
pub fn issue_challenge(
    origin: OriginFor<T>,
    content_hash: H256,
    share_index: u8,
    target_node: T::AccountId,
) -> DispatchResult;
```

**Parameters**:
| Name | Type | Description |
|------|------|-------------|
| `content_hash` | `H256` | 対象コンテンツのハッシュ |
| `share_index` | `u8` | チャレンジするシェアインデックス |
| `target_node` | `AccountId` | チャレンジ対象ノード |

**Success Response**:
- `ChallengeIssued { content_hash, share_index, target_node, deadline }` イベント発行
- `PendingChallenges` ストレージに追加

**Error Responses**:
| Error | Code | Description |
|-------|------|-------------|
| `NotAuthorized` | 1201 | Root権限なし |
| `FragmentNotFound` | 1202 | コンテンツが存在しない |
| `NodeNotHolder` | 1203 | 対象ノードがホルダーでない |
| `ChallengeAlreadyPending` | 1204 | 既にチャレンジ発行済み |

---

### register_fragment

新しい断片を登録する（create_post_v2から内部呼び出し）。

```rust
#[pallet::call_index(13)]
#[pallet::weight(T::WeightInfo::register_fragment())]
pub fn register_fragment(
    origin: OriginFor<T>,
    content_hash: H256,
    commitment: BoundedVec<u8, ConstU32<48>>,
    data_size: u32,
    fragment_count: u8,
    threshold: u8,
    holders: BoundedVec<T::AccountId, ConstU32<16>>,
) -> DispatchResult;
```

**Success Response**:
- `FragmentRegistered { content_hash, commitment, holders }` イベント発行
- `Fragments` ストレージに追加
- 投稿費用の90%を `RewardPoolBalance` に加算
- 投稿費用の10%をバーン

---

## Events

```rust
#[pallet::event]
#[pallet::generate_deposit(pub(super) fn deposit_event)]
pub enum Event<T: Config> {
    /// 断片が登録された
    FragmentRegistered {
        content_hash: H256,
        commitment: Vec<u8>,
        holders: Vec<T::AccountId>,
    },
    
    /// 保持証明が成功した
    HoldingProved {
        content_hash: H256,
        holder: T::AccountId,
        share_index: u8,
    },
    
    /// 保持証明が無効だった
    HoldingProofInvalid {
        content_hash: H256,
        holder: T::AccountId,
        share_index: u8,
    },
    
    /// チャレンジが発行された
    ChallengeIssued {
        content_hash: H256,
        share_index: u8,
        target_node: T::AccountId,
        deadline: BlockNumberFor<T>,
    },
    
    /// チャレンジが期限切れになった
    ChallengeExpired {
        content_hash: H256,
        share_index: u8,
        target_node: T::AccountId,
    },
    
    /// 報酬がクレームされた
    RewardClaimed {
        holder: T::AccountId,
        amount: BalanceOf<T>,
    },
    
    /// 報酬プールに資金が追加された
    RewardPoolFunded {
        amount: BalanceOf<T>,
    },
    
    /// スコアが閾値未満になった
    LowScoreDetected {
        content_hash: H256,
        score: u64,
        threshold: u64,
    },
}
```

---

## Storage

```rust
/// 断片メタデータ
#[pallet::storage]
pub type Fragments<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    H256,  // content_hash
    Fragment<T::AccountId, BlockNumberFor<T>>,
>;

/// 保留中のチャレンジ
#[pallet::storage]
pub type PendingChallenges<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    (H256, u8),  // (content_hash, share_index)
    Challenge<T::AccountId, BlockNumberFor<T>>,
>;

/// 保持証明記録
#[pallet::storage]
pub type ProofRecords<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    H256,  // content_hash
    Blake2_128Concat,
    T::AccountId,  // holder
    ProofRecord<BlockNumberFor<T>>,
>;

/// 報酬プール残高
#[pallet::storage]
pub type RewardPoolBalance<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

/// スコアキャッシュ
#[pallet::storage]
pub type ScoreCache<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    H256,
    u64,
>;
```

---

## Config

```rust
#[pallet::config]
pub trait Config: frame_system::Config + pallet_balances::Config {
    type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    
    /// ベース報酬（1バイトあたり）
    #[pallet::constant]
    type BaseRewardPerByte: Get<BalanceOf<Self>>;
    
    /// スコア閾値
    #[pallet::constant]
    type ScoreThreshold: Get<u64>;
    
    /// 報酬プール比率（perbill, e.g., 90%）
    #[pallet::constant]
    type RewardPoolRatio: Get<Perbill>;
    
    /// チャレンジ応答猶予期間（ブロック数）
    #[pallet::constant]
    type ChallengeGracePeriod: Get<BlockNumberFor<Self>>;
    
    /// 警告フラグ閾値（連続失敗回数）
    #[pallet::constant]
    type WarningThreshold: Get<u32>;
    
    /// スコアプロバイダー
    type ScoreProvider: ScoreProvider;
    
    /// 重み情報
    type WeightInfo: WeightInfo;
}
```

---

## Weight Info

```rust
pub trait WeightInfo {
    fn prove_holding_kzg() -> Weight;
    fn claim_reward() -> Weight;
    fn issue_challenge() -> Weight;
    fn register_fragment() -> Weight;
}
```

**Estimated Weights**:
| Extrinsic | Reads | Writes | Compute (ref_time) |
|-----------|-------|--------|-------------------|
| `prove_holding_kzg` | 3 | 2 | 50_000_000 |
| `claim_reward` | 2 | 3 | 10_000_000 |
| `issue_challenge` | 2 | 1 | 5_000_000 |
| `register_fragment` | 1 | 2 | 15_000_000 |
