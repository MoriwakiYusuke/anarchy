# Commands Reference

ビルド・テスト・開発用コマンドの一覧。`.claude/skills/dev-command/SKILL.md` も併用してください。

## スタック全体

| コマンド | 内容 |
|---|---|
| `pnpm stack:start` | testnet + storage + frontend を依存順起動 |
| `pnpm stack:stop` | 逆順で停止 |
| `pnpm stack:status` | 各層の稼働状況 |
| `pnpm stack:restart` | stop → start |
| `pnpm stack:purge` | stop + データ全消去 (`.next/` も含む) |

## ブロックチェーン

| コマンド | 内容 |
|---|---|
| `pnpm build:blockchain` | リリースビルド |
| `pnpm dev:node` | 単一 dev ノード起動 |

### Testnet (3 ノード)

| コマンド | 内容 |
|---|---|
| `pnpm testnet:start` | 起動 |
| `pnpm testnet:stop` | 停止 |
| `pnpm testnet:status` | ステータス |
| `pnpm testnet:logs` | ログ表示 |
| `pnpm testnet:purge` | データ削除 |

## Storage Node

| コマンド | 内容 |
|---|---|
| `pnpm storage:start` | 5 ノード起動 |
| `pnpm storage:stop` | 全停止 |
| `pnpm storage:status` | ステータス |
| `pnpm storage:purge` | データ削除 |

## フロントエンド

| コマンド | 内容 |
|---|---|
| `pnpm dev:frontend` | 開発サーバー (`http://localhost:3000`) |
| `pnpm build:frontend` | 本番ビルド |

## 統合テスト

| コマンド | 内容 |
|---|---|
| `pnpm test:integration` | 全テスト |
| `pnpm test:sync` | ブロック同期 / GRANDPA finality |
| `pnpm test:consensus` | ネットワーク分断・復旧 / ファイナリティ |
| `pnpm test:invalid` | 不正データ・壊れた署名の拒否 |
| `pnpm test:recovery` | クラッシュ後のリカバリ |
| `pnpm test:scalability` | 10 ノード協調 |

詳細は `apps/blockchain/tests/integration/`。

## Mint スクリプト ($MORAL)

| コマンド | 内容 |
|---|---|
| `node scripts/sudo-mint.mjs <account> <amount>` | Sudo で残高セット (推奨) |
| `node scripts/mint-moral.mjs <account> <amount>` | Alice から転送 |
| `node scripts/mint-moral-seed.mjs <seed> <amount>` | シードフレーズ由来アドレスに転送 |

使用例は [getting-started.md §4](getting-started.md#4-moral-トークン-mint-開発用) を参照。
