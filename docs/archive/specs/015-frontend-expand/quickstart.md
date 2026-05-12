# Quickstart: フロントエンド拡充

**Feature**: 015-frontend-expand  
**Date**: 2026-02-25

## Prerequisites

- Node.js 18+
- pnpm 8+
- Rust toolchain (stable + wasm32v1-none)
- 実行中のブロックチェーンノード + ストレージノード

## Setup

### 1. ブロックチェーンノード起動

```bash
cd apps/blockchain
cargo build --release
./target/release/anarchy-node --dev
```

### 2. ストレージノード起動

```bash
cd apps/storage-node
cargo build --release
./target/release/anarchy-storage-node --config config.example.toml
```

### 3. wasm-engine ビルド（初回のみ）

```bash
cd packages/wasm-engine
wasm-pack build --target web --out-dir pkg
```

### 4. フロントエンド起動

```bash
pnpm install
pnpm dev:frontend
# http://localhost:3000
```

---

## Development Tasks

### Task 1: Nickname Pallet 実装

```bash
# 1. パレット作成
cd apps/blockchain/pallets
cargo new nickname --lib

# 2. テスト実行
cargo test -p pallet-nickname

# 3. ランタイム統合
cd apps/blockchain
cargo build --release
```

**Key Files**:
- `apps/blockchain/pallets/nickname/src/lib.rs`
- `apps/blockchain/pallets/nickname/Cargo.toml`
- `apps/blockchain/runtime/src/lib.rs` (統合)

---

### Task 2: 送金フォーム実装

```bash
# 1. コンポーネント作成
mkdir -p apps/frontend/src/components/TransferForm
touch apps/frontend/src/components/TransferForm/index.tsx
touch apps/frontend/src/components/TransferForm/TransferForm.module.css

# 2. フック作成
touch apps/frontend/src/hooks/useTransfer.ts

# 3. テスト実行
cd apps/frontend && pnpm test
```

**Key Files**:
- `apps/frontend/src/components/TransferForm/index.tsx`
- `apps/frontend/src/hooks/useTransfer.ts`
- `apps/frontend/src/i18n/ja.json` (翻訳追加)

---

### Task 3: 投稿者名表示改善

```bash
# 1. コンポーネント作成
mkdir -p apps/frontend/src/components/AddressDisplay
touch apps/frontend/src/components/AddressDisplay/index.tsx

# 2. フック作成
touch apps/frontend/src/hooks/useNickname.ts
```

**Key Files**:
- `apps/frontend/src/components/AddressDisplay/index.tsx`
- `apps/frontend/src/hooks/useNickname.ts`
- `apps/frontend/src/components/PostItem.tsx` (更新)

---

### Task 4: メディアアップロード実装

```bash
# 1. コンポーネント作成
mkdir -p apps/frontend/src/components/MediaUpload
touch apps/frontend/src/components/MediaUpload/index.tsx
touch apps/frontend/src/components/MediaUpload/MediaPreview.tsx

# 2. フック作成
touch apps/frontend/src/hooks/useMediaUpload.ts

# 3. Worker作成
mkdir -p apps/frontend/src/workers
touch apps/frontend/src/workers/mediaProcessor.worker.ts
```

**Key Files**:
- `apps/frontend/src/components/MediaUpload/index.tsx`
- `apps/frontend/src/hooks/useMediaUpload.ts`
- `apps/frontend/src/workers/mediaProcessor.worker.ts`
- `apps/frontend/src/lib/mediaProcessor.ts`

---

## Testing

### パレットテスト

```bash
cd apps/blockchain
cargo test -p pallet-nickname
cargo test -p pallet-post  # MediaRef拡張後
```

### フロントエンドテスト

```bash
cd apps/frontend
pnpm test                           # 全テスト
pnpm test -- TransferForm.test.tsx  # 特定テスト
```

### 統合テスト

```bash
# ノード起動状態で
pnpm test:integration
```

---

## Verification Checklist

### 1. 送金フォーム
- [ ] フォームが残高表示の下に表示される
- [ ] AccountId入力バリデーションが動作
- [ ] 金額バリデーションが動作
- [ ] 確認ダイアログが表示される
- [ ] 送金成功後に残高が更新される
- [ ] エラー時にメッセージが表示される

### 2. 投稿者名表示
- [ ] AccountIdが短縮形式で表示される
- [ ] クリックでフルアドレスがコピーされる
- [ ] "Copied!" 確認が表示される
- [ ] ホバーでツールチップが表示される

### 3. ニックネーム
- [ ] 設定画面でニックネームを入力できる
- [ ] 保存がオンチェーンに反映される
- [ ] タイムラインでニックネームが表示される
- [ ] 変更・削除ができる

### 4. メディアアップロード
- [ ] ドラッグ&ドロップで画像を追加できる
- [ ] プレビューが表示される
- [ ] 進捗バーが表示される
- [ ] 4ファイル制限が動作する
- [ ] サイズ制限が動作する
- [ ] フォーマット制限が動作する
- [ ] 投稿後にタイムラインで表示される

---

## Common Issues

### PAPI署名エラー
```
Error: Transaction has an invalid signature
```
→ `@polkadot/api` を使用している場合は PAPI に移行

### wasm-engine ロードエラー
```
Error: Cannot find module 'anarchy-wasm-engine'
```
→ `cd packages/wasm-engine && wasm-pack build --target web --out-dir pkg`

### Storage Node 接続エラー
```
Error: Failed to connect to storage node
```
→ ストレージノードが起動しているか確認: `curl http://localhost:3030/health`

### Nickname Pallet が見つからない
```
Error: Pallet 'Nickname' not found
```
→ ランタイムを再ビルド: `cd apps/blockchain && cargo build --release`

---

## Resources

- [PAPI Documentation](https://papi.how/)
- [Substrate Pallet Template](https://docs.substrate.io/tutorials/build-application-logic/)
- [wasm-pack Guide](https://rustwasm.github.io/wasm-pack/)
- [プロジェクトCLAUDE.md](../../CLAUDE.md)
