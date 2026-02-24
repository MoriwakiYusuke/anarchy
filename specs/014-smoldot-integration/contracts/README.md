# Contracts: smoldot Light Client統合

**Feature**: 014-smoldot-integration  
**Date**: 2026-02-24

## 概要

この機能は外部向けAPIを変更しません。内部実装の変更のみです。

## 外部API変更

**なし**

smoldot統合はフロントエンド内部の接続層の変更であり、以下のAPIに影響しません：

- ユーザー向けUI（見た目は変更なし）
- ブロックチェーンRPCエンドポイント（フルノードのRPCは使用しない）
- ストレージノードAPI

## 内部インターフェース変更

### useApi Hook

戻り値の型が変更されます。詳細は[data-model.md](../data-model.md)を参照。

```typescript
// Before
interface UseApiResult {
  isConnected: boolean
  // ...
}

// After
interface UseApiResult {
  connectionState: ConnectionState
  // ...
}
```

## 削除されるインターフェース

### 環境変数

```
NEXT_PUBLIC_WS_ENDPOINT  // 削除
```

### Imports

```typescript
// 削除
import { getWsProvider } from 'polkadot-api/ws-provider/web'
```
