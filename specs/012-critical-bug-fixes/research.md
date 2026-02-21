# Research: Critical Bug Fixes (HIGH Priority 13 Issues)

**Date**: 2026-02-21  
**Feature**: [spec.md](spec.md) | [plan.md](plan.md)

## Overview

13件のHIGH優先度issueに対するコードベース調査結果。各issueの現状、問題の根本原因、推奨される修正アプローチを記載。

---

## Issue 1: issue_challenge にスパム防止不十分

### 調査結果

**場所**: `apps/blockchain/pallets/storage/src/lib.rs` Lines 1102-1172

**現状の実装**:
```rust
pub fn issue_challenge(
    origin: OriginFor<T>,
    content_hash: super::ContentHash,
    node_account: T::AccountId,
    challenge_index: u8,
) -> DispatchResult {
    let issuer = ensure_signed(origin)?;
    // Rate limiting: check per-block challenge count for this issuer
    // Get KZG fragment
    // Security: Verify target node is actually a holder of this fragment
```

**問題点**:
- Rate limiting per issuer は実装済み
- Target node が holder かの検証済み
- **欠落**: 発行者(issuer)が登録済みストレージノードかの検証がない

### 決定

**修正アプローチ**: `issue_challenge`冒頭で `issuer` が `OperatorNodes` に登録されているか検証を追加

```rust
ensure!(OperatorNodes::<T>::contains_key(&issuer), Error::<T>::IssuerNotRegisteredNode);
```

**理由**: 相互チャレンジモデル（spec.md Clarifications参照）に基づき、登録済みストレージノードのみがチャレンジ発行可能

**却下した代替案**: 
- デポジット要求: 実装複雑度が高く、既存経済モデルへの影響が大きい
- ホワイトリスト: 中央集権的で Constitution I. Network Anonymity に反する

---

## Issue 2: チャレンジ期限切れ処理が完全に未実装

### 調査結果

**場所**: `apps/blockchain/pallets/storage/src/lib.rs` Lines 258-270

**現状の実装**:
```rust
fn on_finalize(block: BlockNumberFor<T>) {
    RegistrationCountPerBlock::<T>::remove(block);
    let _ = DeclareHoldingCountPerBlock::<T>::clear_prefix(block, u32::MAX, None);
    let _ = ChallengeCountPerBlock::<T>::clear_prefix(block, u32::MAX, None);
}
```

**問題点**: `PendingChallenges` の期限切れ処理が完全に欠落。チャレンジが蓄積し続けチェーン状態が無限肥大化。

### 決定

**修正アプローチ**: `on_finalize`で `ChallengesByDeadline::<T>::get(block)` を走査し、期限切れチャレンジを削除、対象ノードのスコア減算

```rust
// on_finalize に追加
for challenge_id in ChallengesByDeadline::<T>::drain_prefix(block) {
    if let Some(challenge) = PendingChallenges::<T>::take(&challenge_id) {
        // スコア減算
        NodeScores::<T>::mutate(&challenge.challenged_node, |score| {
            *score = score.saturating_sub(T::ChallengeFailurePenalty::get());
        });
        Self::deposit_event(Event::ChallengeExpired { challenge_id });
    }
}
```

**理由**: ブロック毎の処理量を最小化するため、deadline によるインデックス `ChallengesByDeadline` を活用

**却下した代替案**:
- 全`PendingChallenges`走査: O(n) で大量チャレンジ時にブロック時間超過リスク
- オフチェーン worker: 追加の複雑度、信頼性の問題

---

## Issue 3: 報酬の二重計上

### 調査結果

**場所**: `apps/blockchain/pallets/storage/src/lib.rs` Lines 1070-1090

**現状の実装**:
```rust
ProofRecords::<T>::mutate(content_hash, &prover, |record| {
    record.pending_reward = record.pending_reward.saturating_add(reward);  // ← 計上1
});
if reward > 0 {
    PendingRewards::<T>::mutate(&prover, |pending| {
        *pending = pending.saturating_add(reward);  // ← 計上2
    });
}
```

**問題点**: `ProofRecord::pending_reward` と `PendingRewards` の両方に同じ報酬が加算されている

### 決定

**修正アプローチ**: `ProofRecord::pending_reward` への加算を削除し、`PendingRewards` のみを使用

```rust
// record.pending_reward への加算を削除
// PendingRewards のみを使用
if reward > 0 {
    PendingRewards::<T>::mutate(&prover, |pending| {
        *pending = pending.saturating_add(reward);
    });
}
```

**理由**: 
- `PendingRewards` はノードごとの報酬集計に使用
- `ProofRecord::pending_reward` は個別トラッキング用だが、claim時に整合性が取れない

**却下した代替案**:
- `PendingRewards` 削除: claim処理の効率が悪化
- 両方維持してclaim時に調整: 複雑度が高く、バグの温床

---

## Issue 4: register_kzg_fragment が直接 extrinsic として公開

### 調査結果

**場所**: `apps/blockchain/pallets/storage/src/lib.rs` Lines 933-940

**現状の実装**:
```rust
#[pallet::call_index(6)]
#[pallet::weight(Weight::from_parts(20_000, 0) + T::DbWeight::get().writes(1))]
pub fn register_kzg_fragment(
    origin: OriginFor<T>,
    content_hash: super::ContentHash,
    // ...
) -> DispatchResult {
    let owner = ensure_signed(origin)?;  // ← 誰でも呼び出し可能
```

**問題点**: 誰でも直接呼び出せるため、create_postを経由せずにfragmentを登録し報酬を不正取得可能

### 決定

**修正アプローチ**: 
1. `register_kzg_fragment` から `#[pallet::call_index(6)]` を削除
2. 内部関数 `do_register_kzg_fragment(owner, ...)` を作成
3. Post pallet の tight coupling 経由でのみ呼び出し可能に

```rust
// lib.rs
impl<T: Config> Pallet<T> {
    pub fn do_register_kzg_fragment(
        owner: T::AccountId,
        content_hash: super::ContentHash,
        // ...
    ) -> DispatchResult {
        // Registration logic
    }
}

// create_post 内から呼び出し
storage::Pallet::<T>::do_register_kzg_fragment(sender, ...)?;
```

**理由**: Tight coupling により create_post トランザクションの一部としてのみ実行可能

**却下した代替案**:
- Origin検証: 詐称可能でセキュリティ不十分
- Call filter: 複雑度が高く、ランタイム設定変更が必要

---

## Issue 5: TAU_G2_BYTES がパレットとノードで重複定義

### 調査結果

**場所**: 
- `apps/blockchain/pallets/storage/src/kzg.rs`
- `apps/storage-node/src/storage.rs`

**問題点**: 同じ定数が2箇所で定義され、不整合リスク。末尾ゼロ埋め疑惑あり。

### 決定

**修正アプローチ**: 
1. `packages/kzg-constants/` crate を作成
2. TAU_G2_BYTES を一元定義
3. パレットとノードは依存として参照

```rust
// packages/kzg-constants/src/lib.rs
pub const TAU_G2_BYTES: [u8; 96] = [/* validated BLS12-381 G2 point */];
```

**理由**: 単一ソースにより不整合を物理的に排除

**却下した代替案**:
- build.rs でコピー: ビルド依存度の増加
- 環境変数: ランタイム不整合リスク

---

## Issue 6: Gossip受信接続を無条件Accept

### 調査結果

**場所**: `apps/blockchain/node/src/gossip/mod.rs` Lines 128-130

**現状の実装**:
```rust
Some(NotificationEvent::ValidateInboundSubstream { result_tx, .. }) => {
    let _ = result_tx.send(sc_network::service::traits::ValidationResult::Accept);
}
```

**問題点**: 接続数上限チェックなし。DoS攻撃で無限に接続を受け入れ、リソース枯渇

### 決定

**修正アプローチ**: 接続数上限チェックを追加

```rust
const MAX_CONNECTIONS: usize = 128;

Some(NotificationEvent::ValidateInboundSubstream { result_tx, .. }) => {
    if self.connected_peers.len() >= MAX_CONNECTIONS {
        let _ = result_tx.send(ValidationResult::Reject);
    } else {
        let _ = result_tx.send(ValidationResult::Accept);
    }
}
```

**理由**: 128接続は標準的なP2Pネットワークで十分なメッシュを確保しつつリソース消費を抑制

---

## Issue 7: Gossipメッセージによるレジストリ肥大化制限なし

### 調査結果

**場所**: `apps/blockchain/node/src/gossip/mod.rs` Lines 217-227

**現状の実装**:
```rust
if registry.register(node) {
    info!("Added storage node from gossip: {} (total: {})", endpoint, registry.nodes.len());
}
```

**問題点**: レジストリサイズ上限なし。悪意あるノードが大量の偽エントリを登録可能

### 決定

**修正アプローチ**: サイズ上限とLRU削除を実装

```rust
const MAX_REGISTRY_SIZE: usize = 10_000;

if registry.nodes.len() >= MAX_REGISTRY_SIZE {
    // LRU: 最も古いエントリを削除
    registry.evict_oldest();
}
if registry.register(node) {
    info!(...);
}
```

**理由**: 10,000エントリは大規模ネットワークに対応しつつメモリ使用量を約数MBに抑制

---

## Issue 8: sss_split_byte 内の expect() でRNG失敗時Wasmパニック

### 調査結果

**場所**: `packages/wasm-engine/src/kzg/key_sss.rs` Line 74

**現状の実装**:
```rust
getrandom::getrandom(&mut random_bytes).expect("RNG failure");  // ← パニック
```

**問題点**: ブラウザ環境でRNG失敗時、Wasmモジュール全体がパニックしアプリケーションクラッシュ

### 決定

**修正アプローチ**: Result型に変更しエラー伝播

```rust
pub fn sss_split_byte(secret: u8, k: u8, n: u8) -> Result<Vec<(u8, u8)>, KeySssError> {
    let mut random_bytes = vec![0u8; (k - 1) as usize];
    getrandom::getrandom(&mut random_bytes)
        .map_err(|_| KeySssError::RngFailed)?;
    // ...
}
```

**理由**: グレースフルデグラデーションによりUXを維持

---

## Issue 9: vss_prove がコミットメントとの整合性を検証していない

### 調査結果

**場所**: `packages/wasm-engine/src/kzg/proof.rs` Line 112

**現状の実装**:
```rust
let _ = commitment; // We trust the caller
```

**問題点**: コミットメント無視により、不正な多項式係数によるproof生成が可能

### 決定

**修正アプローチ**: 生成後に整合性検証

```rust
let proof = /* generate proof */;

// Verify proof matches commitment
if !self.verify_kzg_proof(commitment, share.index, &share.value, &proof)? {
    return Err(KzgError::CommitmentMismatch);
}

Ok(proof)
```

**理由**: 不正データの早期検出によりセキュリティ強化

---

## Issue 10: チャレンジモニターがメインループに統合されていない

### 調査結果

**場所**: `apps/storage-node/src/main.rs` Lines 157-265

**現状の実装**: select! マクロに以下のみ
- shutdown_rx
- heartbeat_interval  
- storage_node_broadcast_interval
- gc_check_interval
- network.handle_event

**問題点**: ChallengeMonitor モジュールが存在するが、メインループで使用されていない

### 決定

**修正アプローチ**: ChallengeMonitor をメインループに統合

```rust
let challenge_monitor = ChallengeMonitor::new(chain_client.clone(), kzg_prover.clone(), store.clone());
let mut challenge_rx = challenge_monitor.subscribe();

loop {
    select! {
        // 既存のbranch...
        
        Some(challenge) = challenge_rx.recv() => {
            // Handle challenge
            let proof = kzg_prover.generate_proof(&challenge)?;
            chain_client.submit_holding_proof(challenge.id, proof).await?;
        }
    }
}
```

**理由**: 既存の非同期アーキテクチャに自然に統合

---

## Issue 11: フェイルオーバー後に subxt クライアントが再接続されない

### 調査結果

**場所**: `apps/storage-node/src/chain/mod.rs` Lines 155-172

**現状の実装**:
```rust
if let Some(ref client) = *client_guard {
    return Ok(client.clone());  // ← 再接続チェックなし
}
```

**問題点**: Failover発生後も古いエンドポイントのクライアントを使用し続ける

### 決定

**修正アプローチ**: 
1. Failover時にクライアント無効化
2. Exponential backoff で再接続

```rust
async fn invalidate_client(&self) {
    let mut client_guard = self.subxt_client.lock().await;
    *client_guard = None;
}

async fn ensure_subxt_client(&self) -> Result<OnlineClient<SubstrateConfig>> {
    // Check if client is valid
    // If not, reconnect with exponential backoff
    // Max 10 retries, initial 1s, max 60s
}
```

**理由**: 既存の `report_failure` メカニズムと連携

---

## Issue 12: PostItem ごとに独立 Web Worker 生成

### 調査結果

**場所**: `apps/frontend/src/components/PostItem.tsx` + `useStorage.ts`

**問題点**: 各PostItemが useStorage() を呼び出し、それぞれが独自のWeb Workerを生成。50投稿 = 50 Worker

### 決定

**修正アプローチ**: SharedWorkerプールの導入

```typescript
// src/workers/WorkerPool.ts
class WorkerPool {
  private workers: Worker[];
  private taskQueue: Task[];
  
  constructor(size: number = navigator.hardwareConcurrency || 4) {
    this.workers = Array.from({ length: Math.min(size, 8) }, () => 
      new Worker(new URL('./crypto.worker.ts', import.meta.url))
    );
  }
  
  async execute(task: CryptoTask): Promise<CryptoResult> {
    // Round-robin or least-busy assignment
  }
}

// Context provider
export const WorkerPoolContext = createContext<WorkerPool | null>(null);
```

**理由**: CPU数に応じた動的調整で効率とリソース消費のバランス

---

## Issue 13: useScore がモック実装、useStorage.ts の責務過多

### 調査結果

**場所**: 
- `apps/frontend/src/hooks/useScore.ts` Lines 115-157: 100%モック
- `apps/frontend/src/hooks/useStorage.ts`: 517行

**問題点**: 
- useScore は simulateScoreFetch でモックデータを返す
- useStorage は SSS、Worker管理、RPC呼び出し、Merkle操作、認証を単一ファイルで処理

### 決定

**修正アプローチ**: 
1. useScore を実際のPAPIクエリに置き換え
2. useStorage を5ファイルに分割

```typescript
// useScore.ts - 実装
const fetchScore = async (contentHash: string) => {
  const api = await getApi();
  const score = await api.query.storage.scoreCache(contentHash);
  return { available: score.isSome, score: score.unwrapOr(0).toNumber() };
};

// useStorage分割
// src/hooks/storage/
//   useStorageWorker.ts   ~100行 - Worker pool
//   useStorageCrypto.ts   ~150行 - SSS/Merkle
//   useStorageRpc.ts      ~100行 - RPC calls
//   useStorageAuth.ts     ~80行  - Auth signing
//   useStorage.ts         ~80行  - Composition
```

**理由**: 単一責務原則によりテスト容易性と保守性向上

---

## Summary: Issue → Fix Mapping

| Issue | File | Line | Fix Type | Complexity |
|-------|------|------|----------|------------|
| 1 | pallet-storage/lib.rs | 1102 | Validation追加 | Low |
| 2 | pallet-storage/lib.rs | 258 | on_finalize拡張 | Medium |
| 3 | pallet-storage/lib.rs | 1070 | 重複削除 | Low |
| 4 | pallet-storage/lib.rs | 933 | Internal化 | Medium |
| 5 | kzg.rs/storage.rs | N/A | Crate抽出 | Medium |
| 6 | node/gossip/mod.rs | 128 | Limit追加 | Low |
| 7 | node/gossip/mod.rs | 217 | LRU追加 | Medium |
| 8 | wasm-engine/key_sss.rs | 74 | Result化 | Low |
| 9 | wasm-engine/proof.rs | 112 | Verify追加 | Low |
| 10 | storage-node/main.rs | 157 | select!統合 | Medium |
| 11 | storage-node/chain/mod.rs | 155 | Backoff追加 | Medium |
| 12 | frontend/PostItem.tsx | N/A | WorkerPool | Medium |
| 13 | frontend/hooks/ | N/A | 分割+実装 | High |
