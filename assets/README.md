# assets/

ポートフォリオ・README 向けの画像アセット。

| ファイル | 用途 | 推奨置き換え |
|---|---|---|
| `logo.svg` | アイコン (240×240) | プロのロゴデザイン (Figma / 外注) で差し替え |
| `banner.svg` | README ヘッダ (1280×320) | 同上 |
| `screenshot-home.png` | フロントエンドの主要画面キャプチャ | 実機で UI 改訂のたびに撮り直し |

## 撮影手順

```bash
pnpm stack:start                    # フロント + チェーン + ストレージ
pnpm --filter @anarchy/frontend exec playwright screenshot \
  --viewport-size=1440,900 \
  --device "Desktop Chrome" \
  http://localhost:3000 assets/screenshot-home.png
```

実機での E2E 設定は [.claude/skills/playwright-e2e/SKILL.md](../.claude/skills/playwright-e2e/SKILL.md) を参照。

> 現在の `logo.svg` / `banner.svg` はプレースホルダ。本物のブランディングが固まったら差し替えてください。
