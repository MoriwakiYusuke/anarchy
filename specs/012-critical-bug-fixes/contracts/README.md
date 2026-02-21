# API Contracts: Critical Bug Fixes

**Date**: 2026-02-21  
**Feature**: [spec.md](../spec.md)

## Overview

本機能はバグ修正であり、新規APIエンドポイントの追加はなし。既存APIの動作変更のみ。

## Breaking Changes

### Pallet Storage Extrinsics

#### `register_kzg_fragment` (削除)

**Before**: 公開extrinsic として誰でも呼び出し可能

```typescript
// PAPI
api.tx.storage.registerKzgFragment(contentHash, commitment, ...);
```

**After**: 削除。Post pallet 内部からのみ呼び出し可能

```typescript
// ❌ エラー: extrinsic が存在しない
api.tx.storage.registerKzgFragment(contentHash, commitment, ...);
```

**Migration**: `create_post` を使用すること。直接の fragment 登録は禁止。

---

### Error Types (追加)

#### `IssuerNotRegisteredNode`

`issue_challenge` 呼び出し時、発行者が登録済みストレージノードでない場合に返される。

```rust
Error::IssuerNotRegisteredNode
```

**Affected**: `issue_challenge` extrinsic

---

## Unchanged APIs

以下のAPIは動作が変更されるが、シグネチャは維持:

- `issue_challenge`: 発行者検証追加（エラーが増える可能性）
- `prove_holding_kzg`: 報酬計上ロジック修正（外部動作は同一）
- `claim_rewards`: 報酬計算方法修正（外部動作は同一）

## Frontend Hooks

### useStorage

**公開インターフェース**: 変更なし

```typescript
// Before & After
const { upload, download, encrypt, decrypt } = useStorage();
```

**内部実装**: 分割されるが、公開APIは維持

### useScore

**Before**: モックデータを返す

```typescript
const { score, available } = useScore(contentHash);
// Returns mock data
```

**After**: 実際のブロックチェーンデータを返す

```typescript
const { score, available, loading, error } = useScore(contentHash);
// Returns real blockchain data
```

**追加フィールド**: `loading`, `error` が追加される可能性あり（詳細は実装時に決定）
