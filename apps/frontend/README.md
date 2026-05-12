# @anarchy/frontend

Anarchy の Web フロントエンド (Next.js 14 App Router + React 18 + TypeScript)。

## 技術スタック

| 領域 | 技術 |
|---|---|
| Framework | Next.js 14 (App Router) |
| UI | React 18, lucide-react |
| State | Zustand |
| Chain access | [polkadot-api (PAPI)](https://papi.how) via `getWsProvider` |
| Crypto | [anarchy-wasm-engine](../../packages/wasm-engine/) (KZG, SSS, Merkle, DM) |
| i18n | React Context + JSON 翻訳 (en / ja / zh) |
| Tests | Jest + Testing Library, Playwright (E2E) |

> ⚠️ **PAPI 必須**: Polkadot SDK stable2503 (metadata v16) で legacy `@polkadot/api` は使えません。

## 構成

```
src/
├── app/            # App Router pages
├── components/     # UI コンポーネント
├── hooks/          # zustand store + カスタムフック
├── lib/            # PAPI クライアント / 暗号ヘルパ
├── workers/        # Web Worker プール (Wasm 暗号処理用)
└── i18n/           # 翻訳ファイル

e2e/                # Playwright スペック
tests/              # Jest テスト
```

## 起動

```bash
# プロジェクトルートで
pnpm install              # postinstall で wasm-engine/pkg がコピーされる
pnpm dev:frontend
# http://localhost:3000
```

別途 chain ノード + storage ノードが起動している必要があります。
一括起動: `pnpm stack:start` (リポジトリルート)

## 環境変数

| 変数 | 既定値 | 用途 |
|---|---|---|
| `NEXT_PUBLIC_CHAIN_RPC_URL` | `ws://127.0.0.1:9944` | Chain ノード RPC。Onion address も指定可 |

## テスト

```bash
pnpm test               # Jest (ユニット + 統合)
pnpm test:e2e           # Playwright E2E
pnpm test:e2e:ui        # Playwright UI モード
```

WSL2 で headed モードを動かす際の注意点は [.claude/skills/playwright-e2e/SKILL.md](../../.claude/skills/playwright-e2e/SKILL.md)。

## セキュリティ原則

- **秘密鍵はセッションメモリのみ**: localStorage / IndexedDB に永続化しない
- **暗号処理はクライアント側で完結**: SSS 分割・KZG コミット生成は Web Worker + Wasm
- **storage-node に直接接続しない**: `storage_*` RPC を chain-node 経由で呼ぶ

詳細: [docs/architecture/frontend.md](../../docs/architecture/frontend.md)
