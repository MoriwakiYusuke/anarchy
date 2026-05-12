# Contributing to Anarchy

Anarchy への貢献を検討いただきありがとうございます。本ドキュメントは開発参加の最短経路をまとめています。

> 完全な起動・コマンドガイドは [docs/development/](docs/development/) を参照してください。

## 開発環境

1. ツール:
   - Rust (stable) + `wasm32v1-none` target + `rust-src` component
   - Node.js 18+ / pnpm
   - `wasm-pack` (`cargo install wasm-pack`)
2. クローン後の初期化:
   ```bash
   pnpm install
   pnpm stack:start
   ```

詳細: [docs/development/getting-started.md](docs/development/getting-started.md)

## ブランチ・コミット

- ブランチ命名: `feature/<topic>` / `fix/<topic>` / `docs/<topic>`
- コミットメッセージ: Conventional Commits 風 (例: `feat(post): add reply pallet`, `fix(frontend,timeline): correct id casting`)
- main ブランチへの直接 push は禁止 (PR 経由のみ)

## Pull Request

PR を出す前に以下を確認してください:

- [ ] `cargo test` / `pnpm test` が通る
- [ ] 大きな機能追加時は E2E (`pnpm test:e2e`) を追加
- [ ] 設計判断やトレードオフを PR description に明記
- [ ] スクリーンショット / 動画 (UI 変更時)

## コーディング規約

| 領域 | 規約 |
|---|---|
| Rust | FRAME パターン / `no_std` (pallet) / [.claude/skills/coding-standards/SKILL.md](.claude/skills/coding-standards/SKILL.md) |
| TypeScript | Next.js + PAPI / Zustand / strict mode |
| コメント言語 | 日本語優先 (英語も可) |
| インデント | tabs (Rust) / 2 spaces (TS) |

## セキュリティ原則 (絶対遵守)

新規 RPC・認証・暗号処理・ネットワーク変更時は必ず:

1. **ネットワーク匿名** — Tor/I2P を libp2p トランスポート層で強制
2. **秘密鍵はセッションメモリのみ** — ブラウザ永続化禁止
3. **暗号はクライアント側のみ** — 暗号化・SSS 分割・メタデータ除去は送信前に完了
4. **PoW はフォアグラウンドのみ** — Page Visibility API で制御
5. **フロントは storage-node に直接接続しない** — chain-node RPC 拡張経由のみ

詳細: [.claude/skills/security-review/SKILL.md](.claude/skills/security-review/SKILL.md)

## 互換性ポリシー

このプロジェクトは初期開発段階のため、**前方・後方互換性は考慮しません**。
storage format / extrinsic signature / RPC / chain state / DB schema を破壊変更する際は migration を書かず、データ破棄 + 再生成で対応します。

## ドキュメント

- 設計判断は [docs/](docs/) 配下に置く
- 新規仕様は `docs/superpowers/specs/<date>-<topic>-design.md` (Superpowers skill 経由) を使用
- 過去設計 (Spec-Kit 時代の `docs/specs/` 形式) は `docs/archive/specs/` に保管。新規追加しない

## Issue / 質問

- バグ: GitHub Issues に再現手順付きで提出
- 設計議論: PR の RFC モードか Issue のラベル `design` で

## License

MIT License — コントリビュート時点で同意したものとみなします。
