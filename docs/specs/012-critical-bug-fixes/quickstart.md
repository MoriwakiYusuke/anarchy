# Quickstart: Critical Bug Fixes (HIGH Priority 13 Issues)

**Date**: 2026-02-21  
**Feature**: [spec.md](spec.md) | [research.md](research.md) | [data-model.md](data-model.md)

## Prerequisites

```bash
# Rust toolchain (stable with wasm target)
rustup update stable
rustup target add wasm32v1-none

# Node.js & pnpm
node --version  # v20+
pnpm --version  # v8+

# Build dependencies
cd apps/blockchain && cargo build --release
cd packages/wasm-engine && wasm-pack build --target web --out-dir pkg
cd apps/frontend && pnpm install
```

## Implementation Order

以下の順序で実装することを推奨。依存関係と独立性を考慮。

### Phase 1: Independent Fixes (並行実装可能)

| Issue | Component | File | Estimated Time |
|-------|-----------|------|----------------|
| 8 | wasm-engine | key_sss.rs | 1h |
| 9 | wasm-engine | proof.rs | 1h |
| 6 | node | gossip/mod.rs | 2h |
| 7 | node | gossip/mod.rs | 2h |

### Phase 2: Pallet Storage (順次実装)

| Issue | File | Estimated Time | Dependency |
|-------|------|----------------|------------|
| 1 | lib.rs | 2h | - |
| 3 | lib.rs | 1h | - |
| 4 | lib.rs | 3h | - |
| 2 | lib.rs | 4h | Issue 1 |

### Phase 3: Storage Node (順次実装)

| Issue | File | Estimated Time | Dependency |
|-------|------|----------------|------------|
| 11 | chain/mod.rs | 3h | - |
| 10 | main.rs | 4h | Issue 11 |

### Phase 4: Shared Code

| Issue | Files | Estimated Time |
|-------|-------|----------------|
| 5 | 新規crate + 参照更新 | 2h |

### Phase 5: Frontend (順次実装)

| Issue | Files | Estimated Time |
|-------|-------|----------------|
| 12 | WorkerPool + PostItem | 4h |
| 13 | useScore + useStorage分割 | 6h |

---

## Quick Fix Guides

### Issue 1: issue_challenge スパム防止

```rust
// apps/blockchain/pallets/storage/src/lib.rs
// Line ~1105 after ensure_signed

pub fn issue_challenge(
    origin: OriginFor<T>,
    content_hash: super::ContentHash,
    node_account: T::AccountId,
    challenge_index: u8,
) -> DispatchResult {
    let issuer = ensure_signed(origin)?;
    
    // ★ 追加: 発行者が登録済みストレージノードか検証
    ensure!(
        OperatorNodes::<T>::contains_key(&issuer),
        Error::<T>::IssuerNotRegisteredNode
    );
    
    // 既存のロジック...
}
```

### Issue 2: チャレンジ期限切れ処理

```rust
// apps/blockchain/pallets/storage/src/lib.rs
// on_finalize hook 内

fn on_finalize(block: BlockNumberFor<T>) {
    // 既存の処理...
    
    // ★ 追加: 期限切れチャレンジの処理
    for challenge_id in ChallengesByDeadline::<T>::drain_prefix(block) {
        if let Some(challenge) = PendingChallenges::<T>::take(&challenge_id) {
            // スコア減算
            NodeScores::<T>::mutate(&challenge.challenged_node, |score| {
                *score = score.saturating_sub(T::ChallengeFailurePenalty::get());
            });
            Self::deposit_event(Event::ChallengeExpired { 
                challenge_id,
                challenged_node: challenge.challenged_node,
            });
        }
    }
}
```

### Issue 3: 報酬二重計上修正

```rust
// apps/blockchain/pallets/storage/src/lib.rs
// prove_holding_kzg 内 (~Line 1070)

// ★ 削除: ProofRecords への pending_reward 加算
// ProofRecords::<T>::mutate(content_hash, &prover, |record| {
//     record.pending_reward = record.pending_reward.saturating_add(reward);
// });

// 維持: PendingRewards のみを使用
if reward > 0 {
    PendingRewards::<T>::mutate(&prover, |pending| {
        *pending = pending.saturating_add(reward);
    });
}
```

### Issue 4: register_kzg_fragment 内部化

```rust
// apps/blockchain/pallets/storage/src/lib.rs

// ★ call_index 削除
// #[pallet::call_index(6)]  ← 削除
// #[pallet::weight(...)]    ← 削除
// pub fn register_kzg_fragment(...)  ← impl ブロックへ移動

impl<T: Config> Pallet<T> {
    /// 内部関数: create_post からのみ呼び出し可能
    pub fn do_register_kzg_fragment(
        owner: T::AccountId,
        content_hash: super::ContentHash,
        commitment: [u8; 48],
        // ...
    ) -> DispatchResult {
        // 既存のロジック
    }
}
```

### Issue 6: 接続数制限

```rust
// apps/blockchain/node/src/gossip/mod.rs

const MAX_CONNECTIONS: usize = 128;

// handle_notification_event 内
Some(NotificationEvent::ValidateInboundSubstream { result_tx, .. }) => {
    // ★ 修正: 接続数チェック
    if self.connected_peers.len() >= MAX_CONNECTIONS {
        let _ = result_tx.send(ValidationResult::Reject);
        warn!("Connection rejected: max connections ({}) reached", MAX_CONNECTIONS);
    } else {
        let _ = result_tx.send(ValidationResult::Accept);
    }
}
```

### Issue 8: RNG エラーハンドリング

```rust
// packages/wasm-engine/src/kzg/key_sss.rs

#[derive(Debug, Clone)]
pub enum KeySssError {
    RngFailed,
    InvalidThreshold,
    InvalidShareCount,
}

// ★ 修正: Result を返す
fn sss_split_byte(secret: u8, k: u8, n: u8) -> Result<Vec<(u8, u8)>, KeySssError> {
    let mut coeffs = vec![secret];
    let mut random_bytes = vec![0u8; (k - 1) as usize];
    
    // ★ expect → map_err
    getrandom::getrandom(&mut random_bytes)
        .map_err(|_| KeySssError::RngFailed)?;
    
    coeffs.extend(random_bytes);
    // ...
    Ok(shares)
}
```

### Issue 12: Web Worker プール

```typescript
// apps/frontend/src/workers/WorkerPool.ts

export class WorkerPool {
  private workers: Worker[];
  private taskQueue: Map<string, { resolve: Function; reject: Function }>;
  private currentWorkerIndex = 0;

  constructor(size = Math.min(navigator.hardwareConcurrency || 4, 8)) {
    this.workers = Array.from({ length: size }, () => {
      const worker = new Worker(
        new URL('./crypto.worker.ts', import.meta.url)
      );
      worker.onmessage = this.handleMessage.bind(this);
      return worker;
    });
    this.taskQueue = new Map();
  }

  async execute(task: CryptoTask): Promise<CryptoResult> {
    return new Promise((resolve, reject) => {
      this.taskQueue.set(task.id, { resolve, reject });
      // Round-robin assignment
      const worker = this.workers[this.currentWorkerIndex];
      this.currentWorkerIndex = (this.currentWorkerIndex + 1) % this.workers.length;
      worker.postMessage(task);
    });
  }

  private handleMessage(event: MessageEvent<CryptoResult>) {
    const task = this.taskQueue.get(event.data.id);
    if (task) {
      this.taskQueue.delete(event.data.id);
      if (event.data.success) {
        task.resolve(event.data);
      } else {
        task.reject(new Error(event.data.error));
      }
    }
  }

  terminate() {
    this.workers.forEach(w => w.terminate());
  }
}
```

---

## Testing

### Pallet Tests

```bash
cd apps/blockchain
cargo test -p pallet-storage -- --nocapture

# 特定テスト
cargo test -p pallet-storage test_issue_challenge_requires_registered_issuer
cargo test -p pallet-storage test_challenge_expiration
cargo test -p pallet-storage test_no_double_reward_accounting
```

### Wasm Engine Tests

```bash
cd packages/wasm-engine
cargo test

# 特定テスト
cargo test test_sss_split_byte_rng_error
cargo test test_vss_prove_commitment_mismatch
```

### Frontend Tests

```bash
cd apps/frontend
pnpm test

# 特定テスト
pnpm test -- --testPathPattern=WorkerPool
pnpm test -- --testPathPattern=useScore
```

### Integration Tests

```bash
# フル統合テスト
pnpm test:integration

# 個別
pnpm test:consensus
```

---

## Verification Checklist

各修正後に確認すること：

- [ ] `cargo build --release` が成功する
- [ ] `cargo test --all` が全てパスする
- [ ] `cargo clippy` に警告がない
- [ ] wasm-pack build が成功する
- [ ] `pnpm build:frontend` が成功する
- [ ] `pnpm test` (frontend) が全てパスする
