# assets/

ポートフォリオ・README 向けの画像アセット。

| ファイル | 用途 | 推奨置き換え |
|---|---|---|
| `logo.svg` | アイコン (240×240) | プロのロゴデザイン (Figma / 外注) で差し替え |
| `banner.svg` | README ヘッダ (1280×320) | 同上 |
| `screenshot-home.png` | タイムライン (Alice 接続済み) | UI 改訂のたびに撮り直し |
| `screenshot-home-disconnected.png` | 未接続ランディング | 同上 |
| `screenshot-stealth.png` | ステルスアドレスページ | 同上 |

## 撮影手順

[apps/frontend/scripts/capture-screenshots.mjs](../apps/frontend/scripts/capture-screenshots.mjs) が CLI Chromium 経由で全 3 枚を一括取得します。

```bash
pnpm stack:start                    # フロント + チェーン + ストレージ (依存順)
cd apps/frontend
BASE_URL=http://127.0.0.1:3000 node scripts/capture-screenshots.mjs
```

スクリプトは:

1. `localStorage.anarchy-locale = 'en'` を初期スクリプトで設定 (UI を英語に統一)
2. [e2e/fixtures/chain.ts](../apps/frontend/e2e/fixtures/chain.ts) と同じ Dev ドロップダウン経由で `//Alice` に Connect
3. リアル感のある英語投稿を 4 件作成 (PoW dev は 30s blocktime のため約 2-3 分かかる)
4. タイムラインと stealth ページを撮影

投稿をスキップしたいときは `SKIP_POSTS=1` を渡してください。

> Playwright MCP は OS 互換性 (chrome-for-testing) で WSL2 では動きません。CLI Chromium (`@playwright/test` 同梱) を使ってください。詳細は [.claude/skills/playwright-e2e/SKILL.md](../.claude/skills/playwright-e2e/SKILL.md) §「Playwright MCP は現状非対応」参照。

> 現在の `logo.svg` / `banner.svg` はプレースホルダ。本物のブランディングが固まったら差し替えてください。
