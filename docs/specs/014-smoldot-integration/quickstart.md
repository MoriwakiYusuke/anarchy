# Quickstart: smoldot Light Client統合

**Feature**: 014-smoldot-integration  
**Date**: 2026-02-24

## 前提条件

- Node.js 18+
- pnpm 8+
- Rust (stable) + `wasm32v1-none` target
- ブロックチェーンノードがビルド済み

## 開発環境セットアップ

### 1. ブロックチェーンノードをビルド

```bash
cd apps/blockchain
cargo build --release
```

### 2. チェーンスペックを出力

```bash
# スクリプトを実行
./scripts/export-chainspec.sh

# または手動で
./target/release/anarchy-node build-spec --dev --raw > ../frontend/src/lib/chainspec.json
```

### 3. smoldotパッケージをインストール

```bash
cd apps/frontend
pnpm add smoldot
```

### 4. フロントエンドを起動

```bash
pnpm dev:frontend
```

ブラウザで `http://localhost:3000` を開く。

### 5. ブロックチェーンノードを起動（ブートノード用）

```bash
cd apps/blockchain
./target/release/anarchy-node --dev

# マルチノードの場合
pnpm testnet:start
```

**注意**: smoldotはP2Pネットワーク経由で接続するため、少なくとも1つのノードが起動している必要があります。

## テスト実行

### 単体テスト

```bash
cd apps/frontend
pnpm test
```

### 特定のテストのみ

```bash
pnpm test -- --testPathPattern=useSmoldot
```

## 開発中の確認ポイント

### 接続状態の確認

ブラウザのDevToolsコンソールで以下を確認：

```javascript
// smoldotの初期化ログ
[smoldot] Initializing...
[smoldot] Chain added: anarchy_dev
[smoldot] Syncing...
[smoldot] Connected - Block #123
```

### ネットワークタブ

WebSocketの接続ではなく、P2Pトラフィック（表示されない）を使用していることを確認。

## トラブルシューティング

### チェーンスペックが見つからない

```
Error: Cannot find module './chainspec.json'
```

**解決**: `apps/frontend/src/lib/chainspec.json`が存在することを確認。

### ブートノードに接続できない

```
Error: Failed to connect to any bootnode
```

**解決**: 
1. ブロックチェーンノードが起動していることを確認
2. `chainspec.json`の`bootNodes`にノードのアドレスが含まれていることを確認

### WebAssemblyエラー

```
Error: WebAssembly not supported
```

**解決**: ブラウザがWebAssemblyをサポートしていることを確認（Chrome, Firefox, Safari最新版）

## ファイル構成

```
apps/frontend/
├── src/
│   ├── hooks/
│   │   ├── useApi.ts           # smoldotプロバイダー使用に変更
│   │   └── useSmoldot.ts       # 新規: smoldot状態管理
│   ├── lib/
│   │   ├── chainspec.json      # 新規: チェーンスペック
│   │   └── smoldot-provider.ts # 新規: smoldotプロバイダー
│   └── types/
│       └── connection.ts       # 新規: ConnectionState型

apps/blockchain/
└── scripts/
    └── export-chainspec.sh     # 新規: チェーンスペック出力
```

## コード変更の概要

### useApi.ts の変更

```typescript
// Before
import { getWsProvider } from 'polkadot-api/ws-provider/web'
const provider = getWsProvider(WS_ENDPOINT)

// After
import { getSmProvider } from 'polkadot-api/sm-provider'
import { initSmoldot } from '@/lib/smoldot-provider'
const chain = await initSmoldot()
const provider = getSmProvider(chain)
```

### 接続状態の変更

```typescript
// Before
const [isConnected, setIsConnected] = useState(false)

// After
const [connectionState, setConnectionState] = useState<ConnectionState>({
  status: 'initializing'
})
```

## 次のステップ

1. `/speckit.tasks` で実装タスクを生成
2. タスク順に実装を進める
3. 各タスク完了時にテストを実行
