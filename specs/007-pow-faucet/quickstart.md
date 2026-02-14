# Quickstart: PoW Faucet

**Feature**: 007-pow-faucet  
**Date**: 2026-02-09

## Prerequisites

- Rust 1.82+ with `wasm32v1-none` target
- Node.js 20+ with pnpm
- Running local Substrate node (`apps/blockchain/`)

## Quick Test

### 1. パレットのビルド・テスト

```bash
cd apps/blockchain

# パレット単体テスト
cargo test -p pallet-faucet

# ランタイム全体ビルド
cargo build --release
```

### 2. ローカルノード起動

```bash
./target/release/anarchy-node --dev --tmp
```

### 3. フロントエンド起動

```bash
cd apps/frontend
pnpm install
pnpm dev
```

### 4. Faucetテスト

1. ブラウザで `http://localhost:3000` を開く
2. 新規アカウントを作成（シードフレーズ生成）
3. ウォレット接続後、残高表示の下の「Faucet」ボタンをクリック
4. 計算完了後、残高が100 MORALになることを確認
5. 再度Faucetボタンを押し、「既に利用済み」エラーが表示されることを確認

## Development Workflow

### パレット開発

```bash
# 新しいテストを追加したらすぐにテスト
cargo test -p pallet-faucet

# ランタイムの型チェック
cargo check -p anarchy-runtime

# ベンチマーク（weight計算用）
cargo build --release --features runtime-benchmarks
```

### フロントエンド開発

```bash
cd apps/frontend

# 開発サーバー
pnpm dev

# テスト
pnpm test

# 型チェック
pnpm typecheck
```

## Key Files

### パレット

| File | Description |
|------|-------------|
| `apps/blockchain/pallets/faucet/src/lib.rs` | メインパレットロジック |
| `apps/blockchain/pallets/faucet/src/tests.rs` | ユニットテスト |
| `apps/blockchain/runtime/src/lib.rs` | ランタイム統合 |

### フロントエンド

| File | Description |
|------|-------------|
| `apps/frontend/src/components/WalletConnect.tsx` | Faucetボタンを含むウォレットUI |
| `apps/frontend/src/components/FaucetButton.tsx` | Faucetボタンコンポーネント |
| `apps/frontend/src/lib/faucet/worker.ts` | Web Workerスクリプト |
| `apps/frontend/src/hooks/useFaucet.ts` | Faucet状態管理フック |

## Testing Checklist

### Pallet Tests (Rust) ✅
- [X] `cargo test -p pallet-faucet` 全パス (11 tests)
- [X] 正しいPoW解でclaimが成功する
- [X] AlreadyClaimed: 同一アカウントで2回目のclaimが拒否される
- [X] ChallengeExpired: 期限切れブロック番号で拒否される
- [X] InvalidProof: 難易度を満たさないnonceで拒否される
- [X] BlockNotFound: 存在しないブロック番号で拒否される
- [X] 動的難易度: TotalClaimsに応じて難易度が正しく計算される
- [X] TotalClaims: claim成功時に+1される

### Frontend Tests (Jest) ✅
- [X] `pnpm test` 全パス (91 tests)
- [X] Faucetボタンが残高表示の下に表示される
- [X] ボタンクリックでWorkerが起動しPoW計算が開始される
- [X] 計算成功後にトランザクションが送信される
- [X] AlreadyClaimedエラーが日本語で表示される
- [X] ChallengeExpiredエラーが日本語で表示される
- [X] 計算中はローディング状態が表示される
- [X] エラー後もボタンは再度押せる状態になる

### Anonymity Verification ✅
- [X] パレットコードレビュー: IP/PII記録なし
- [X] ログ出力なし（scale-info/RuntimeDebugは非PII）

### Integration Tests (E2E)
- [ ] 新規アカウントでFaucet利用→残高増加
- [ ] 利用済みアカウントでFaucet利用→エラー表示
- [ ] フロントエンドE2E（手動）
  - [ ] 新規アカウントでFaucet成功
  - [ ] 既存アカウントで再請求拒否
  - [ ] 進捗表示の動作確認

## Common Issues

### ビルドエラー: "trait bounds not satisfied"

→ `pallet-balances`の`Config`トレイトとの整合性を確認。`NativeToken`型の定義を確認。

### Web Worker エラー: "Module not found"

→ Next.jsのworker設定を確認。`next.config.js`に`webpack`設定が必要な場合あり。

### Tor Browser で動作しない

→ blakejsはpure JSなので動作するはず。CORS設定を確認。
