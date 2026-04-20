---
name: coding-standards
description: Anarchy プロジェクトのコーディング規約。Rust (FRAME pallet / no_std / wasm32v1-none ランタイム), TypeScript (Next.js + PAPI), 命名、コメント言語 (日本語優先)、ログ/デバッグ規則、エラー処理、import 順、型安全性の指針。新規ファイル追加・既存コード修正時に使用する。
---

# Coding Standards — Anarchy

プロジェクト共通の非交渉ルールと、言語ごとの規約。

## 言語横断ルール

### コメント言語
- **ドキュメントコメント (doc comment)、インラインコメントとも日本語優先**
- 外部 OSS 由来のパターンの理由説明は英語で可 (例: frame_support の attribute 意味)
- ユーザー向けエラーメッセージは i18n key で、**`error.<domain>.<key>`** 形式

### 命名
- Rust: `snake_case` (関数/変数/モジュール), `PascalCase` (型/トレイト/enum/struct), `SCREAMING_SNAKE_CASE` (const)
- TypeScript: `camelCase` (変数/関数), `PascalCase` (型/クラス/React コンポーネント), `SCREAMING_SNAKE_CASE` (モジュールレベル const), ファイル名は `camelCase.ts` / コンポーネントは `PascalCase.tsx`
- Extrinsic / runtime API 名は **snake_case** で統一 (例: `publish_dm_key`, `dispatches_at`)、TS 側からも同じ名前で呼ぶ

### コメントは「なぜ」を書く
「何をするか」は識別子で読めるべき。残すべきコメント例:
- 仕様書への参照 (`// contracts/pallet-messaging-extrinsics.md §R4`)
- 非自明な制約 (`// MerkleRoot が既存の場合は重複送信 → reject`)
- パフォーマンス・セキュリティ上の選択理由

消すべきコメント例:
```rust
// ❌ 変数を初期化
let x = 0;
// ❌ errorの場合は失敗を返す
return Err(...);
```

### ログ出力
- **秘密情報 (private key, seed phrase, session key, signed payload の中身) を絶対に出力しない**
- Rust runtime: `log::info!/warn!` のみ、`log::debug!` は release build で除去される保証無く wasm binary を肥やすため必要最小限
- Frontend: `console.log` は PR 前に削除。永続ログは `logger.info()` ラッパ経由で環境変数制御

## Rust (FRAME Pallet / Runtime)

### no_std 厳守
全 pallet crate の lib.rs 冒頭:
```rust
#![cfg_attr(not(feature = "std"), no_std)]
```

**禁止**:
- `std::vec::Vec` → `sp_std::vec::Vec`
- `std::collections::HashMap` → `sp_std::collections::btree_map::BTreeMap`
- `println!` / `dbg!` → `log::info!`
- `thread::spawn` / I/O / filesystem アクセス

### 型安全
- 全 Storage 型に `MaxEncodedLen` を derive (state growth upper bound 必須)
- extrinsic 引数の境界値は Config const で明示 (`MaxContentLength`, `MaxDispatchesPerBlock`)
- overflow 可能な算術は `checked_add` / `checked_mul` + `Error::Overflow` マップ
- Option は `?` で早期 return、`unwrap` は `#[cfg(test)]` 内のみ許可

### Error / Event 定義
- Error variant は引数無し (Event と分離)
- Event には副作用の結果を載せる (送信者・受信者・id 等)
- Error メッセージ文字列は持たせない (variant 名で説明)

### Import 順
```rust
// 1. 外部 crate (alphabetical)
use codec::{Decode, Encode};
use frame_support::{pallet_prelude::*, traits::fungible::Inspect};
use frame_system::pallet_prelude::*;
use sp_runtime::traits::Saturating;

// 2. 兄弟 pallet
use pallet_storage::StorageInterface;

// 3. 自 crate 内
use super::*;
use crate::types::DmDispatch;
```

### doc comment
```rust
/// 受信者がステルスアドレスを公開する。
///
/// # 失敗条件
/// - `InvalidMetaAddress`: scan_pub または spend_pub が all-zero
///
/// # Events
/// - `DmKeyPublished { account }`
///
/// 契約: contracts/pallet-messaging-extrinsics.md §E1
pub fn publish_dm_key(origin: OriginFor<T>, meta_address: DmMetaAddress) -> DispatchResult {
    ...
}
```

### unsafe
- runtime コードに `unsafe` は原則禁止
- wasm-engine 内で `unsafe` を使う場合は **関数単位でガードを SAFETY コメントに書く**

### clippy
```bash
cd apps/blockchain && cargo clippy -- -D warnings
```
CI で warning = error。新規コードは警告ゼロで merge。

## TypeScript (Next.js Frontend)

### strict モード固定
`tsconfig.json` で `"strict": true`。`any` / 暗黙 any 禁止 (PAPI の `getUnsafeApi()` 戻り値は暫定 `any` 許容、ただし新規ヘルパ関数で型付けして局所化)。

### 型定義の位置
- Domain 型は `src/types/<domain>.ts` に集約
- React コンポーネントの props は同ファイル内 `interface XxxProps`
- API 境界型 (Wasm / PAPI ↔ app) は別ファイルで定義、そこからだけ import

### React
- **Server Component デフォルト**。`'use client'` は必要時のみ、ファイル先頭に必ず記載
- hook 名は `useXxx`、1 hook 1 責務
- `useEffect` の cleanup を省略しない (特に subscription / timer)
- key は index を避け、安定な id を使う

### Import 順
```typescript
// 1. 標準 / 外部
import { useState, useCallback } from 'react'
import { createClient } from 'polkadot-api'
// 2. alias import (@/...)
import type { TransferState } from '@/types/transfer'
import { parseMoralAmount } from '@/types/transfer'
import { validateSS58Address } from '@/lib/addressValidation'
// 3. 相対 import
import { MyLocalHelper } from './helper'
```

### BigInt リテラル
- u64/u128 値は必ず BigInt (`1_000_000_000_000n`)
- 表示時は `formatMoral()` ヘルパ経由で 12 decimal 適用

### 非同期エラー
```typescript
try {
  const result = await unsafeApi.tx.Foo.bar().signAndSubmit(signer)
} catch (err) {
  // raw message は UI に出さない
  const userMsg = mapRpcError(err)
  setError(userMsg)
}
```

### ESLint
```bash
cd apps/frontend && pnpm lint
```
CI で error 扱い。`// eslint-disable-next-line` は理由コメント必須。

## Rust ⇄ TypeScript 境界 (Wasm)

### 型シリアライゼーション
- `#[wasm_bindgen]` 関数は `Vec<u8>` / `String` / number primitives に限定
- 複雑構造は JSON serialize して `String` で渡す、または `serde-wasm-bindgen`
- **bigint は直接渡せない**。u64 は `BigUint64Array` または string 経由

### エラー
```rust
#[wasm_bindgen]
pub fn dm_encrypt(plaintext: &[u8]) -> Result<Vec<u8>, JsError> {
    do_encrypt(plaintext).map_err(|e| JsError::new(&format!("encrypt: {}", e)))
}
```
TS 側で `try/catch`。raw error を UI に見せない。

## ファイル配置のルール

### 新規 pallet
```
apps/blockchain/pallets/<name>/
  Cargo.toml
  src/{lib.rs, types.rs, weights.rs, mock.rs, tests.rs}
```
必ず runtime の `construct_runtime!` と `Cargo.toml` の両方を更新。

### Frontend 新規機能
```
src/app/<feature>/page.tsx         # route
src/components/<feature>/*.tsx     # UI
src/hooks/use<Feature>.ts          # state & logic
src/lib/<feature>/*.ts             # chain / wasm 層
src/types/<feature>.ts             # 型
```

### テスト配置
- Rust unit: `<pallet>/src/tests.rs` (mock は同ディレクトリ `mock.rs`)
- Rust 統合: `apps/blockchain/tests/integration/<domain>/*.sh`
- Frontend unit: `src/lib/<domain>/__tests__/*.test.ts` または `src/components/<cmp>/<cmp>.test.tsx`

## 禁止事項 (CLAUDE.md の非妥協ルール再掲)

1. **偽のタスク完了報告**: 実装前に完了マーク禁止。動作確認 (cargo test / 実行) 必須
2. **存在しないファイル参照**: 作成/編集を報告する前に必ず tool 呼び出し
3. **テスト偽成功**: 実出力を確認してから報告
4. **未実装を「完了」と表記**: コード存在 + コンパイル / build 成功後のみ
5. **未検証の tasks.md checkbox 変更**: 100% 完了のみ `[X]`
6. **実装無しの mock-only test**: 実コードの検証を伴うテストのみ許容
