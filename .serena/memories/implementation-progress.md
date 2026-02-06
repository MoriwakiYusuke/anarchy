# Anarchy プロジェクト実装進捗

## Phase 1: セキュア・ファンデーション（完了）

### 完了タスク
- ✅ Substrateノード基盤構築 (Polkadot SDK stable2503)
- ✅ Postパレット実装（投稿作成・保存・取得）
- ✅ Moralパレット実装（mint/burn/transfer）
- ✅ コンテンツ本文のオンチェーン保存（Contentsストレージ）
- ✅ 投稿時のMoral消費連携
- ✅ Next.js + PAPIフロントエンド

### 未着手タスク
- ❌ libp2p + Tor統合
- ❌ WebAuthn署名検証

## Phase 2-3: 未着手
- SSS断片化ストレージ
- ステルスアドレス（DM機能）
- 反応マイニング
- ZKP匿名人間証明

## 現在の動作状況
- ブロックチェーンノード: ws://127.0.0.1:9944
- フロントエンド: http://localhost:3000
- 投稿→オンチェーン保存→タイムライン表示が動作
