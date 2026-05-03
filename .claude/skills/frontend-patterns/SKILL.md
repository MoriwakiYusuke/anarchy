---
name: frontend-patterns
description: Anarchy フロントエンド (Next.js 14 App Router + React 18 + TypeScript + zustand) における PAPI 経由のチェーン連携、smoldot light client、anarchy-wasm-engine の利用、Web Worker プール、ステルス/DM スキャン実装、i18n 対応のパターン集。新規ページ/コンポーネント/フック追加、チェーン extrinsic 呼び出し、Wasm 経由の暗号処理を書く際に使用する。
---

# Frontend Patterns — Anarchy Next.js App

Next.js 14 App Router ベース。**@polkadot/api ではなく polkadot-api (PAPI) + smoldot light client** を正。Wasm 暗号処理は Web Worker プール経由。

## ディレクトリ構成

```
apps/frontend/src/
├── app/                       # Next.js App Router (page.tsx, layout.tsx)
│   ├── stealth/
│   └── dm/                    # 019-direct-messages 新規
├── components/                # 機能別コンポーネント
│   ├── stealth/
│   └── dm/
├── hooks/                     # React hooks (useXxx.ts)
├── lib/                       # 非 React ロジック
│   ├── smoldot-provider.ts    # PAPI client singleton
│   ├── stealth/               # ステルスアドレス client/scanner/keyManager
│   ├── dm/                    # DM sender/scanner/keyManager
│   ├── reaction/              # PoW mining control
│   └── matrix/                # (将来用)
├── workers/                   # Web Worker
│   ├── crypto.ts              # wasm-engine 呼び出しワーカー
│   └── WorkerPool.ts          # 共有ワーカープール
└── types/
```

## PAPI (polkadot-api) 必須ルール

Polkadot SDK stable2503 は metadata v16 で、**`@polkadot/api` は署名エラーで動かない**。PAPI + `getUnsafeApi()` で型無しアクセス (metadata 固定前の過渡期対応)。

### Client 初期化 (singleton)

`lib/smoldot-provider.ts` が唯一の PAPI client を持つ。二重生成禁止。

```typescript
import { createClient, PolkadotClient } from 'polkadot-api'
import { getSmProvider } from 'polkadot-api/sm-provider'
import { start } from 'smoldot'
import chainSpecJson from './chainspec.json'

// singleton
let papiClient: PolkadotClient | null = null

export async function initSmoldotClient(): Promise<PolkadotClient> {
  if (papiClient) return papiClient
  const smoldot = start({ forbidWs: false })
  const chainPromise = smoldot.addChain({ chainSpec: JSON.stringify(chainSpecJson) })
  const provider = getSmProvider(chainPromise)   // Promise を直接渡せる (await 不要)
  papiClient = createClient(provider)
  return papiClient
}
```

`chainspec.json` は `scripts/update-chainspec.sh` で dev node から自動抽出 (predev hook)。**手書き編集しない**。

### Storage read / Runtime API

```typescript
const client = await initSmoldotClient()
const unsafeApi = client.getUnsafeApi()

// Storage read
const balance = await unsafeApi.query.System.Account.getValue(address)

// Runtime API (例: DmScanApi)
const dispatches = await unsafeApi.apis.DmScanApi.dispatches_at(blockNumber)

// watchValue (reactive)
const sub = unsafeApi.query.Messaging.DmDispatchesByBlock
  .watchValue(blockNumber)
  .subscribe(value => { /* ... */ })
```

### Extrinsic 送信

```typescript
const tx = unsafeApi.tx.Messaging.send_dm({
  recipient_stealth: recipientAddress,
  ephemeral_pubkey: Binary.fromBytes(ephemeral),
  merkle_root: Binary.fromBytes(root),
  k: 3,
  n: 5,
  ciphertext_len: 4096n,            // BigInt 必須 (u64)
})
const result = await tx.signAndSubmit(signer)   // signer: PolkadotSigner
```

**よくある罠**:
- `u64` / `u128` 引数は必ず `BigInt` (`0n` リテラルまたは `BigInt(x)`)。number 渡すと decode 失敗
- `[u8; 32]` は `Binary.fromBytes(Uint8Array)` でラップ
- `AccountId` は SS58 string そのまま渡せる

## Hook 設計パターン

### State machine pattern (useTransfer 等)

`idle → confirming → pending → success | error` の明示的遷移。[apps/frontend/src/hooks/useTransfer.ts](apps/frontend/src/hooks/useTransfer.ts) が参考実装。

```typescript
type TransferStatus = 'idle' | 'confirming' | 'pending' | 'success' | 'error'
interface TransferState {
  status: TransferStatus
  recipient?: string
  amount?: bigint
  txHash?: string
  error?: string
}
```

validation は **trigger 関数 (transfer) と expose 関数 (validateRecipient) の両方から** 呼べるように切り出す → form 側で事前フィードバック可能。

### RPC タイムアウト

全 RPC 呼び出しを 30 秒でラップする (smoldot hang 対策):

```typescript
const RPC_TIMEOUT_MS = 30_000
function withTimeout<T>(p: Promise<T>, ms: number, label: string): Promise<T> {
  return Promise.race([
    p,
    new Promise<T>((_, reject) => setTimeout(() => reject(new Error(`Timeout: ${label}`)), ms))
  ])
}
```

### SSR 対策

Next.js App Router は default server-rendered。PAPI/smoldot/Wasm/Worker は必ず `'use client'` ファイル内で、かつ:

```typescript
if (typeof window === 'undefined') return null
```

ガードで SSR パスから除外する。`WorkerPool` constructor も同じガードを持つ。

## Wasm Engine (anarchy-wasm-engine) 利用

### インストール

`"anarchy-wasm-engine": "file:../../packages/wasm-engine/pkg"` で file dep。`postinstall` の `scripts/copy-wasm.sh` が `pkg/` を `node_modules/` にリンクする。**`pnpm install` の前に `wasm-pack build` 済みであること**。

### 直接呼び出し (メインスレッド、軽量処理のみ)

```typescript
import init, { hybrid_split, hybrid_reconstruct } from 'anarchy-wasm-engine'

await init()   // WASM instantiation (一度だけ)
const shards = hybrid_split(data, threshold, totalShards)
```

### Web Worker 経由 (重い処理は必須)

KZG proof 生成、SSS reconstruct、DM encrypt/decrypt scan 等は main thread で絶対回さない。`WorkerPool` に投げる:

```typescript
import { workerPool } from '@/workers/WorkerPool'

const result = await workerPool.run({
  id: crypto.randomUUID(),
  type: 'dm_encrypt',
  payload: { plaintext, recipientMetaAddress }
})
```

ワーカー数は `navigator.hardwareConcurrency || 4` を上限 8 にクランプ。round-robin 配分。

## ステルスアドレス / DM 共通パターン

### Key Manager (session-only)

秘密鍵は **セッションメモリのみ**。`lib/stealth/keyManager.ts` と `lib/dm/keyManager.ts` が所有。原則:

1. localStorage / IndexedDB に生鍵を書かない
2. `beforeunload` で明示破棄 (`zeroize` 相当)
3. エクスポートは AES-256-GCM + PBKDF2 で暗号化したバックアップ json のみ
4. インポートはパスフレーズ入力必須

### Scanner

ブロック範囲を区切って runtime API (`dispatches_range`) を呼び、worker で復号試行 (view-key マッチ)。

- 1 回の range 問い合わせは ≤ 1024 ブロック (pallet 側で reject される)
- progress イベントを store に流して UI で進捗表示
- スキャン位置はローカル (IndexedDB) に `last_scanned_block` として保持、再入場時差分のみ

## Zustand ストア規約

`lib/<domain>/store.ts` にまとめる。React の外から更新されるもの (scanner, worker) との連携に使う。

```typescript
import { create } from 'zustand'
interface DmStoreState {
  threads: Thread[]
  scanProgress: number
  addThread: (t: Thread) => void
}
export const useDmStore = create<DmStoreState>((set) => ({
  threads: [],
  scanProgress: 0,
  addThread: (t) => set(state => ({ threads: [...state.threads, t] })),
}))
```

- persist middleware を使う場合は **秘密情報を必ず除外** (session-only 原則)
- server component から import しない (`'use client'` ファイル経由で参照)

## エラーハンドリング方針

- ユーザー向けメッセージは `error.<key>` 形式で i18n 化 (`useLocale`)
- RPC エラーは raw message を表示せず、`TransactionError.fromRpc()` のような変換層で既知エラーへマップ
- 秘密鍵関連エラーは log に鍵内容を絶対入れない (`console.log(keyManager.privateKey)` は禁止)

## Foreground PoW 制御

reaction mining は必ず Page Visibility API でフォアグラウンド時のみ稼働:

```typescript
useEffect(() => {
  const onVisibility = () => {
    if (document.hidden) pauseMining()
    else resumeMining()
  }
  document.addEventListener('visibilitychange', onVisibility)
  return () => document.removeEventListener('visibilitychange', onVisibility)
}, [])
```

## i18n

`lib/i18n/` と `useLocale` フック。全ユーザー文字列を翻訳キー経由にし、ハードコード禁止 (特に英語/日本語混在のエラーメッセージ)。

## テスト (Jest + Testing Library)

- 配置: `apps/frontend/src/lib/<domain>/__tests__/` または `apps/frontend/tests/`
- `jest.config.ts` で `ts-jest` + `jest-environment-jsdom`
- Wasm を読むテストはメインスレッドロードを許す (`init()` を `beforeAll` で)
- PAPI は mock せず、**実際の chain spec + testnet 相当 mock** を立てるか worker 層までで切る

## よくある失敗

| 症状 | 原因 |
|---|---|
| `ReferenceError: window is not defined` | server component / SSR パスで client-only コードを import |
| `Codec decode error` on extrinsic | `number` を u64/u128 に渡している → `BigInt` に |
| `postinstall` 失敗 | `packages/wasm-engine/pkg/` が未生成 → `wasm-pack build` 先行 |
| smoldot 永続 "syncing" | chainspec.json が古い → `scripts/update-chainspec.sh` 再実行 |
| Worker 側から wasm-engine init が失敗 | Worker 内でも `await init()` が必要、メインスレッドの init は伝搬しない |
