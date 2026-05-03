---
name: tdd-workflow
description: Anarchy の TDD ワークフロー。Rust pallet (mock runtime + assert_ok!/assert_noop!)、Frontend (Jest + Testing Library + ts-jest)、Shell ベース統合テスト (apps/blockchain/tests/integration/)、wasm-engine 単体テストの使い分け。新機能追加・バグ修正・リファクタリング時にテスト先行で書くために使用。
---

# TDD Workflow — Anarchy

Anarchy のテストは 4 層構造。それぞれで先にテストを書き、失敗を確認してから実装する原則。**CLAUDE.md #6 "実装無しの mock-only test 禁止"** に違反しないよう、各層の守備範囲を厳密に分ける。

## 層構成

| 層 | ツール | 位置 | 守備範囲 |
|---|---|---|---|
| Rust pallet unit | `cargo test` + mock runtime | `pallets/*/src/tests.rs` | 単一 pallet の extrinsic / storage / event / error 論理 |
| Rust workspace integ | `cargo test --all` | 複数 pallet を含むテスト crate | pallet 間 trait 配線 / Runtime API |
| Wasm-engine unit | `cargo test -p anarchy-wasm-engine` | `packages/wasm-engine/src/**/tests.rs` | 暗号プリミティブ単体 |
| Frontend unit | `pnpm test` (Jest + jsdom) | `apps/frontend/**/__tests__/`, `*.test.ts(x)` | React コンポーネント / hook / lib 関数 / i18n key 存在チェック |
| Shell E2E | `pnpm test:integration` 等 | `apps/blockchain/tests/integration/**/*.sh` | dev node + storage node の複合起動シナリオ |

## Rust pallet: TDD フロー

### 1. Mock runtime を先に作る

`<pallet>/src/mock.rs` (別ファイル推奨) または `tests.rs` 先頭で。最小構成:

```rust
use crate as pallet_messaging;
use frame_support::{traits::{ConstU32, ConstU64, ConstU128, fungible::Mutate}};
use sp_runtime::{traits::{BlakeTwo256, IdentityLookup}, BuildStorage};

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        Messaging: pallet_messaging,
    }
);

// 依存 pallet の trait は mock struct で
pub struct MockStorage;
impl pallet_storage::StorageInterface<u64, u64> for MockStorage { /* no-op */ }

pub struct MockStealthReward;
impl crate::StealthRewardInterface for MockStealthReward {
    fn do_deposit_to_stealth_reward_pool(_amount: u128) {}
}

// frame_system / balances / 自 pallet の impl Config を続ける
```

参考: [apps/blockchain/pallets/post/src/tests.rs](apps/blockchain/pallets/post/src/tests.rs)

### 2. `new_test_ext()` で初期残高付与

```rust
pub fn new_test_ext() -> sp_io::TestExternalities {
    let t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| {
        System::set_block_number(1);  // block 0 は event 抑制されるので 1 に
        <Balances as Mutate<_>>::mint_into(&1u64, 10_000_000).unwrap();
    });
    ext
}
```

### 3. RED: 失敗するテストを書く

```rust
#[test]
fn send_dm_rejects_duplicate_merkle_root() {
    new_test_ext().execute_with(|| {
        // 事前状態: 一度成功
        assert_ok!(Messaging::send_dm(RuntimeOrigin::signed(1), /* ... */));

        // 2 回目の同一 merkle_root
        assert_noop!(
            Messaging::send_dm(RuntimeOrigin::signed(1), /* ... same root ... */),
            Error::<Test>::DuplicateContent
        );
    });
}
```

### 4. GREEN: extrinsic を実装

`ensure!(!DmMessagesByRoot::<T>::contains_key(merkle_root), Error::<T>::DuplicateContent);`

### 5. REFACTOR

重複バリデーションヘルパの抽出など。各ステップごとに `cargo test -p pallet-messaging` で緑を確認。

### テスト書き分けテンプレ

| 確認したいこと | マクロ |
|---|---|
| 正常系 extrinsic 成功 | `assert_ok!(...)` |
| エラー系で storage 不変 | `assert_noop!(call, Error::<Test>::Variant)` |
| エラー系 (storage 変更あり) | `assert_err!(call, Error::...)` |
| Event 発火 | `System::assert_has_event(RuntimeEvent::Messaging(Event::Xxx { ... }).into())` |
| Event 最新 | `System::assert_last_event(...)` |

### アサーションパターン

```rust
// Event の完全マッチ
System::assert_last_event(
    RuntimeEvent::Messaging(Event::DmDispatched {
        message_id: 1,
        recipient_stealth: 2,
        ephemeral_pubkey: [1u8; 32],
        content_hash: [2u8; 32],
        block_number: 1,
    }).into()
);

// Storage 値
assert_eq!(NextMessageId::<Test>::get(), 1);
assert!(DmMessagesByRoot::<Test>::contains_key([2u8; 32]));
```

## Frontend: Jest + Testing Library

### 設定
- `jest.config.ts` (ts-jest + `jest-environment-jsdom`)
- `apps/frontend/pnpm test`, `pnpm test:watch`, `pnpm test:coverage`

### 配置
- 関数/フック: `src/lib/<domain>/__tests__/foo.test.ts` または `src/hooks/__tests__/useFoo.test.ts`
- コンポーネント: 同ディレクトリ `Foo.test.tsx` 併置

### 典型パターン

```typescript
import { renderHook, act } from '@testing-library/react'
import { useTransfer } from './useTransfer'

describe('useTransfer', () => {
  it('validates empty amount', () => {
    const { result } = renderHook(() => useTransfer({ /* deps */ }))
    const validation = result.current.validateAmount('')
    expect(validation.valid).toBe(false)
    expect(validation.error).toBe('error.emptyAmount')
  })

  it('transitions idle → confirming on valid input', () => {
    const { result } = renderHook(() => useTransfer({ /* valid deps */ }))
    act(() => { result.current.transfer('5FH...', '10') })
    expect(result.current.state.status).toBe('confirming')
  })
})
```

### Wasm を含むテスト
- `beforeAll(async () => { await init() })` で WASM をロード
- main thread 実行 OK (jsdom 環境)
- Worker 経由コードはこの層でテストせず、Worker unit test を別途

### PAPI / smoldot のモック方針
- **原則モックしない**。代わりに
  - pure lib 関数 (validation, serialization, bigint 変換) を分離してそこをテスト
  - 統合フローは shell integration test に寄せる
- どうしても必要なら `jest.mock('@/lib/smoldot-provider')` で最小限

## Shell 統合テスト

### 位置
- `apps/blockchain/tests/integration/*.sh`
- 019 DM 用: `apps/blockchain/tests/integration/dm/` (T052 以降)

### 実行
```bash
pnpm test:dm                  # DM シナリオ
pnpm test:integration         # 全部
pnpm test:integration:quick   # 主要のみ
```

### 書き方規約
- 先頭で `set -euo pipefail`
- dev node + (必要なら) storage node を起動、終了時に cleanup (trap)
- PAPI CLI スクリプト (`scripts/`) を使ってチェーン操作 → 期待 Event / Storage 状態を確認
- 失敗時に exit 1、成功時に 0

### いつ shell に置くか
- 複数 pallet を跨ぐフロー
- blockchain node ↔ storage node の P2P 往復
- runtime upgrade / migration 検証

## Wasm-engine 単体テスト

```bash
cd packages/wasm-engine
cargo test
```

- Rust 側 `#[cfg(test)] mod tests` で書く
- wasm-bindgen-test は基本不要 (pure algorithm のテストのみで足りる)
- KZG / SSS / stealth / DM の各モジュールに vector test を置く

## カバレッジ目標

- **Rust pallet**: 主要 extrinsic の正常系 + 代表的な失敗系を全 Error variant 分
- **Wasm-engine**: 仕様書の known-answer test を必ず入れる (暗号系は 1 ビット間違いが致命)
- **Frontend hook**: 状態機械の全 transition, validation の全ケース
- **統合**: spec の acceptance criteria (`spec.md` の FR-xxx 各々に 1 シナリオ)

## TDD 違反しないチェックリスト

タスク完了を報告する前に:
1. `cargo test -p <pallet>` / `cd apps/blockchain && cargo test --all` を**実行して通っている**
2. Frontend なら `cd apps/frontend && pnpm test`, `pnpm lint`
3. 追加した extrinsic に **正常系 1 + 代表エラー ≥1** のテストあり
4. mock のみのテストで「実装済み」と報告していない (実 pallet コードが存在して compile する)
5. shell integ は dev node が実際に動く前提の内容になっている (placeholder echo だけで success を返す類ではない)

## 参考実装

| シナリオ | 参照 |
|---|---|
| mock runtime + fungible mint | `apps/blockchain/pallets/post/src/tests.rs:67-150` |
| MockStorage / MockReaction | `apps/blockchain/pallets/post/src/tests.rs:19-65` |
| Event assert | `apps/blockchain/pallets/reaction/src/tests.rs` |
| Frontend hook test | `apps/frontend/src/hooks/__tests__/useTransfer.test.ts` (存在する場合) |
| Shell integ 雛形 | `apps/blockchain/tests/integration/test_block_sync.sh` |
