# Data Model: smoldot Light Client統合

**Feature**: 014-smoldot-integration  
**Date**: 2026-02-24  
**Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)

## 1. ConnectionState

接続状態を管理するステートマシン。4つの状態を持つ。

### 型定義

```typescript
// apps/frontend/src/types/connection.ts

export type ConnectionStatus = 
  | 'initializing'  // smoldot起動中
  | 'syncing'       // チェーン同期中
  | 'connected'     // 接続完了、操作可能
  | 'error'         // エラー発生

export interface ConnectionState {
  status: ConnectionStatus
  blockNumber?: number    // 最新ブロック番号（connectedの場合のみ）
  errorMessage?: string   // エラーメッセージ（errorの場合のみ）
}
```

### 状態遷移図

```
┌──────────────┐
│ initializing │
└──────┬───────┘
       │ smoldot起動完了
       ▼
┌──────────────┐
│   syncing    │
└──────┬───────┘
       │
   ┌───┴────┐
   │        │
   ▼        ▼
┌─────────┐  ┌─────────┐
│connected│  │  error  │
└─────────┘  └─────────┘
```

### 状態遷移条件

| From | To | Condition |
|------|-----|-----------|
| initializing | syncing | smoldot.addChain()成功 |
| syncing | connected | query.System.Number.getValue()成功 |
| syncing | error | タイムアウト（60秒）またはaddChain()失敗 |
| initializing | error | Worker起動失敗、チェーンスペック読み込み失敗 |

### UIマッピング

| Status | 既存UIでの表示 |
|--------|---------------|
| initializing | 「接続中...」（既存の「未接続」状態を流用） |
| syncing | 「同期中...」（例外的に許可されたテキスト変更） |
| connected | 「接続済み」（既存） |
| error | 「接続エラー」（例外的に許可されたテキスト変更） |

---

## 2. ChainSpec

Anarchyブロックチェーンのチェーン仕様。ビルド時に静的に埋め込む。

### 型定義

```typescript
// apps/frontend/src/types/chainspec.ts

export interface ChainSpec {
  name: string              // "Anarchy"
  id: string                // "anarchy_dev" or "anarchy"
  chainType: ChainType
  bootNodes: string[]       // マルチアドレス形式
  genesis: GenesisConfig
  properties?: ChainProperties
  // ... その他Substrate標準フィールド
}

export type ChainType = 
  | 'Development'
  | 'Local'
  | 'Live'

export interface ChainProperties {
  tokenSymbol: string       // "MORAL"
  tokenDecimals: number     // 12
  ss58Format: number
}

export interface GenesisConfig {
  raw: {
    top: Record<string, string>
    childrenDefault: Record<string, Record<string, string>>
  }
}
```

### 保存場所

```
apps/frontend/src/lib/chainspec.json
```

### 生成方法

```bash
cd apps/blockchain
./scripts/export-chainspec.sh
```

---

## 3. SmoldotInstance

smoldotランタイムのインスタンス管理。シングルトンパターンを使用。

### 型定義

```typescript
// apps/frontend/src/lib/smoldot-provider.ts

import type { Client } from 'polkadot-api/smoldot'

export interface SmoldotState {
  client: Client | null
  chain: Chain | null
  worker: Worker | null
}

// モジュールレベルのシングルトン
let smoldotState: SmoldotState = {
  client: null,
  chain: null,
  worker: null
}
```

### ライフサイクル

1. **初期化**: アプリケーション起動時に`initSmoldot()`を呼び出す
2. **使用**: `getSmoldotProvider()`でPAPIプロバイダーを取得
3. **破棄**: アプリケーション終了時に`destroySmoldot()`を呼び出す

---

## 4. UseApiResult（既存型の拡張）

既存の`useApi`フックの戻り値型を拡張。

### 現在の型

```typescript
export interface UseApiResult {
  client: PolkadotClient | null
  unsafeApi: any
  isConnected: boolean
  error: string | null
  createSigner: (seedPhrase: string) => Promise<PolkadotSigner | null>
}
```

### 拡張後の型

```typescript
export interface UseApiResult {
  client: PolkadotClient | null
  unsafeApi: any
  connectionState: ConnectionState      // 変更: isConnected → connectionState
  error: string | null                  // 維持
  createSigner: (seedPhrase: string) => Promise<PolkadotSigner | null>  // 維持
}

// 後方互換性のためのヘルパー
export function isConnected(state: ConnectionState): boolean {
  return state.status === 'connected'
}
```

### コンポーネントでの使用例

```typescript
// Before
const { isConnected } = useApi()
if (isConnected) { ... }

// After
const { connectionState } = useApi()
if (connectionState.status === 'connected') { ... }

// または互換性ヘルパーを使用
import { isConnected } from '@/hooks/useApi'
if (isConnected(connectionState)) { ... }
```

---

## 5. 削除対象型

### WS_ENDPOINTの廃止

```typescript
// 削除
const WS_ENDPOINT = process.env.NEXT_PUBLIC_WS_ENDPOINT || 'ws://127.0.0.1:9944'
```

環境変数`NEXT_PUBLIC_WS_ENDPOINT`は不要になる。

---

## Entity Relationships

```
┌────────────────┐      uses      ┌────────────────┐
│  useApi Hook   │ ──────────────▶│ SmoldotState   │
└────────────────┘                 └────────────────┘
        │                                  │
        │ returns                          │ loads
        ▼                                  ▼
┌────────────────┐                 ┌────────────────┐
│ ConnectionState│                 │   ChainSpec    │
└────────────────┘                 └────────────────┘
```

---

## Validation Rules

### ConnectionState

- `status`は必須、4つの値のいずれか
- `blockNumber`は`status === 'connected'`の場合のみ存在
- `errorMessage`は`status === 'error'`の場合のみ存在

### ChainSpec

- `bootNodes`は空配列可（開発環境のみ）
- `genesis.raw`は必須
- JSONファイルとしてビルド時に存在すること
