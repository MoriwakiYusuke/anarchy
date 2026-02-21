# Anarchy Blockchain Node 技術仕様書

このドキュメントはAnarchyブロックチェーンノードの技術仕様をコードベースから抽出して記録したものです。

## 目次

1. [概要](#概要)
2. [アーキテクチャ](#アーキテクチャ)
3. [コンセンサス](#コンセンサス)
4. [Runtime構成](#runtime構成)
5. [パレット詳細](#パレット詳細)
6. [RPC API](#rpc-api)
7. [P2Pネットワーク](#p2pネットワーク)
8. [Torプライバシーモード](#torプライバシーモード)

---

## 概要

Anarchyは**匿名分散型SNS**のためのL1ブロックチェーンであり、Polkadot SDK (stable2503) をベースに構築されています。

### 基本情報

| 項目 | 値 |
|------|-----|
| Runtime名 | `anarchy` |
| spec_version | 104 |
| transaction_version | 2 |
| ネイティブトークン | MORAL |
| トークン精度 | 12桁 (1 MORAL = 10^12 planck) |
| SS58 Prefix | 42 (Substrate generic) |
| ブロック時間 | 6秒 (6000ms) |

### プロジェクト構造

```
apps/blockchain/
├── Cargo.toml          # ワークスペース定義
├── rust-toolchain.toml # Rustツールチェーン設定
├── node/               # ノードバイナリ
│   └── src/
│       ├── main.rs         # エントリーポイント
│       ├── cli.rs          # CLI定義
│       ├── command.rs      # コマンド実行
│       ├── chain_spec.rs   # チェーン仕様
│       ├── service.rs      # ノードサービス
│       ├── rpc/            # RPC拡張
│       └── gossip/         # Storage Nodeディスカバリー
├── runtime/            # FRAME Runtime
│   └── src/lib.rs      # パレット構成
└── pallets/            # カスタムパレット
    ├── post/           # 投稿機能
    ├── faucet/         # PoW Faucet
    └── storage/        # 分散ストレージ
```

---

## アーキテクチャ

### システム全体像

```
┌─────────────────────────────────────────────────────────────────┐
│                        Frontend (Next.js)                       │
│                      (Wasm暗号化処理)                           │
└─────────────────────────────────────────────────────────────────┘
                              │
                    WebSocket / HTTP-RPC
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Blockchain Node (Substrate)                  │
│  ┌──────────────┬──────────────┬──────────────┬───────────────┐ │
│  │   Post       │   Faucet     │   Storage    │  System       │ │
│  │   Pallet     │   Pallet     │   Pallet     │  Pallets      │ │
│  └──────────────┴──────────────┴──────────────┴───────────────┘ │
│  ┌──────────────────────────────────────────────────────────────┤
│  │ RPC Layer: System, TransactionPayment, Storage               │
│  └──────────────────────────────────────────────────────────────┤
│  ┌──────────────────────────────────────────────────────────────┤
│  │ P2P Network: libp2p + Storage Node Gossip                    │
│  └──────────────────────────────────────────────────────────────┤
└─────────────────────────────────────────────────────────────────┘
                              │
                    Gossip / HTTP API
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Storage Node (Rust Daemon)                   │
│                   (libp2p P2P + HTTP JSON-RPC)                  │
└─────────────────────────────────────────────────────────────────┘
```

### ノードコンポーネント

1. **CLI (`cli.rs`)**: コマンドライン引数パーサー
2. **Command (`command.rs`)**: 各サブコマンドの実行ロジック
3. **Service (`service.rs`)**: ノードサービス構築、コンセンサス起動
4. **Chain Spec (`chain_spec.rs`)**: Genesis設定、ブートノード定義
5. **RPC (`rpc/`)**: 外部API提供
6. **Gossip (`gossip/`)**: Storage Nodeディスカバリープロトコル

---

## コンセンサス

Anarchyは**Aura + GRANDPA**ハイブリッドコンセンサスを採用しています。

### Aura (Authority Round)

| 設定 | 値 |
|------|-----|
| スロット間隔 | 6000ms |
| 最大オーソリティ数 | 32 |
| 複数ブロック/スロット | 無効 |

ブロック生成はスロットごとに順番にオーソリティが担当します。

### GRANDPA (Ghost-based Recursive Ancestor Deriving Prefix Agreement)

| 設定 | 値 |
|------|-----|
| 最大オーソリティ数 | 32 |
| Gossip間隔 | 333ms |
| Justification生成周期 | 512ブロック |
| Equivocationレポート | 無効 |

**Note**: Equivocation（二重署名）報告は無効化されています。将来的にPoWコンセンサスへの移行を予定しているためです。

### ブロック制限

| 項目 | 値 |
|------|-----|
| 最大ブロックサイズ | 5MB |
| ref_time上限 | 2,000,000,000,000 picoseconds |
| proof_size上限 | 5MB |
| Normalクラス比率 | 75% |

---

## Runtime構成

### 型定義

```rust
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;
pub type Balance = u128;
pub type BlockNumber = u32;
pub type Nonce = u32;
pub type Hash = sp_core::H256;
pub type Signature = MultiSignature;
```

### パレット一覧

```rust
construct_runtime!(
    pub struct Runtime {
        System: frame_system,
        Timestamp: pallet_timestamp,
        Aura: pallet_aura,
        Grandpa: pallet_grandpa,
        Balances: pallet_balances,
        TransactionPayment: pallet_transaction_payment,
        Sudo: pallet_sudo,
        Storage: pallet_storage,
        Post: pallet_post,
        Faucet: pallet_faucet,
    }
);
```

### トランザクション手数料

**手数料は0に設定されています。** スパム対策は投稿時の$moral消費で行います。

```rust
type WeightToFee = ConstantMultiplier<Balance, ConstU128<0>>;
type LengthToFee = ConstantMultiplier<Balance, ConstU128<0>>;
```

### SignedExtra

```rust
pub type SignedExtra = (
    frame_system::CheckNonZeroSender<Runtime>,
    frame_system::CheckSpecVersion<Runtime>,
    frame_system::CheckTxVersion<Runtime>,
    frame_system::CheckGenesis<Runtime>,
    frame_system::CheckEra<Runtime>,
    frame_system::CheckNonce<Runtime>,
    frame_system::CheckWeight<Runtime>,
    // ChargeTransactionPaymentは削除 - TX手数料は無料
);
```

---

## パレット詳細

### Post Pallet (`pallet-post`)

投稿機能を提供するパレット。SSS分割された投稿コンテンツのオンチェーン参照を管理します。

#### ストレージ

| 名前 | キー | 値 | 説明 |
|------|-----|-----|------|
| `NextPostId` | - | `u64` | 次の投稿ID |
| `Posts` | `post_id: u64` | `Post<T>` | 投稿メタデータ |
| `ContentRefs` | `post_id: u64` | `PostContent` | オフチェーン参照 |
| `MerkleRootToPostId` | `[u8; 32]` | `u64` | 逆引きマップ |
| `UserPosts` | `AccountId` | `BoundedVec<u64, 1000>` | ユーザー投稿一覧 |

#### 構造体

```rust
pub struct Post<T: Config> {
    pub author: T::AccountId,
    pub content_hash: [u8; 32],
    pub created_at: BlockNumberFor<T>,
    pub parent_id: Option<u64>,
}

pub struct PostContent {
    pub root: [u8; 32],     // MerkleRoot
    pub k: u32,             // 復元閾値
    pub n: u32,             // 総断片数
    pub size: u64,          // 元データサイズ
}
```

#### Extrinsic

##### `create_post`

```rust
fn create_post(
    origin: OriginFor<T>,
    merkle_root: [u8; 32],
    k: u32,
    n: u32,
    total_size: u64,
    parent_id: Option<u64>,
) -> DispatchResult
```

| パラメータ | 説明 |
|-----------|------|
| `merkle_root` | 断片ハッシュのMerkleRoot（投稿の一意識別子） |
| `k` | 復元に必要な最小断片数（SSS threshold） |
| `n` | 総断片数 (1-255) |
| `total_size` | 元データサイズ（バイト） |
| `parent_id` | 親投稿ID（リプライの場合） |

**コスト計算**:
```
total_cost = PostBaseCost + (total_size × PostByteCost) + deposit
deposit = (base_cost + size_cost) / 5
```

| 定数 | 値 |
|------|-----|
| PostBaseCost | 10 MORAL |
| PostByteCost | 0.1 MORAL/byte |
| MaxContentLength | 10,000 bytes |

---

### Faucet Pallet (`pallet-faucet`)

PoW Faucetによる匿名アカウント初期化を提供します。

#### コンセプト

- **1アカウント1回制限**: Sybil攻撃防止
- **動的難易度調整**: 総claim数に応じて難易度上昇
- **IPログなし**: 匿名性保持

#### ストレージ

| 名前 | キー | 値 | 説明 |
|------|-----|-----|------|
| `FaucetClaims` | `AccountId` | `FaucetClaimRecord<T>` | Claim記録 |
| `TotalClaims` | - | `u64` | 累計Claim数 |

#### 設定定数

| 定数 | 値 | 説明 |
|------|-----|------|
| BaseDifficulty | 18 bits | 初期難易度（約3秒） |
| MaxDifficulty | 28 bits | 難易度上限（約3分） |
| DifficultyScalingFactor | 1000 | 1000アカウントごとに+1 bit |
| RewardAmount | 100 MORAL | 報酬量 |
| ChallengeValidity | 100 blocks | チャレンジ有効期限 |

#### Extrinsic

##### `claim` (Unsigned)

```rust
fn claim(
    origin: OriginFor<T>,
    account: T::AccountId,
    block_number: BlockNumberFor<T>,
    nonce: u64,
) -> DispatchResult
```

**PoW検証**: 
```
challenge = blake2_256(block_hash || encode(account_id))
hash = blake2_256(challenge || nonce.to_le_bytes())
valid = leading_zeros(hash) >= difficulty
```

**難易度計算**:
```
difficulty = min(base + floor(log2(1 + total_claims / scaling_factor)), max)
```

---

### Storage Pallet (`pallet-storage`)

分散ストレージノードの管理とKZG証明ベースの報酬システムを提供します。

#### ストレージ

##### 基本ストレージ

| 名前 | キー | 値 | 説明 |
|------|-----|-----|------|
| `Fragments` | `FragmentId` | `FragmentMetadata<T>` | 断片メタデータ |
| `StorageNodes` | `PeerId` | `StorageNodeInfo<T>` | ノード情報 |
| `OperatorNodes` | `AccountId` | `PeerId` | 運営者→PeerIDマップ |
| `FragmentHolders` | `FragmentId` | `Vec<PeerId>` | 断片保持者一覧 |
| `NodeHoldings` | `PeerId` | `Vec<FragmentId>` | ノード保持断片一覧 |

##### KZG証明関連ストレージ

| 名前 | キー | 値 | 説明 |
|------|-----|-----|------|
| `KzgFragments` | `ContentHash` | `KzgFragment<T>` | KZG断片メタデータ |
| `PendingChallenges` | `(ContentHash, share_index)` | `Challenge<T>` | 保留中チャレンジ |
| `ProofRecords` | `(ContentHash, AccountId)` | `ProofRecord` | 証明履歴 |
| `RewardPoolBalance` | - | `u128` | 報酬プール残高 |
| `PendingRewards` | `AccountId` | `u128` | 未請求報酬 |
| `ScoreCache` | `ContentHash` | `u64` | スコアキャッシュ |
| `ForgettingCandidates` | `ContentHash` | `BlockNumber` | 忘却候補 |

##### レート制限ストレージ

| 名前 | キー | 値 | 説明 |
|------|-----|-----|------|
| `RegistrationCountPerBlock` | `BlockNumber` | `u32` | ブロックあたり登録数 |
| `DeclareHoldingCountPerBlock` | `(BlockNumber, PeerId)` | `u32` | ブロック・ノードあたり宣言数 |

#### 設定定数

| 定数 | 値 | 説明 |
|------|-----|------|
| MaxFragmentSize | 1MB | 断片最大サイズ |
| MaxPeerIdLen | 64 bytes | PeerID最大長 |
| MinPeerIdLen | 38 bytes | PeerID最小長 |
| MaxHoldersPerFragment | 100 | 断片あたり最大保持者数 |
| MaxFragmentsPerNode | 10,000 | ノードあたり最大断片数 |
| MaxRegistrationsPerBlock | 5 | ブロックあたり最大登録数 |
| MaxDeclarationsPerBlockPerNode | 10 | ブロック・ノードあたり最大宣言数 |
| MinNodeCapacity | 1GB | ノード最小容量 |
| BasePowDifficulty | 12 bits | PoW基本難易度 |
| PowObservationPeriod | 10 blocks | PoW観測期間 |
| MaxHttpUrlLen | 256 bytes | HTTP URL最大長 |
| BaseRewardPerByte | 1 (1e-12 MORAL/byte) | バイトあたり報酬 |
| ScoreThreshold | 100 | 報酬対象スコア閾値 |
| ScoreHysteresisMargin | 20 | ヒステリシスマージン |

#### Extrinsics

| call_index | 名前 | 説明 |
|------------|------|------|
| 0 | `register_fragment` | 断片登録 |
| 1 | `register_node` | ノード登録（PoW必須） |
| 2 | `update_node` | ノード容量更新 |
| 3 | `unregister_node` | ノード登録解除 |
| 4 | `declare_holding` | 断片保持宣言 |
| 5 | `revoke_holding` | 保持宣言取り消し |
| 6 | `register_kzg_fragment` | KZG断片登録 |
| 7 | `prove_holding_kzg` | KZG保持証明提出 |
| 8 | `issue_challenge` | チャレンジ発行 |
| 9 | `claim_reward` | 報酬請求 |

#### ノード登録 (`register_node`)

```rust
fn register_node(
    origin: OriginFor<T>,
    peer_id: BoundedVec<u8, MaxPeerIdLen>,
    capacity: u64,
    pow_nonce: u64,
    http_url: BoundedVec<u8, MaxHttpUrlLen>,
) -> DispatchResult
```

**セキュリティ検証**:
1. レート制限チェック (FR-410)
2. PeerID長検証 (FR-405)
3. 容量検証 (FR-411)
4. PoW検証 (FR-409)
5. 重複チェック

#### KZG保持証明 (`prove_holding_kzg`)

```rust
fn prove_holding_kzg(
    origin: OriginFor<T>,
    content_hash: ContentHash,
    share_index: u8,
    share_value: BoundedVec<u8, 32>,
    proof: BoundedVec<u8, 48>,
) -> DispatchResult
```

**報酬計算**:
```rust
reward = if score < threshold {
    0
} else {
    data_size * base_reward_per_byte
}
```

**スコア回復条件** (ヒステリシス):
```rust
recovery_threshold = score_threshold + hysteresis_margin  // 100 + 20 = 120
```

---

## RPC API

### System RPC

標準のSubstrate System RPCを提供。

### TransactionPayment RPC

標準のTransaction Payment RPCを提供。

### Storage RPC

Storage Node関連のカスタムRPCを提供。

#### `storage_registerEndpoint`

Storage Nodeをチェーンノードに登録します。

```json
{
  "method": "storage_registerEndpoint",
  "params": {
    "endpoint": "http://127.0.0.1:3030"
  }
}
```

#### `storage_listNodes`

登録済みStorage Node一覧を取得します（ランダム順）。

```json
{
  "method": "storage_listNodes"
}
```

#### `storage_uploadFragment`

断片をStorage Nodeにアップロードします。

```json
{
  "method": "storage_uploadFragment",
  "params": {
    "merkle_root": "[32-byte hex]",
    "index": 0,
    "data": "[base64]",
    "proof": "[base64]",
    "total_leaves": 5
  }
}
```

#### `storage_getFragment`

断片をStorage Nodeから取得します。

### セキュリティ制限

| 制限 | 値 |
|------|-----|
| MAX_FRAGMENT_SIZE | 256KB |
| MAX_TOTAL_LEAVES | 255 |
| MAX_PROOF_SIZE | 8KB |

---

## P2Pネットワーク

### libp2pプロトコル

Substrateの標準libp2pスタックを使用。

### Storage Node Gossipプロトコル

Storage Node情報をチェーンノード間で共有するカスタムプロトコル。

**プロトコル名**: `/anarchy/storage-nodes/1`

**メッセージタイプ**:

```rust
pub enum StorageNodeGossipMessage {
    NodeRegistered {
        endpoint: String,
        registered_at: u64,
        latency_ms: Option<u64>,
    },
    SyncRequest,
    SyncResponse {
        nodes: Vec<GossipNodeInfo>,
    },
}
```

**設定**:
| 項目 | 値 |
|------|-----|
| 最大通知サイズ | 64KB |
| 受信ピア数 | 25 |
| 送信ピア数 | 25 |

---

## Torプライバシーモード

### 概要

Anarchyはネットワーク匿名性のためにTor統合をサポートしています。

### Torモード

| モード | 説明 |
|--------|------|
| `off` | Torなし（開発用） |
| `outbound-only` | 送信のみTor経由（警告: 受信IPは露出） |
| `forced` | 完全匿名（Tor必須） |

### 起動方法

```bash
# 開発モード (Torなし)
./anarchy-node --dev --tor-mode=off

# 完全匿名モード (本番)
./scripts/anarchy-tor.sh ./anarchy-node --tor-mode=forced
```

### Forcedモードの制約

1. **Outbound Lock**: torsocks環境必須
2. **Inbound Lock**: `127.0.0.1:30333`でのみリッスン
3. **Mainnet強制**: mainnetチェーンは自動的にforcedモードに変更

### Onion v3アドレス形式

```
/onion3/<56-char-base32>:<port>/p2p/<peer-id>
```

**例**:
```
/onion3/vww6ybal4bd7szmgncyruucpgfkqahzddi37ktceo3ah7ngmcopnpyyd:30333/p2p/12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp
```

---

## Genesis設定

### 開発設定 (`dev`)

| 項目 | 値 |
|------|-----|
| オーソリティ | Alice |
| Sudoアカウント | Alice |
| 事前資金アカウント | Alice, Bob, Charlie, Dave, Eve, Ferdie, Alice//stash, Bob//stash |
| 初期残高 | 10,000 MORAL/アカウント |
| 報酬プール | 1,000,000 MORAL |

### ローカルテストネット設定 (`local_testnet`)

| 項目 | 値 |
|------|-----|
| オーソリティ | Alice, Bob |
| Sudoアカウント | Alice |
| 事前資金アカウント | 同上 |

---

## Runtime API

### PostApi

```rust
pub trait PostApi {
    /// MerkleRootからコンテンツ参照を取得
    fn get_content_by_merkle_root(merkle_root: [u8; 32]) -> Option<PostContentInfo>;
    
    /// PostIDからコンテンツ参照を取得
    fn get_content_by_post_id(post_id: u64) -> Option<PostContentInfo>;
}
```

### StorageApi

```rust
pub trait StorageApi {
    /// 全Storage Node情報を取得
    fn get_all_storage_nodes() -> Vec<StorageNodeInfoRpc>;
    
    /// KZG断片情報を取得
    fn get_kzg_fragment(content_hash: ContentHash) -> Option<KzgFragmentInfoRpc>;
}
```

---

## 依存関係

### Polkadot SDK stable2503

主要な依存関係:

| クレート | バージョン |
|---------|-----------|
| frame-support | 45.1.0 |
| frame-system | 45.0.0 |
| sp-core | 39.0.0 |
| sp-runtime | 45.0.0 |
| pallet-balances | 46.0.0 |
| sc-cli | 0.57.0 |
| sc-service | 0.56.0 |

### 暗号ライブラリ

| クレート | 用途 |
|---------|------|
| p256 | WebAuthn ECDSA検証 |
| sha2 | ハッシュ計算 |
| parity-scale-codec | SCALE エンコーディング |

---

## 注意事項

### PAPI必須

Polkadot SDK stable2503はmetadata v16を使用するため、レガシー`@polkadot/api`は使用不可。`polkadot-api` (PAPI)を使用してください。

```typescript
import { createClient } from 'polkadot-api'
import { getWsProvider } from 'polkadot-api/ws-provider/node'
const client = createClient(getWsProvider('ws://127.0.0.1:9944'))
const api = client.getUnsafeApi()
```

### セキュリティ原則

1. **ネットワーク匿名性**: Tor/I2P強制（メタデータ漏洩防止）
2. **生秘密鍵禁止**: WebAuthn + Account Abstraction
3. **クライアントサイド暗号**: 暗号化・SSS分割はクライアント側で実行
4. **Foreground PoWのみ**: Page Visibility API制御

---

## 更新履歴

| バージョン | 日付 | 変更内容 |
|-----------|------|----------|
| 104 | 2026-02 | $moral = native token化, pallet_moral削除 |
