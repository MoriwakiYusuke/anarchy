# Anarchy 分散ストレージシステム

本ドキュメントでは、Anarchy プロジェクトにおけるコンテンツの保存方法、保存ロジック、および報酬ロジックについて詳述する。

---

## 1. アーキテクチャ概要

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              FRONTEND (Next.js)                             │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │  useStorage.ts + WASM Worker (wasm-engine)                            │  │
│  │  • AES-256-GCM 暗号化                                                  │
│  │  • Reed-Solomon k-of-n エンコード                                      │
│  │  • SSS 鍵分割                                                          │
│  │  • MerkleTree 構築                                                     │
│  └───────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
                         │
                         │ WebSocket RPC (ws://localhost:9944)
                         │ ├─ storage_uploadFragment
                         │ ├─ storage_getFragment  
                         │ └─ post.create_post (Extrinsic)
                         ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                      BLOCKCHAIN NODE (Substrate)                            │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  Storage RPC Extension (apps/blockchain/node/src/rpc/storage.rs)    │    │
│  │  • MerkleProof 検証                                                  │    │
│  │  • StorageNodeRegistry (ノード管理・ラウンドロビン)                    │    │
│  │  • 断片転送: merkle_root ベースでノード選択                           │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  Runtime (pallet-post, pallet-storage)                              │    │
│  │  • PostContent (root, k, n, size)                                   │    │
│  │  • KzgFragment, RewardPool, ProofRecords                            │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────────────────┘
          │                                      │
          │ HTTP転送 (内部)                       │ Challenge/Proof
          ▼                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                        STORAGE NODE (Rust)                                  │
│  ┌────────────────────────────────┐  ┌────────────────────────────────┐    │
│  │  HTTP JSON-RPC (:3030)         │  │  FragmentStore                 │    │
│  │  • storage_storeKzgShard       │  │  • data/fragments/{id}/{i}.bin │    │
│  │  • storage_getKzgShard         │  │  • 最大 256KB/fragment         │    │
│  │  • storage_health              │  │  • AtomicU64 容量管理          │    │
│  └────────────────────────────────┘  └────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────────────────┘
```

**ポイント**: フロントエンドはブロックチェーンノードのRPC経由でのみストレージにアクセスする（直接アクセス禁止）。

---

## 2. 保存方法: KZG-VSS ハイブリッド方式

### 2.1 データフロー（クライアント側）

```
┌──────────────┐
│  原文データ   │
│  (UTF-8 etc) │
└──────┬───────┘
       │ ① 圧縮 (256B以上で自動)
       ▼
┌──────────────┐
│  圧縮データ   │
└──────┬───────┘
       │ ② AES-256-GCM 暗号化
       │   ├─ 鍵: K_post (32バイト乱数)
       │   ├─ Nonce: 12バイト
       │   └─ Tag: 16バイト
       ▼
┌──────────────────────────┐
│  暗号文                   │
│  (元データ + 28バイト)    │
└──────┬───────────────────┘
       │ ③ Reed-Solomon k-of-n エンコード
       │   └─ shardSize = ceil(ciphertextLen / k)
       ▼
┌────────────────────────────────────────┐
│  n個のチャンク (各shardSizeバイト)       │
│  ┌────┐ ┌────┐ ┌────┐ ... ┌────┐      │
│  │ C0 │ │ C1 │ │ C2 │     │Cn-1│      │
│  └────┘ └────┘ └────┘     └────┘      │
└────────────────────────────────────────┘
       │
       │ ④ 各チャンクの Blake2b-256 ハッシュ計算
       ▼
┌────────────────────────────────────────┐
│  n個のチャンクハッシュ (各32バイト)      │
└──────┬─────────────────────────────────┘
       │ ⑤ MerkleTree 構築 (Blake2b)
       ▼
┌─────────────────────┐
│  MerkleRoot (32B)   │ ──► オンチェーンに記録
└─────────────────────┘


┌────────────────────────────────────────┐
│  K_post (暗号化鍵)                      │
└──────┬─────────────────────────────────┘
       │ ⑥ SSS k-of-n 分割 (vsss-rs)
       ▼
┌────────────────────────────────────────┐
│  n個の鍵シェア (各 33-53 バイト)         │
│  ┌─────┐ ┌─────┐ ... ┌─────┐          │
│  │ KS0 │ │ KS1 │     │KSn-1│          │
│  └─────┘ └─────┘     └─────┘          │
└────────────────────────────────────────┘
```

### 2.2 HybridShard の構成

各ストレージノードに保存されるシャードの構造:

```
HybridShard (シリアライズ形式)
┌────────────────────────────────────────────────────────────┐
│ index (4B)         │ シェアのインデックス (0..n-1)          │
├────────────────────────────────────────────────────────────┤
│ chunk_len (4B)     │ RS チャンクの長さ                      │
├────────────────────────────────────────────────────────────┤
│ key_share_idx (1B) │ 鍵シェアのインデックス                 │
├────────────────────────────────────────────────────────────┤
│ chunk_hash (32B)   │ Blake2b-256 ハッシュ                  │
├────────────────────────────────────────────────────────────┤
│ chunk (可変)       │ RS チャンクデータ                      │
├────────────────────────────────────────────────────────────┤
│ key_share_len (4B) │ 鍵シェアの長さ                        │
├────────────────────────────────────────────────────────────┤
│ key_share (可変)   │ SSS 鍵シェアデータ                    │
└────────────────────────────────────────────────────────────┘
```

---

## 3. 保存ロジック

### 3.1 投稿作成フロー

```
┌────────────────────────────────────────────────────────────────────────────┐
│                           クライアント (Frontend)                           │
└────────────────────────────────────────────────────────────────────────────┘
        │
        │ 1. uploadContent(data)
        │    ├─ WASM: hybrid_split(data, k=3, n=5)
        │    └─ 結果: HybridSplitResult { shards[], merkleRoot, metadata }
        ▼
┌────────────────────────────────────────────────────────────────────────────┐
│  2. ブロックチェーンノードRPC経由でシャードをアップロード                   │
│     WebSocket: storage_uploadFragment(merkle_root, index, data, proof)     │
│                                                                            │
│     ※ フロントエンドはストレージノードに直接アクセスしない                   │
└────────────────────────────────────────────────────────────────────────────┘
        │
        ▼
┌────────────────────────────────────────────────────────────────────────────┐
│  3. ブロックチェーンノードが内部でストレージノードに転送                    │
│     ├─ MerkleProof を検証                                                  │
│     ├─ StorageNodeRegistry から適切なノードを選択                          │
│     └─ HTTP POST でストレージノードに転送                                   │
└────────────────────────────────────────────────────────────────────────────┘
        │
        │ 4. 全シャードのアップロード完了
        ▼
┌────────────────────────────────────────────────────────────────────────────┐
│  5. オンチェーン登録                                                       │
│     Extrinsic: post.create_post(merkle_root, k, n, total_size, parent_id)  │
│                                                                            │
│     内部処理:                                                              │
│     ├─ 投稿費用の計算・焼却                                                │
│     │   cost = base_cost + (size × byte_cost) + deposit                   │
│     ├─ Post, PostContent 保存                                             │
│     └─ storage.do_register_fragment(fragment_id, size, creator, block)    │
└────────────────────────────────────────────────────────────────────────────┘
```

### 3.2 ブロックチェーンノードからストレージノードへの転送

```
フロントエンド
      │
      │ WebSocket RPC: storage_uploadFragment(merkle_root, index, data, proof)
      ▼
┌────────────────────────────────────────────────────────────────────────────┐
│                    Blockchain Node (apps/blockchain/node)                  │
├────────────────────────────────────────────────────────────────────────────┤
│  Storage RPC Extension (rpc/storage.rs)                                    │
│                                                                            │
│  1. MerkleProof 検証                                                       │
│     └─ verify_merkle_proof(root, index, data_hash, proof)                 │
│                                                                            │
│  2. ノード選択 (StorageNodeRegistry)                                       │
│     └─ select_node_for_fragment(merkle_root, index)                       │
│     └─ merkle_root × index でPost毎に分散配置（追跡困難）                   │
│                                                                            │
│  3. HTTP転送                                                               │
│     └─ POST http://{node_endpoint}:3030/rpc                               │
│        { method: "storage_storeKzgShard", params: {...} }                  │
└────────────────────────────────────────────────────────────────────────────┘
      │
      │ HTTP POST (内部ネットワーク)
      ▼
┌────────────────────────────────────────────────────────────────────────────┐
│                      Storage Node (apps/storage-node)                       │
├────────────────────────────────────────────────────────────────────────────┤
│  HTTP JSON-RPC Server (:3030)                                              │
│  ├─ storage_storeKzgShard   → FragmentStore.store()                        │
│  ├─ storage_getKzgShard     → FragmentStore.get()                          │
│  └─ storage_health          → ヘルスチェック                                │
├────────────────────────────────────────────────────────────────────────────┤
│  FragmentStore                                                             │
│  ├─ 保存パス: data/fragments/{content_hash_hex}/{index}.bin               │
│  ├─ 最大サイズ: 256KB / fragment                                           │
│  └─ 容量管理: AtomicU64 で使用量追跡                                        │
└────────────────────────────────────────────────────────────────────────────┘
```

### 3.3 復元フロー

```
┌────────────────────────────────────────────────────────────────────────────┐
│  1. オンチェーンから PostContent 取得                                       │
│     { root: [32], k: 3, n: 5, size: 1234 }                                 │
└────────────────────────────────────────────────────────────────────────────┘
        │
        ▼
┌────────────────────────────────────────────────────────────────────────────┐
│  2. ブロックチェーンノードRPC経由でシャードを取得                            │
│     WebSocket: storage_getFragment(merkle_root, index)                     │
│                                                                            │
│     ブロックチェーンノードが内部でストレージノードに問い合わせ               │
│     ※ k個集まれば復元可能（冗長性: n-k 個まで消失許容）                      │
└────────────────────────────────────────────────────────────────────────────┘
        │
        ▼
┌────────────────────────────────────────────────────────────────────────────┐
│  3. WASM: hybrid_recover(shards, k, n, metadata)                           │
│     ├─ 各チャンクの Blake2b ハッシュ検証                                    │
│     ├─ SSS: 鍵シェアから K_post 復元                                        │
│     ├─ RS: k 個のチャンクから暗号文復元                                      │
│     ├─ AES-GCM: K_post で復号                                              │
│     └─ 解凍（必要な場合）                                                   │
└────────────────────────────────────────────────────────────────────────────┘
        │
        ▼
┌────────────────────────────────────────────────────────────────────────────┐
│  4. 元データ取得完了                                                       │
└────────────────────────────────────────────────────────────────────────────┘
```

---

## 4. 報酬ロジック

### 4.1 報酬フローの全体像

```
                              投稿費用 (MORAL)
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────────┐
│                           pallet-post                                      │
│  create_post()                                                             │
│  ├─ cost = base_cost + (size × byte_cost) + deposit                       │
│  └─ NativeToken::burn_from(&who, cost)  ───────► 100% 焼却                 │
└────────────────────────────────────────────────────────────────────────────┘
        │
        │ (将来: 90% を RewardPool へ、10% 焼却)
        ▼
┌────────────────────────────────────────────────────────────────────────────┐
│                   RewardPoolBalance (pallet-storage)                       │
│                           現在: Genesis で初期化                            │
└────────────────────────────────────────────────────────────────────────────┘
        │
        │ prove_holding_kzg() 成功時に報酬計算
        ▼
┌────────────────────────────────────────────────────────────────────────────┐
│                        報酬計算ロジック                                     │
│                                                                            │
│   reward = base_reward_per_byte × data_size  (if score >= threshold)      │
│   reward = 0                                 (if score < threshold)        │
│                                                                            │
│   ※ threshold: 100 (SCORE_THRESHOLD 定数)                                  │
│   ※ hysteresis margin: 50 (回復には 150 以上必要)                           │
└────────────────────────────────────────────────────────────────────────────┘
```

### 4.2 Proof of Holding (KZG 証明)

```
┌────────────────────────────────────────────────────────────────────────────┐
│                        ストレージノード                                     │
│  ① シェアを保持中                                                          │
│  ② チェーンからチャレンジ or 定期的に証明提出                               │
└────────────────────────────────────────────────────────────────────────────┘
        │
        │ prove_holding_kzg(content_hash, share_index, share_value, proof)
        ▼
┌────────────────────────────────────────────────────────────────────────────┐
│                        pallet-storage                                      │
├────────────────────────────────────────────────────────────────────────────┤
│  検証フロー:                                                               │
│  1. KzgFragments から commitment 取得                                      │
│  2. kzg::verify_kzg_proof(commitment, share_index, share_value, proof)    │
│  3. 検証成功 → ProofRecords 更新                                           │
│  4. 報酬計算 → PendingRewards に加算                                        │
└────────────────────────────────────────────────────────────────────────────┘
        │
        │ claim_reward()
        ▼
┌────────────────────────────────────────────────────────────────────────────┐
│  報酬受取:                                                                 │
│  • PendingRewards → ノードオペレーターの残高へ                              │
│  • RewardPoolBalance から減算                                              │
│  • プール不足時: Pro-rata 分配（残りは次回繰り越し）                         │
└────────────────────────────────────────────────────────────────────────────┘
```

### 4.3 スコアシステムと GC ライフサイクル

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    コンテンツスコアに基づく報酬・GC                          │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   Score ≥ 150  ──► 報酬対象 & ForgettingCandidate から回復                  │
│         │                                                                   │
│   150 > Score ≥ 100  ──► 報酬対象（通常状態）                               │
│         │                                                                   │
│   Score < 100  ──► 報酬なし & ForgettingCandidate マーク                    │
│         │                                                                   │
│         ▼                                                                   │
│   一定期間経過 ──► GC 対象（将来実装: 自動削除）                             │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│  ヒステリシス防止:                                                          │
│  • 閾値100で ForgettingCandidate 入り                                      │
│  • 回復には 100 + 50 = 150 以上が必要                                       │
│  • 境界付近での状態の往復を防止                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 4.4 報酬計算式

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           報酬計算の詳細                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  基本式:                                                                    │
│    reward = base_reward_per_byte × data_size                               │
│                                                                             │
│  条件:                                                                      │
│    if (score < SCORE_THRESHOLD) then reward = 0                             │
│                                                                             │
│  パラメータ (Config):                                                       │
│    • BaseRewardPerByte: 1,000,000 (= 0.000001 MORAL / byte)                │
│    • ScoreThreshold: 100                                                    │
│    • ScoreHysteresisMargin: 50                                              │
│                                                                             │
│  例: 1KB のコンテンツ (score = 150)                                         │
│    reward = 1,000,000 × 1024 = 1,024,000,000 単位                          │
│           = 0.001024 MORAL (12 decimals)                                   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 5. オンチェーンデータ構造

### 5.1 pallet-post

```rust
/// 分散ストレージ参照情報
pub struct PostContent {
    pub root: [u8; 32],   // MerkleRoot
    pub k: u32,           // 復元閾値
    pub n: u32,           // 総シェア数
    pub size: u64,        // 元データサイズ（バイト）
}
```

### 5.2 pallet-storage

```rust
/// KZG フラグメント情報
pub struct KzgFragment<T: Config> {
    pub owner: T::AccountId,                        // 所有者
    pub commitment: BoundedVec<u8, ConstU32<48>>,  // KZG コミットメント
    pub data_size: u32,                             // データサイズ
    pub fragment_count: u8,                         // n
    pub threshold: u8,                              // k
    pub created_at: BlockNumberFor<T>,              // 作成ブロック
    pub holders: BoundedVec<T::AccountId, ConstU32<16>>, // ホルダー
}

/// 証明記録
pub struct ProofRecord<BlockNumber> {
    pub last_proved_at: BlockNumber,  // 最後の証明成功ブロック
    pub success_count: u32,           // 連続成功回数
    pub failure_count: u32,           // 連続失敗回数
    pub pending_reward: u128,         // 未請求報酬
}
```

---

## 6. セキュリティ特性

| 特性 | 実現方法 |
|------|----------|
| **機密性** | AES-256-GCM 暗号化（クライアント側で完結） |
| **可用性** | Reed-Solomon k-of-n（n-k 個のノード消失を許容） |
| **鍵管理** | SSS k-of-n 分割（単独ノードでは鍵復元不可） |
| **整合性** | Blake2b ハッシュ + MerkleTree 検証 |
| **証明可能性** | KZG コミットメント + Opening Proof |
| **インセンティブ** | 報酬システム（保持証明に対する MORAL 報酬） |

---

## 8. ストレージノード間通信 (P2P)

### 8.1 通信アーキテクチャ

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     Storage Node A                                          │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │  libp2p Stack                                                          │ │
│  │  ├─ TCP Transport (noise + yamux)                                      │ │
│  │  ├─ Request-Response Protocol (/anarchy/fragment/1.0.0)                │ │
│  │  ├─ Gossipsub (endpoints + storage-nodes)                              │ │
│  │  └─ Identify Protocol                                                  │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────┘
          │                                    │
          │ FragmentRequest/Response           │ Gossipsub Messages
          │ (Binary, JSON-RPC風)               │ (Ed25519署名付き)
          ▼                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                     Storage Node B                                          │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 8.2 プロトコル仕様

#### 8.2.1 Fragment Exchange Protocol

```
プロトコル: /anarchy/fragment/1.0.0
トランスポート: libp2p request-response
タイムアウト: 30秒
最大メッセージサイズ: 10MB
```

```rust
// apps/storage-node/src/network/mod.rs

/// リクエストタイプ
pub enum FragmentRequest {
    /// フラグメント取得
    Get { fragment_id: [u8; 32] },
    /// フラグメント保存（レプリケーション用）
    Put { fragment_id: [u8; 32], data: Vec<u8> },
}

/// レスポンスタイプ
pub enum FragmentResponse {
    /// データ返却（None = 未保持）
    Data(Option<Vec<u8>>),
    /// Put への応答
    Ack { success: bool, error: Option<String> },
}
```

```
メッセージフォーマット:
┌──────────────────────────────────────────┐
│ length (4B, big-endian)                  │
├──────────────────────────────────────────┤
│ JSON payload (可変長)                    │
└──────────────────────────────────────────┘
```

#### 8.2.2 Gossipsub Topics

```
┌────────────────────────────────────────────────────────────────────────────┐
│                         Gossipsub Configuration                            │
├────────────────────────────────────────────────────────────────────────────┤
│  • Heartbeat interval: 10秒                                                │
│  • Validation mode: Strict                                                 │
│  • Message authenticity: Ed25519 署名必須                                   │
│  • Max message size: 4KB                                                   │
└────────────────────────────────────────────────────────────────────────────┘
```

**Topic 1: ブロックチェーンエンドポイント共有**

```
Topic: /anarchy/endpoints/1.0.0
目的: ストレージノード間でブロックチェーンRPCエンドポイントを共有
```

```rust
// apps/storage-node/src/network/gossip.rs

pub struct EndpointMessage {
    /// 既知エンドポイント一覧 (最大20件)
    pub endpoints: Vec<BlockchainEndpoint>,
    /// 送信者のPeerID (Base58)
    pub sender_peer_id: String,
    /// 公開鍵 (Protobuf hex)
    pub sender_public_key: String,
    /// タイムスタンプ (Unix秒)
    pub timestamp: u64,
    /// Ed25519署名 (hex)
    pub signature: String,
}

pub struct BlockchainEndpoint {
    /// WebSocket URL (最大256バイト)
    pub url: String,
    /// チェーンID (genesis hash)
    pub chain_id: [u8; 32],
    /// 最終検証時刻 (Unix秒)
    pub last_verified: u64,
    /// レイテンシ (ms)
    pub latency_ms: u32,
    /// TTL (秒, デフォルト300)
    pub ttl_secs: u32,
}
```

**Topic 2: ストレージノードアドレス共有**

```
Topic: /anarchy/storage-nodes/1.0.0
目的: ストレージノード間で互いのHTTP RPCエンドポイントを共有
```

```rust
pub struct StorageNodeMessage {
    /// 既知ストレージノード一覧 (最大20件)
    pub nodes: Vec<StorageNodeEndpoint>,
    /// 送信者のPeerID (Base58)
    pub sender_peer_id: String,
    /// 公開鍵 (Protobuf hex)
    pub sender_public_key: String,
    /// タイムスタンプ (Unix秒)
    pub timestamp: u64,
    /// Ed25519署名 (hex)
    pub signature: String,
}

pub struct StorageNodeEndpoint {
    /// HTTP RPC URL (例: "http://localhost:3030")
    pub url: String,
    /// 最終検証時刻 (Unix秒)
    pub last_verified: u64,
    /// レイテンシ (ms)
    pub latency_ms: u32,
    /// TTL (秒, デフォルト300)
    pub ttl_secs: u32,
}
```

### 8.3 署名検証フロー

```
┌────────────────────────────────────────────────────────────────────────────┐
│                        メッセージ検証フロー                                 │
├────────────────────────────────────────────────────────────────────────────┤
│                                                                            │
│  1. サイズチェック                                                         │
│     └─ message.len() <= 4096 bytes                                        │
│                                                                            │
│  2. JSON パース                                                            │
│     └─ serde_json::from_slice()                                           │
│                                                                            │
│  3. 公開鍵デコード                                                         │
│     └─ PublicKey::try_decode_protobuf(sender_public_key)                  │
│                                                                            │
│  4. PeerID 検証                                                            │
│     └─ public_key.to_peer_id() == claimed_peer_id                         │
│                                                                            │
│  5. 署名対象データ構築                                                     │
│     └─ sign_data = peer_id || timestamp || Blake2b(endpoints/nodes)       │
│                                                                            │
│  6. Ed25519署名検証                                                        │
│     └─ public_key.verify(sign_data, signature)                            │
│                                                                            │
│  7. タイムスタンプ検証 (スキュー許容: 5分)                                   │
│     └─ now - timestamp <= 300秒                                           │
│                                                                            │
└────────────────────────────────────────────────────────────────────────────┘
```

### 8.4 レピュテーションシステム

```
┌────────────────────────────────────────────────────────────────────────────┐
│                      Peer Reputation System                                │
├────────────────────────────────────────────────────────────────────────────┤
│                                                                            │
│  初期スコア:     100                                                       │
│  有効メッセージ: +1                                                        │
│  無効メッセージ: -20                                                       │
│  無視閾値:       50 (以下のピアからのメッセージは破棄)                       │
│  最大スコア:     100                                                       │
│                                                                            │
├────────────────────────────────────────────────────────────────────────────┤
│                                                                            │
│  Score = 100 ──► Score = 100 (有効: +1, cap)                              │
│       │                                                                    │
│       │ 無効メッセージ受信 × 3回                                           │
│       ▼                                                                    │
│  Score = 40 ──► IGNORED (閾値50以下)                                       │
│       │                                                                    │
│       │ 以降のメッセージは検証せず破棄                                      │
│       ▼                                                                    │
│  (復帰なし: 再起動まで永続)                                                 │
│                                                                            │
└────────────────────────────────────────────────────────────────────────────┘
```

```rust
// apps/storage-node/src/network/reputation.rs

pub struct PeerReputation {
    pub peer_id: PeerId,
    pub score: i32,           // 現在スコア
    pub last_updated: Instant,
    pub invalid_count: u32,   // 無効メッセージ数
    pub valid_count: u32,     // 有効メッセージ数
}
```

### 8.5 エンドポイントキャッシュ

```
┌────────────────────────────────────────────────────────────────────────────┐
│                   Endpoint Cache (TTL-based)                               │
├────────────────────────────────────────────────────────────────────────────┤
│                                                                            │
│  ストレージ: HashMap<URL, CacheEntry>                                      │
│  GC間隔:     60秒                                                          │
│  デフォルトTTL: 300秒 (5分)                                                 │
│                                                                            │
│  検証:                                                                     │
│  • Blockchain Endpoint: chain_id (genesis hash) が一致するか               │
│  • Storage Node: ヘルスチェック (/rpc storage_health)                      │
│                                                                            │
└────────────────────────────────────────────────────────────────────────────┘
```

### 8.6 NetworkBehaviour 構成

```rust
// apps/storage-node/src/network/mod.rs

#[derive(NetworkBehaviour)]
pub struct StorageNodeBehaviour {
    /// Fragment取得/保存 (request-response)
    pub fragment_protocol: request_response::Behaviour<FragmentCodec>,
    /// ピア識別 (libp2p identify)
    pub identify: libp2p::identify::Behaviour,
    /// エンドポイント・ノード情報共有 (gossipsub)
    pub gossipsub: gossipsub::Behaviour,
}
```

### 8.7 イベントフロー

```
┌────────────────────────────────────────────────────────────────────────────┐
│                        Network Event Loop                                  │
├────────────────────────────────────────────────────────────────────────────┤
│                                                                            │
│  ┌─────────────────┐                                                       │
│  │  SwarmEvent     │                                                       │
│  └────────┬────────┘                                                       │
│           │                                                                │
│           ├─► NewListenAddr       → NetworkEvent::Listening                │
│           ├─► ConnectionEstablished → NetworkEvent::PeerConnected          │
│           ├─► ConnectionClosed    → NetworkEvent::PeerDisconnected         │
│           │                                                                │
│           ├─► Fragment::Request   → handle_request()                       │
│           │   └─► Get             → FragmentStore.retrieve()               │
│           │   └─► Put             → FragmentStore.store()                  │
│           │                         └─► NetworkEvent::FragmentStored       │
│           │                                                                │
│           ├─► Fragment::Response  → NetworkEvent::FragmentResponse         │
│           │                                                                │
│           └─► Gossipsub::Message                                           │
│               ├─► /anarchy/endpoints/1.0.0                                 │
│               │   └─► validate_message() → NetworkEvent::EndpointUpdate    │
│               └─► /anarchy/storage-nodes/1.0.0                             │
│                   └─► validate_storage_node_message()                      │
│                       └─► NetworkEvent::StorageNodeUpdate                  │
│                                                                            │
└────────────────────────────────────────────────────────────────────────────┘
```

### 8.8 セキュリティ特性

| 特性 | 実装 |
|------|------|
| **認証** | Ed25519署名必須（全Gossipsubメッセージ） |
| **完全性** | 署名にエンドポイント全フィールドのBlake2bハッシュを含む |
| **Sybil攻撃対策** | レピュテーションシステム（無効メッセージで即座にペナルティ） |
| **リプレイ攻撃対策** | タイムスタンプ検証（5分スキュー許容） |
| **DoS対策** | メッセージサイズ制限（4KB）、エンドポイント数制限（20件/メッセージ） |
| **チェーン分離** | chain_id (genesis hash) による異なるチェーンのエンドポイント拒否 |

---

## 7. 参考資料

- [EIP-4844: Shard Blob Transactions](https://eips.ethereum.org/EIPS/eip-4844)
- [Ethereum KZG Ceremony](https://ceremony.ethereum.org/)
- [BLS12-381 Specification](https://hackmd.io/@benjaminion/bls12-381)
- [Reed-Solomon Codes](https://en.wikipedia.org/wiki/Reed%E2%80%93Solomon_error_correction)
- [Shamir's Secret Sharing](https://en.wikipedia.org/wiki/Shamir%27s_Secret_Sharing)