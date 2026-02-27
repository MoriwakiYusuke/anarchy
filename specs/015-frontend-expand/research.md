# Research: フロントエンド拡充

**Feature**: 015-frontend-expand  
**Date**: 2026-02-25  
**Status**: Complete

## Research Topics

### 1. PAPI Transfer API

**Question**: MORALトークンの送金にはどのAPIを使用するか？

**Decision**: `Balances.transfer_allow_death` extrinsic を PAPI経由で呼び出す

**Rationale**: 
- PAPI (`polkadot-api`) は metadata v16 対応の唯一のクライアントライブラリ
- `@polkadot/api` は使用禁止（署名エラーが発生）
- `transfer_allow_death` は送金後に残高がゼロになることを許容（existential deposit を考慮しない）

**Implementation Pattern**:
```typescript
// apps/frontend/src/hooks/useTransfer.ts
import { PolkadotSigner } from 'polkadot-api/signer'

const api = client.getUnsafeApi()
const tx = api.tx.Balances.transfer_allow_death({
  dest: { type: 'Id', value: recipientAccountId },
  value: amountInPlanck  // 1 MORAL = 1_000_000_000_000 planck
})
const result = await tx.signSubmitAndWatch(signer)
```

**Alternatives Considered**:
- `transfer_keep_alive`: アカウント存続を保証するが、ゼロ送金後の残高チェックが複雑になる
- Legacy `@polkadot/api`: metadata v16 非対応のため使用不可

---

### 2. AccountId Validation

**Question**: SS58形式のAccountIdをどのように検証するか？

**Decision**: `@polkadot/util-crypto` の `decodeAddress` を使用

**Rationale**:
- 標準的なSS58デコード関数
- 無効なアドレスは例外をスローするため、try-catchでバリデーション可能
- ネットワークプレフィックス（42 = Substrate Generic）も検証可能

**Implementation Pattern**:
```typescript
import { decodeAddress, encodeAddress } from '@polkadot/util-crypto'

function validateAccountId(address: string): boolean {
  try {
    decodeAddress(address)
    return true
  } catch {
    return false
  }
}
```

**Alternatives Considered**:
- 正規表現: 長さチェックのみで、チェックサムを検証できない
- カスタム実装: 不要なコード量増加

---

### 3. Clipboard API

**Question**: AccountIdコピー機能の実装方法は？

**Decision**: `navigator.clipboard.writeText()` を使用し、フォールバックも実装

**Rationale**:
- モダンブラウザで広くサポート
- 非同期APIでエラーハンドリングが容易
- セキュアコンテキスト（HTTPS）でのみ動作するが、ローカル開発では動作

**Implementation Pattern**:
```typescript
async function copyToClipboard(text: string): Promise<boolean> {
  if (navigator.clipboard) {
    try {
      await navigator.clipboard.writeText(text)
      return true
    } catch {
      // フォールバック
    }
  }
  // Legacy fallback: document.execCommand (deprecated but works)
  const textArea = document.createElement('textarea')
  textArea.value = text
  document.body.appendChild(textArea)
  textArea.select()
  const success = document.execCommand('copy')
  document.body.removeChild(textArea)
  return success
}
```

**Alternatives Considered**:
- react-copy-to-clipboard: 追加依存、自前実装で十分

---

### 4. Media Split & Upload

**Question**: 大容量メディアファイル（100MB画像、1GB動画）をどのように分散ストレージに保存するか？

**Decision**: 既存の `hybrid_split()` (wasm-engine) を使用し、256KBシャードに分割してStorage Nodeにアップロード

**Rationale**:
- `MAX_FRAGMENT_SIZE = 256KB` が既に定義済み
- `hybrid_split()` は AES-256-GCM暗号化 + Reed-Solomon k-of-n分割 + SSS鍵分散 を実行
- 既存の `storage_storeFragment` / `storage_storeKzgShard` APIがそのまま利用可能
- ストレージノード側の変更は不要

**Implementation Pattern**:
```typescript
// Web Workerで実行（メインスレッドブロッキング回避）
import { hybrid_split, hybrid_recover } from 'anarchy-wasm-engine'

const result = hybrid_split(mediaBytes, 3, 5) // 3-of-5分割
// result.shards: HybridShard[] - 各シャードは 256KB 以下
// result.original_len, result.compressed などのメタデータ

// 各シャードを Storage Node にアップロード
for (const shard of result.shards) {
  await fetch('/api/storage_storeKzgShard', {
    body: JSON.stringify({
      merkle_root: result.merkleRoot,
      index: shard.index,
      data: base64Encode(shard.chunk),
      kzg_commitment: shard.kzg_commitment
    })
  })
}
```

**Alternatives Considered**:
- 独自チャンキング: SSS/KZGの利点が失われる
- Storage Node側での分割: クライアントサイド完結の原則に反する

---

### 5. Progress Tracking

**Question**: メディアアップロードの進捗表示はどのように実装するか？

**Decision**: シャードアップロード完了数 / 総シャード数 から計算

**Rationale**:
- `hybrid_split()` の結果でシャード数が確定
- 各シャードアップロード完了時にカウンターをインクリメント
- 実装がシンプルで正確

**Implementation Pattern**:
```typescript
const [progress, setProgress] = useState({ current: 0, total: 0 })

// 分割後
setProgress({ current: 0, total: shards.length })

// 並列アップロード（5並列程度）
for await (const batch of chunkedArray(shards, 5)) {
  await Promise.all(batch.map(async (shard) => {
    await uploadShard(shard)
    setProgress(prev => ({ ...prev, current: prev.current + 1 }))
  }))
}
```

**Alternatives Considered**:
- バイト単位進捗: XHR progressイベントはfetch APIでは複雑
- WebSocketストリーム: 過剰な複雑性

---

### 6. Nickname Pallet Design

**Question**: ニックネーム機能はどのパレット設計にするか？

**Decision**: 軽量な独自Nickname Palletを新規作成

**Rationale**:
- FRAME `pallet-identity` は本格的なオンチェーン身分証明向けで複雑すぎる
- ニックネームは「自称」であり、検証不要
- ユニーク制約なし（重複許可、識別はAccountId）
- 変更・削除可能

**Storage Design**:
```rust
#[pallet::storage]
pub type Nicknames<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    T::AccountId,
    BoundedVec<u8, ConstU32<128>>,  // 最大128バイト (UTF-8で32文字程度)
    OptionQuery
>;
```

**Extrinsics**:
1. `set_nickname(nickname: Vec<u8>)` - 設定/変更
2. `clear_nickname()` - 削除

**Query RPC**:
- `api.query.Nickname.nicknames(accountId)` で取得

**Alternatives Considered**:
- pallet-identity: 複雑すぎる、deposit要求あり
- Off-chain storage: 一貫性保証が難しい
- IPFS IPNS: 追加インフラ不要の方針に反する

---

### 7. EXIF Metadata Stripping

**Question**: 画像のEXIFメタデータをどのように削除するか？

**Decision**: Canvas APIで再エンコード

**Rationale**:
- クライアントサイドで完結（Constitution準拠）
- JPEGをCanvasに描画→再エクスポートするとEXIFは含まれない
- 追加ライブラリ不要

**Implementation Pattern**:
```typescript
async function stripExif(file: File): Promise<Blob> {
  const img = await createImageBitmap(file)
  const canvas = document.createElement('canvas')
  canvas.width = img.width
  canvas.height = img.height
  const ctx = canvas.getContext('2d')!
  ctx.drawImage(img, 0, 0)
  return new Promise(resolve => {
    canvas.toBlob(blob => resolve(blob!), file.type, 0.9)
  })
}
```

**Alternatives Considered**:
- exif-js: 読み取り専用、削除には別ライブラリ必要
- piexif.js: 追加依存

---

### 8. i18n Pattern

**Question**: 既存のi18nパターンに従うには？

**Decision**: `@/i18n` の `useLocale` フックと翻訳ファイル（ja.json, en.json, zh.json）を使用

**Rationale**:
- 既存コードベースで確立されたパターン
- `useLocale()` は `{ t, locale, setLocale }` を返す
- 翻訳キーは `types.ts` でTypeScript型定義

**Implementation Pattern**:
```typescript
import { useLocale } from '@/i18n'

const { t } = useLocale()
// 使用例: t('transfer.title')
```

**File Updates Required**:
- `apps/frontend/src/i18n/ja.json` - 日本語
- `apps/frontend/src/i18n/en.json` - 英語
- `apps/frontend/src/i18n/zh.json` - 中国語
- `apps/frontend/src/i18n/types.ts` - 型定義

---

## Summary

| Topic | Decision |
|-------|----------|
| 送金API | `Balances.transfer_allow_death` via PAPI |
| AccountId検証 | `@polkadot/util-crypto.decodeAddress` |
| クリップボード | `navigator.clipboard.writeText` + fallback |
| メディア分割 | `hybrid_split()` + existing storage APIs |
| 進捗表示 | シャード完了カウント |
| ニックネーム | 新規Nickname Pallet（軽量） |
| EXIF削除 | Canvas再エンコード |
| i18n | 既存 `useLocale` パターン |

**NEEDS CLARIFICATION**: なし（全て解決済み）
