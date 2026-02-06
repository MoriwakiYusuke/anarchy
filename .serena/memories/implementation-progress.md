# Anarchy プロジェクト実装進捗

## Phase 1: セキュア・ファンデーション

### 完了タスク
- ✅ Substrateノード基盤構築 (Polkadot SDK stable2503)
- ✅ Postパレット実装（投稿作成・保存・取得）
- ✅ Moralパレット実装（mint/burn/transfer）
- ✅ コンテンツ本文のオンチェーン保存（Contentsストレージ）
- ✅ 投稿時のMoral消費連携
- ✅ Next.js + PAPIフロントエンド
- ✅ Genesis設定でテストアカウントにMoral配布（Alice/Bob等に10,000 MORAL）
- ✅ **投稿コスト動的計算（byte数ベース）**
  - PostBaseCost = 10 MORAL（基本料金）
  - PostByteCost = 0.1 MORAL/byte（バイト単価）
  - 計算式: `total_cost = base_cost + content_bytes × byte_cost`
- ✅ usePostCost hook作成（コスト設定取得用）

### 作業中タスク
- 🔄 フロントエンドからruntime constantsの取得
  - PAPIでconstantsアクセスが「Runtime entry not found」エラー
  - フォールバック値で動作中（正確な値は取得できていない）

### 未着手タスク
- ❌ libp2p + Tor統合
- ❌ WebAuthn署名検証
- ❌ Identity Pallet

## Phase 2-3: 未着手
- SSS断片化ストレージ
- ステルスアドレス（DM機能）
- 反応マイニング（報酬ロジック）
- ZKP匿名人間証明

## 現在の動作状況
- ブロックチェーンノード: ws://127.0.0.1:9944
- フロントエンド: http://localhost:3000
- 投稿→オンチェーン保存→タイムライン表示が動作
- 投稿コストはフォールバック値（10 + 0.1×bytes）で表示

## 起動コマンド
```bash
# ノード起動
cd apps/blockchain && ./target/release/anarchy-node --dev --tmp

# フロントエンド起動
pnpm dev:frontend
```