# Research: Post Storage Migration

**Feature**: 009-post-storage-migration  
**Date**: 2026-02-10

## 研究タスク

本Phaseで解決すべき技術的な不明点と調査結果。

---

## 1. Shamir's Secret Sharing (SSS) ライブラリ選定

### 調査対象

- `sharks` crate
- `vsss-rs` crate

### 決定: `sharks`

**理由**:
- シンプルなAPI: `Sharks::new(k)` → `dealer.make_shareholders(data)` → `sharks.recover(shares)`
- Wasm対応: `wasm-bindgen`との組み合わせ実績あり
- 軽量: 依存関係が少なく、ブラウザWasmに適している
- GF(256)ベース: 断片サイズがオリジナルと同等（膨張なし）

**却下した代替案**:
- `vsss-rs`: よりセキュアだが複雑。楕円曲線ベースで本ユースケースには過剰

### 実装例

```rust
use sharks::{Share, Sharks};

// 分割 (k=3, n=5)
let sharks = Sharks(3);
let dealer = sharks.dealer(&data);
let shares: Vec<Share> = dealer.take(5).collect();

// 復元 (任意のk個から)
let recovered = sharks.recover(&shares[0..3]).unwrap();
```

---

## 2. MerkleTree 実装

### 決定: `rs_merkle` crate + custom Blake2b hasher

**理由**:
- Wasm対応済み
- Proof生成・検証APIが明確
- カスタムハッシュ関数対応（Blake2bをAnarchy標準として統一）

**却下した代替案**:
- `merkle_light`: メンテナンス停滞
- 自作実装: 車輪の再発明

### 実装例

```rust
use rs_merkle::{MerkleTree, MerkleProof, Hasher};
use blake2::{Blake2b256, Digest};

struct Blake2bHasher;
impl Hasher for Blake2bHasher {
    type Hash = [u8; 32];
    fn hash(data: &[u8]) -> Self::Hash {
        Blake2b256::digest(data).into()
    }
}

// ツリー構築
let leaves: Vec<[u8; 32]> = fragments.iter()
    .map(|f| Blake2bHasher::hash(f))
    .collect();
let tree = MerkleTree::<Blake2bHasher>::from_leaves(&leaves);
let root = tree.root().unwrap();

// Proof生成
let proof = tree.proof(&[index]);
let proof_bytes = proof.to_bytes();

// Proof検証
let proof = MerkleProof::<Blake2bHasher>::from_bytes(&proof_bytes).unwrap();
proof.verify(root, &[index], &[leaf_hash], leaves.len())
```

---

## 3. Blockchain Node カスタムRPC実装パターン

### 既存コード分析

`apps/blockchain/node/src/rpc.rs` を確認:
- `jsonrpsee::RpcModule` を使用
- `FullDeps<C, P>` で依存性注入

### 決定: 既存パターンを拡張

**実装方針**:
1. `FullDeps`に`NetworkService`（libp2p接続用）を追加
2. `StorageRpc` trait を定義
3. `create_full()`で`module.merge()`

### 実装スケッチ

```rust
// rpc/storage.rs
#[rpc(server)]
pub trait StorageApi {
    #[method(name = "storage_uploadFragment")]
    async fn upload_fragment(
        &self,
        post_id: String,
        index: u32,
        data: Bytes,
        proof: Bytes,
    ) -> RpcResult<bool>;

    #[method(name = "storage_getFragment")]
    async fn get_fragment(
        &self,
        post_id: String,
        index: u32,
    ) -> RpcResult<Option<Bytes>>;
}

pub struct Storage<N> {
    network: Arc<N>,  // libp2p NetworkService
}

impl<N: NetworkService> StorageApiServer for Storage<N> {
    // MerkleProof検証 → Storage Node転送
}
```

---

## 4. Blockchain Node ↔ Storage Node 通信

### 既存コード分析

`apps/storage-node/src/network/mod.rs` を確認:
- **既にlibp2p request-responseが実装済み**
- `FragmentRequest::Get/Put` が定義済み
- `FragmentResponse::Data/Ack` が定義済み
- プロトコル: `/anarchy/fragment/1.0.0`

### 決定: 既存プロトコルを再利用

**実装方針**:
1. Blockchain Nodeにlibp2pクライアントを追加
2. 既存の`/anarchy/fragment/1.0.0`プロトコルで通信
3. Storage Nodeは変更不要（受信側は既に実装済み）

### 必要な変更

| コンポーネント | 変更内容 |
|--------------|---------|
| Blockchain Node | libp2p request-responseクライアント追加 |
| Storage Node | **変更不要**（既存プロトコルで対応可能） |

---

## 5. Wasm暗号エンジン パッケージ構成

### 決定: `packages/wasm-engine` として新規作成

**依存関係**:
```toml
[dependencies]
sharks = "0.5"
rs_merkle = "1.4"
blake2 = "0.10"
wasm-bindgen = "0.2"
serde = { version = "1.0", features = ["derive"] }
serde-wasm-bindgen = "0.6"

[lib]
crate-type = ["cdylib", "rlib"]
```

**エクスポート関数**:
- `split_data(data: &[u8], k: u32, n: u32) -> Vec<Fragment>`
- `recover_data(fragments: &[Fragment]) -> Vec<u8>`
- `build_merkle_tree(fragments: &[Fragment]) -> MerkleRoot`
- `generate_proof(tree: &MerkleTree, index: u32) -> MerkleProof`
- `verify_proof(root: &[u8], proof: &[u8], leaf: &[u8], index: u32) -> bool`

---

## 6. フロントエンド統合

### 決定: Web Worker + PAPI

**アーキテクチャ**:
```
Main Thread                     Worker Thread
    │                               │
    │ ── postMessage(content) ──▶   │
    │                               │ Wasm: split + merkle
    │ ◀── fragments[], proofs[] ──  │
    │                               │
    │ PAPI: storage_uploadFragment  │
    │ PAPI: post.create_post        │
```

**理由**:
- Main ThreadのUI応答性維持（SSS計算は重い）
- Wasmは`wasm-bindgen`経由でWorkerから読み込み
- PAPIはMain Threadで実行（WebSocket接続管理）

---

## 結論

| 項目 | 決定 |
|------|------|
| SSS | `sharks` crate |
| MerkleTree | `rs_merkle` + Blake2b |
| Blockchain RPC | jsonrpsee拡張（既存パターン） |
| Node間通信 | 既存libp2p protocol再利用 |
| Wasm | `packages/wasm-engine`新規作成 |
| Frontend | Web Worker + PAPI |

**NEEDS CLARIFICATION**: なし。Phase 1 設計に進む。
