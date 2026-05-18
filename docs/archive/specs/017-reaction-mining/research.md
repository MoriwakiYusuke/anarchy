# Research: Reaction Mining

**Feature**: 017-reaction-mining  
**Date**: 2026-02-28

このドキュメントはPhase 0で収集した技術調査結果をまとめる。

## 1. PoW検証ロジックの流用

### Decision
pallet-faucetの既存PoW検証ロジックを**共通モジュールとして抽出**し、pallet-reactionで再利用する。

### Rationale
- pallet-faucetには完成度の高いPoW実装が存在
- `compute_challenge()`, `verify_proof()`, `count_leading_zero_bits()` が再利用可能
- 同じBlake2b-256ベースのPoWを使用することで一貫性を保つ

### Implementation Pattern

```rust
// pallet-faucetから抽出する関数群:
// 1. compute_challenge(block_hash, account_id) -> [u8; 32]
// 2. verify_proof(challenge, nonce, difficulty) -> bool
// 3. count_leading_zero_bits(hash) -> u8

// 選択肢:
// A) 共通クレートを作成 (primitives/pow など)
// B) pallet-faucetをtraitとして公開し依存
// C) コードをコピー（非推奨）

// 推奨: A) 共通クレートを作成
// 理由: 将来他のパレットでもPoWを使う可能性あり
```

### Alternatives Considered
- **trait依存**: pallet-faucetへの直接依存はカップリングが強すぎる
- **コードコピー**: 保守性の観点から非推奨

---

## 2. 報酬プール管理

### Decision
pallet-storageの`RewardPoolBalance`パターンを流用し、独立した`ReactionRewardPool`を管理する。

### Rationale
- pallet-storageで実績のあるパターン（genesis設定、mint_into、残高管理）
- 投稿手数料の10%を`do_deposit_to_reaction_reward_pool()`経由で蓄積
- 90%は既存のストレージノード報酬プールへ

### Implementation Pattern

```rust
// StorageValue for reaction reward pool
#[pallet::storage]
pub type ReactionRewardPool<T: Config> = StorageValue<_, u128, ValueQuery>;

// Genesis config
#[pallet::genesis_config]
pub struct GenesisConfig<T: Config> {
    pub initial_reward_pool: u128,  // 10_000_000 * 10^12
}

// Deposit function (called by Post Pallet)
pub trait ReactionInterface {
    fn do_deposit_to_reaction_pool(amount: u128);
}
```

### Alternatives Considered
- **新規ミント**: インフレリスクが高い
- **投稿者からの直接支払い**: 投稿コストが増大する

---

## 3. WebWorker PoWマイニング

### Decision
既存の`workers/crypto.ts`を拡張し、`mine_reaction`タスクを追加する。

### Rationale
- WorkerPool基盤が既に存在し、安定稼働中
- Wasm初期化済みの環境を流用可能
- Blake2bハッシュはWasm側で高速実行可能

### Implementation Pattern

```typescript
// workers/crypto.ts に追加
case 'mine_reaction':
  const { challenge, difficulty } = payload as MineReactionPayload;
  let nonce = 0n;
  const startTime = performance.now();
  
  while (true) {
    const hash = wasmModule!.blake2b_hash(
      new Uint8Array([...challenge, ...bigIntToLeBytes(nonce)])
    );
    if (countLeadingZeroBits(hash) >= difficulty) {
      const elapsed = performance.now() - startTime;
      const hashrate = Number(nonce) / (elapsed / 1000);
      return { nonce: nonce.toString(), hashrate, elapsed };
    }
    nonce++;
    // 10000回ごとにyield (UIブロック防止)
    if (nonce % 10000n === 0n) {
      await new Promise(r => setTimeout(r, 0));
    }
  }
```

### Alternatives Considered
- **純JavaScript実装**: Wasmより低速
- **SharedArrayBuffer**: 複雑性が増す、PWA制約あり

---

## 4. Page Visibility API統合

### Decision
`useReactionMining`フック内でPage Visibility APIを監視し、バックグラウンド移行時にマイニングを一時停止する。

### Rationale
- Bot/自動化スクリプトによるスパム反応を防止
- ユーザーの意図的な参加を保証
- ブラウザ標準API、追加依存なし

### Implementation Pattern

```typescript
// hooks/useReactionMining.ts
const [isPaused, setIsPaused] = useState(false);
const abortControllerRef = useRef<AbortController | null>(null);

useEffect(() => {
  const handleVisibility = () => {
    if (document.hidden) {
      setIsPaused(true);
      abortControllerRef.current?.abort();
    } else {
      setIsPaused(false);
      // 自動再開はユーザーアクションに任せる or 自動再開
    }
  };
  
  document.addEventListener('visibilitychange', handleVisibility);
  return () => document.removeEventListener('visibilitychange', handleVisibility);
}, []);
```

### Alternatives Considered
- **サーバー側タイムスタンプ検証**: 実装が複雑、falsePositiveリスク
- **定期的なユーザーインタラクション要求**: UX悪化

---

## 5. 動的難易度調整アルゴリズム

### Decision
Bitcoin風の目標ベース調整を採用。直近N反応の平均時間から次の難易度を計算。

### Rationale
- シンプルで予測可能な動作
- pallet-faucetの`calculate_difficulty()`パターンを拡張
- 目標反応レート（例: 1ブロックあたり10反応）を維持

### Implementation Pattern

```rust
// 難易度調整ロジック
fn adjust_difficulty() {
    let recent_count = ReactionHistory::<T>::get(window_blocks);
    let target_rate = T::TargetReactionRate::get(); // e.g., 10 per block
    
    let current = CurrentDifficulty::<T>::get();
    let ratio = recent_count as i32 - target_rate as i32;
    
    // 調整係数（緩やかな調整）
    let adjustment = ratio / T::AdjustmentDivisor::get() as i32;
    let new_difficulty = (current as i32 + adjustment)
        .clamp(T::MinDifficulty::get() as i32, T::MaxDifficulty::get() as i32) as u8;
    
    CurrentDifficulty::<T>::put(new_difficulty);
}
```

### Alternatives Considered
- **固定難易度**: MVP可だが長期的に持続不可能
- **EMA(指数移動平均)**: 実装が複雑

---

## 6. インフレ調整係数γ

### Decision
γを報酬プール残高と総供給量の比率から導出。プール枯渇時は報酬減少。

### Rationale
- 経済的自律性を実現（Constitution原則V）
- 報酬プールが大きいほど報酬も大きい
- 自然なインフレ抑制

### Implementation Pattern

```rust
/// γを計算（PRECISION = 1_000_000）
fn calculate_gamma() -> u128 {
    let pool = ReactionRewardPool::<T>::get();
    let base_reward = T::BaseReactionReward::get(); // e.g., 0.01 MORAL
    
    // γ = min(1.0, pool / (base_reward * TARGET_REACTIONS * BLOCKS_PER_ERA))
    // 報酬プールが十分にある場合は1.0、不足時は比例的に減少
    let target_payout = base_reward
        .saturating_mul(T::TargetReactionRate::get() as u128)
        .saturating_mul(T::BlocksPerEra::get() as u128);
    
    if target_payout == 0 {
        return PRECISION;
    }
    
    (pool.saturating_mul(PRECISION) / target_payout).min(PRECISION)
}
```

### Alternatives Considered
- **固定γ**: インフレ/デフレリスク
- **供給量ベース**: 複雑性が増す

---

## 7. ステルスアドレス連携

### Decision
pallets/stealthの既存機能を活用。`reward_dest`パラメータで報酬先を指定可能。

### Rationale
- pallets/stealthで検証済みのステルスアドレス実装が存在
- 名寄せ防止によるプライバシー強化
- P3優先度のため、基本機能後に追加

### Implementation Pattern

```rust
// react extrinsic
pub fn react(
    origin: OriginFor<T>,
    post_id: u64,
    reaction_type: ReactionType,
    block_number: BlockNumberFor<T>,
    nonce: u64,
    reward_dest: Option<T::AccountId>,  // ステルスアドレス対応
) -> DispatchResult {
    // ...
    let effective_dest = reward_dest.unwrap_or(author.clone());
    // 報酬を effective_dest に送付
}
```

### Alternatives Considered
- **別エクストリンシック**: API複雑化
- **常時ステルス強制**: UX悪化

---

## Summary

| Topic | Decision | Confidence |
|-------|----------|------------|
| PoW流用 | 共通クレート抽出 | High |
| 報酬プール | pallet-storage パターン流用 | High |
| WebWorker | crypto.ts 拡張 | High |
| Visibility API | フック内で監視 | High |
| 難易度調整 | 目標ベース調整 | Medium |
| γ計算 | プール比率ベース | Medium |
| ステルス | pallets/stealth連携 | High |

**未解決事項**: なし（全てのNEEDS CLARIFICATIONが解決済み）
