# Frontend Logic - Anarchy Web Client

> Next.js 14 (App Router) + React 18 + TypeScript で構築された分散型SNSフロントエンド

## 1. アーキテクチャ概要

### 1.1 技術スタック

| カテゴリ | 技術 | バージョン | 用途 |
|---------|------|-----------|------|
| フレームワーク | Next.js (App Router) | 14.x | SSR/CSR ハイブリッド |
| UIライブラリ | React | 18.x | コンポーネント |
| 言語 | TypeScript | 5.x | 型安全性 |
| ブロックチェーン | PAPI (polkadot-api) | 1.x | チェーン接続（**@polkadot/api は非対応**） |
| 暗号処理 | anarchy-wasm-engine | - | KZG-VSS Hybrid, MerkleTree |
| ハッシュ | blakejs | 1.x | Blake2b-256 |
| アイコン | lucide-react | - | UI アイコン |

### 1.2 ディレクトリ構成

```
apps/frontend/src/
├── app/                    # Next.js App Router
│   ├── layout.tsx         # ルートレイアウト（メタデータ、プロバイダー）
│   ├── page.tsx           # メインページ
│   ├── providers.tsx      # クライアントプロバイダー
│   ├── globals.css        # グローバルスタイル
│   └── page.module.css    # ページスタイル
├── components/             # UIコンポーネント
│   ├── WalletConnect.tsx  # ウォレット接続
│   ├── PostForm.tsx       # 投稿フォーム
│   ├── Timeline.tsx       # タイムライン
│   ├── PostItem.tsx       # 投稿アイテム
│   ├── FaucetButton.tsx   # PoW Faucet
│   ├── MatrixBackground.tsx # 背景アニメーション
│   ├── LanguageSwitcher.tsx # 言語切替
│   └── ScoreIndicator.tsx # スコア表示
├── hooks/                  # カスタムフック
│   ├── useApi.ts          # PAPI接続
│   ├── useStorage.ts      # 分散ストレージ
│   ├── useFaucet.ts       # PoW Faucet
│   ├── useMoralBalance.ts # 残高取得
│   ├── usePostCost.ts     # 投稿コスト
│   ├── useScore.ts        # スコア取得
│   └── useReducedMotion.ts # アクセシビリティ
├── lib/                    # ライブラリ
│   ├── faucet/            # PoW計算
│   │   ├── challenge.ts   # チャレンジ生成
│   │   └── worker.ts      # Web Worker
│   └── matrix/            # 背景アニメーション
│       ├── index.ts       # エンジン
│       ├── config.ts      # 設定
│       └── types.ts       # 型定義
├── workers/                # Web Workers
│   └── crypto.ts          # Wasm暗号処理
└── i18n/                   # 国際化
    ├── index.ts           # エクスポート
    ├── context.tsx        # LocaleProvider
    ├── types.ts           # 型定義
    └── translations/      # 翻訳ファイル
        ├── en.json
        ├── ja.json
        └── zh.json
```

---

## 2. コア機能

### 2.1 ブロックチェーン接続 (useApi)

**ファイル**: [src/hooks/useApi.ts](../../apps/frontend/src/hooks/useApi.ts)

PAPI (polkadot-api) を使用してSubstrateノードに接続します。

```typescript
interface UseApiResult {
  client: PolkadotClient | null
  unsafeApi: any                    // getUnsafeApi() の結果
  isConnected: boolean
  error: string | null
  createSigner: (seedPhrase: string) => Promise<PolkadotSigner | null>
}
```

**重要な制約**:
- **PAPI必須**: Polkadot SDK stable2503 は metadata v16 を使用するため、`@polkadot/api` は動作しません
- **getUnsafeApi()**: 型定義なしでの柔軟なAPI呼び出し
- **定期ヘルスチェック**: 5秒ごとに `System.Number` クエリで接続確認

**環境変数**:
```bash
NEXT_PUBLIC_WS_ENDPOINT=ws://127.0.0.1:9944
```

### 2.2 分散ストレージ (useStorage)

**ファイル**: [src/hooks/useStorage.ts](../../apps/frontend/src/hooks/useStorage.ts)

KZG-VSS Hybrid方式でコンテンツを分割・復元します。

```typescript
interface UseStorageResult {
  uploadContent: (content: Uint8Array) => Promise<UploadResult>
  recoverContent: (merkleRoot: Uint8Array, metadata: HybridMetadata) => Promise<RecoverResult>
  progress: number           // 0-100
  error: string | null
  isProcessing: boolean
  isReady: boolean           // Worker準備完了
}
```

**処理フロー (Upload)**:
```
1. コンテンツ → Wasm Worker (hybrid_split)
   - AES-256-GCM暗号化
   - Reed-Solomon符号化
   - キーSSS分割

2. MerkleTree構築 (merkle_build)
   - 断片からMerkleRoot生成

3. RPC アップロード (storage_uploadFragment)
   - 各断片 + MerkleProof
   - 認証ヘッダー (X-Anarchy-Auth)
   - リトライ機構 (3回)
```

**処理フロー (Recover)**:
```
1. 断片取得 (storage_getFragment)
   - k個以上の断片をRPCで取得
   - インデックス順に試行

2. Wasm復元 (hybrid_recover)
   - Reed-Solomon復号
   - AES-256-GCM復号
```

**認証プロトコル**:
```typescript
interface SignedAuth {
  account_id: string      // Sr25519公開鍵 (hex)
  timestamp: number       // Unixタイムスタンプ (秒)
  nonce: string          // ランダム16バイト (hex)
  payload_hash: string   // Blake2b(request_body) (hex)
  signature: string      // Sr25519署名 (hex)
}
```

### 2.3 投稿フォーム (PostForm)

**ファイル**: [src/components/PostForm.tsx](../../apps/frontend/src/components/PostForm.tsx)

投稿作成のUIとトランザクション送信を担当します。

**投稿フロー**:
```
1. ユーザー入力
2. バイト数・コスト計算 (リアルタイム)
3. useStorage.uploadContent() で分割・アップロード
4. Post.create_post() でオンチェーン記録
   - merkle_root: [u8; 32]
   - k: 3 (閾値)
   - n: 5 (総断片数)
   - total_size: u64
```

**コスト計算式**:
```
総コスト = PostBaseCost + (バイト数 × PostByteCost)
         = 10 MORAL + (bytes × 0.1 MORAL)
```

**エラーハンドリング**:
| エラーコード | 翻訳キー | 説明 |
|-------------|---------|------|
| ContentTooLong | error.contentTooLong | 10,000バイト超過 |
| InsufficientMoralBalance | error.insufficientMoralBalance | 残高不足 |
| Payment | error.payment | トランザクション手数料不足 |

### 2.4 タイムライン (Timeline)

**ファイル**: [src/components/Timeline.tsx](../../apps/frontend/src/components/Timeline.tsx)

投稿一覧を表示します。

**データ取得**:
```typescript
// 全投稿メタデータ
const postEntries = await unsafeApi.query.Post.Posts.getEntries()

// V1: インラインコンテンツ (旧形式)
const contentEntries = await unsafeApi.query.Post.Contents.getEntries()

// V2: 分散ストレージ参照
const refEntries = await unsafeApi.query.Post.ContentRefs.getEntries()
```

**V1/V2 互換性**:
- V1投稿: `Contents` ストレージから直接取得
- V2投稿: `ContentRefs` から参照を取得 → `recoverContent()` で復元

### 2.5 投稿アイテム (PostItem)

**ファイル**: [src/components/PostItem.tsx](../../apps/frontend/src/components/PostItem.tsx)

個々の投稿を表示します。

```typescript
interface Props {
  postId: number
  author: string
  contentHash: string
  createdAt: number          // ブロック番号
  parentId: number | null    // リプライ先
  inlineContent?: string     // V1
  contentRef?: ContentRef    // V2
}
```

**コンテンツ表示ロジック**:
```
if (inlineContent) → 直接表示
else if (contentRef && isReady) → recoverContent() で復元
else → "読み込み中..."
```

### 2.6 PoW Faucet (useFaucet + FaucetButton)

**ファイル**: 
- [src/hooks/useFaucet.ts](../../apps/frontend/src/hooks/useFaucet.ts)
- [src/components/FaucetButton.tsx](../../apps/frontend/src/components/FaucetButton.tsx)

クライアントサイドPoW計算でトークンを請求します。

**処理フロー**:
```
1. 最新ファイナライズドブロック取得
2. 難易度計算
   difficulty = min(
     BaseDifficulty + log2(1 + TotalClaims/ScalingFactor),
     MaxDifficulty
   )
3. チャレンジ計算
   challenge = blake2b_256(block_hash || account_id)
4. Web Workerでマイニング
   - 解: hash(challenge || nonce) の先頭ゼロビット >= difficulty
5. Faucet.claim_faucet(block_number, nonce) 送信
```

**Web Worker (lib/faucet/worker.ts)**:
- 進捗報告: 50,000ハッシュごと
- ハッシュレート表示
- キャンセル機能

### 2.7 残高表示 (useMoralBalance)

**ファイル**: [src/hooks/useMoralBalance.ts](../../apps/frontend/src/hooks/useMoralBalance.ts)

```typescript
// ネイティブトークン残高 (System.Account.data.free)
const result = await unsafeApi.query.System.Account.getValue(address)
const balance = result?.data?.free ?? 0n
```

**精度**: 12桁 (1 MORAL = 1,000,000,000,000 units)

**更新タイミング**:
- 初回ロード
- `refreshTrigger` 変更時
- 10秒ごとのポーリング

### 2.8 投稿コスト取得 (usePostCost)

**ファイル**: [src/hooks/usePostCost.ts](../../apps/frontend/src/hooks/usePostCost.ts)

ブロックチェーンのランタイム定数から動的に取得します。

```typescript
// PAPI constants
const baseCost = await unsafeApi.constants.Post.PostBaseCost()
const byteCost = await unsafeApi.constants.Post.PostByteCost()
```

**フォールバック値**:
- PostBaseCost: 10 MORAL
- PostByteCost: 0.1 MORAL/byte

---

## 3. ウォレット接続 (WalletConnect)

**ファイル**: [src/components/WalletConnect.tsx](../../apps/frontend/src/components/WalletConnect.tsx)

### 3.1 認証モード

| モード | 用途 | 実装 |
|-------|------|------|
| dev | 開発用 | //Alice, //Bob, //Charlie |
| seedphrase | 本番用 | 12/24単語ニーモニック |

### 3.2 シードフレーズ処理

```typescript
// バリデーション
const { mnemonicValidate } = await import('@polkadot/util-crypto')
if (!mnemonicValidate(input)) {
  throw new Error('Invalid seed phrase')
}

// キーペア生成
const { Keyring } = await import('@polkadot/keyring')
const keyring = new Keyring({ type: 'sr25519' })
const pair = keyring.addFromUri(seedPhrase)
```

### 3.3 セキュリティ

- **メモリ内のみ保持**: シードフレーズはページを閉じると消去
- **クリア後の入力欄**: 接続後は入力欄をクリア
- **コピー機能**: ユーザーがバックアップ可能

---

## 4. 暗号処理 (Web Worker + Wasm)

**ファイル**: [src/workers/crypto.ts](../../apps/frontend/src/workers/crypto.ts)

メインスレッドをブロックしないよう、暗号処理はWeb Workerで実行します。

### 4.1 サポートする操作

| 操作 | 関数 | 説明 |
|-----|------|------|
| hybrid_split | HybridSplit.split() | KZG-VSS Hybrid分割 |
| hybrid_recover | HybridSplit.recover() | 復元 |
| merkle_build | build_merkle_tree() | MerkleTree構築 |
| merkle_generate_proof | generate_proof() | Proof生成 |
| merkle_verify | verify() | Proof検証 |
| blake2b_hash | blake2b_256() | ハッシュ計算 |

### 4.2 メッセージプロトコル

```typescript
// リクエスト
interface WorkerRequest {
  id: string
  type: "hybrid_split" | "hybrid_recover" | ...
  payload: unknown
}

// レスポンス
interface WorkerResponse {
  id: string
  success: boolean
  result?: unknown
  error?: string
}
```

### 4.3 Wasm初期化

```typescript
const module = await import("anarchy-wasm-engine")
await module.default()  // Wasmバイナリロード
```

---

## 5. 国際化 (i18n)

**ファイル**: [src/i18n/](../../apps/frontend/src/i18n/)

### 5.1 サポート言語

| コード | 言語 | ファイル |
|-------|------|---------|
| en | English | translations/en.json |
| ja | 日本語 | translations/ja.json |
| zh | 中文 | translations/zh.json |

### 5.2 使用方法

```typescript
import { useLocale } from '@/i18n'

function MyComponent() {
  const { t, locale, setLocale } = useLocale()
  
  return <p>{t('post.success', { block: '123' })}</p>
  // → "投稿しました！ (ブロック #123)"
}
```

### 5.3 永続化

- localStorage: `anarchy-locale` キー
- デフォルト: `en`

---

## 6. UI/UX

### 6.1 テーマ (Blood Glitch)

**グローバルスタイル** ([globals.css](../../apps/frontend/src/app/globals.css)):

```css
:root {
  --bg-primary: #0a0a0a;
  --bg-secondary: #141414;
  --text-primary: #ffffff;
  --text-secondary: #888888;
  --accent: #ff4444;
  --border: #2a2a2a;
  
  /* Matrix Background */
  --matrix-main: #333333;
  --matrix-head: #999999;
  --matrix-glitch: #CC0000;
}
```

### 6.2 Matrix背景アニメーション

**ファイル**: [src/components/MatrixBackground.tsx](../../apps/frontend/src/components/MatrixBackground.tsx)

cMatrix風の落下文字アニメーション。

**設定** ([lib/matrix/config.ts](../../apps/frontend/src/lib/matrix/config.ts)):
```typescript
{
  mainColor: '#333333',      // メイン文字
  headColor: '#CC0000',      // 先頭文字 (赤)
  glitchColor: '#00cc0a',    // グリッチ (緑)
  trailAlpha: 0.15,          // 残像透明度
  intervalMs: 100,           // アニメーション間隔
  glitchProbability: 0.0005, // グリッチ確率 0.05%
  fontSize: 16,
  streamLength: 12,
  columnGap: 1.5,
}
```

### 6.3 アクセシビリティ

- **prefers-reduced-motion**: アニメーションを無効化可能
- **useReducedMotion フック**: システム設定を検出

---

## 7. ビルド設定

### 7.1 Next.js設定 ([next.config.js](../../apps/frontend/next.config.js))

```javascript
{
  reactStrictMode: true,
  transpilePackages: [
    'anarchy-wasm-engine',
    '@polkadot/*',
    // ... 他のpolkadotパッケージ
  ],
  webpack: (config) => {
    config.experiments.asyncWebAssembly = true;
    config.resolve.fallback = {
      fs: false, net: false, tls: false, crypto: false,
    };
    return config;
  },
}
```

### 7.2 Wasm依存関係

```json
{
  "dependencies": {
    "anarchy-wasm-engine": "file:../../packages/wasm-engine/pkg"
  }
}
```

**ビルド順序**:
```bash
cd packages/wasm-engine && wasm-pack build --target web
cd apps/frontend && pnpm install && pnpm build
```

---

## 8. テスト

**ディレクトリ**: [apps/frontend/tests/](../../apps/frontend/tests/)

```bash
pnpm test              # 全テスト実行
pnpm test:watch        # ウォッチモード
pnpm test:coverage     # カバレッジ
```

**設定**: Jest + Testing Library

---

## 9. 環境変数

| 変数 | デフォルト | 説明 |
|-----|-----------|------|
| NEXT_PUBLIC_WS_ENDPOINT | ws://127.0.0.1:9944 | ノードWebSocket URL |

---

## 10. データフロー図

```
┌─────────────────────────────────────────────────────────────────┐
│                         Frontend                                 │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐        │
│  │ WalletConnect│   │  PostForm   │    │  Timeline   │        │
│  └──────┬──────┘    └──────┬──────┘    └──────┬──────┘        │
│         │                  │                  │                │
│         ▼                  ▼                  ▼                │
│  ┌─────────────────────────────────────────────────────┐       │
│  │                    Hooks Layer                       │       │
│  │  useApi │ useStorage │ useFaucet │ useMoralBalance  │       │
│  └─────────────────────────┬───────────────────────────┘       │
│                            │                                    │
│         ┌──────────────────┼──────────────────┐                │
│         ▼                  ▼                  ▼                │
│  ┌───────────┐     ┌───────────────┐   ┌───────────────┐      │
│  │Crypto     │     │ PAPI Client   │   │ Storage RPC   │      │
│  │Worker     │     │ (WebSocket)   │   │ (HTTP)        │      │
│  │(Wasm)     │     │               │   │               │      │
│  └─────┬─────┘     └───────┬───────┘   └───────┬───────┘      │
└────────┼───────────────────┼───────────────────┼───────────────┘
         │                   │                   │
         ▼                   ▼                   ▼
   ┌──────────┐        ┌──────────┐        ┌──────────┐
   │  Wasm    │        │Blockchain│        │ Storage  │
   │  Engine  │        │  Node    │        │  Node    │
   └──────────┘        └──────────┘        └──────────┘
```

---

## 11. セキュリティ考慮事項

### 11.1 秘密鍵の取り扱い

- **ブラウザメモリのみ**: シードフレーズはlocalStorageに保存しない
- **ページ離脱時消去**: React状態のクリーンアップ
- **クリップボードコピー**: ユーザー主導のバックアップのみ

### 11.2 クライアントサイド暗号化

- **AES-256-GCM**: コンテンツの機密性
- **SSS (Shamir's Secret Sharing)**: 鍵の分散
- **Reed-Solomon**: 可用性向上

### 11.3 署名検証

- **Sr25519**: すべてのトランザクション
- **SignedAuth**: ストレージNode認証
- **nonce + timestamp**: リプレイ攻撃防止
