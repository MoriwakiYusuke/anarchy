# Thread Reply 機能 — フロントエンド設計

- 日付: 2026-05-06
- スコープ: `apps/frontend` のみ (チェーン側は実装済み、変更なし)
- 関連 pallet: [`apps/blockchain/pallets/post`](../../../apps/blockchain/pallets/post/src/lib.rs)

## 背景

チェーン側 ([apps/blockchain/pallets/post/src/lib.rs](../../../apps/blockchain/pallets/post/src/lib.rs)) は既に
`Post.parent_id: Option<u64>` を持ち、`create_post` extrinsic も `parent_id` を受け取って
`ParentPostNotFound` バリデーションを行う。チェーンが保持するのは「親 0..1 個へのポインタ」のみで、
スレッド構造は持たない。

フロントエンドは `parent_id` を読み取り `Reply to #N` バッジを表示する箇所までは実装済み
([PostItem.tsx:251-255](../../../apps/frontend/src/components/PostItem.tsx#L251-L255))。
ただし [PostForm.tsx:185](../../../apps/frontend/src/components/PostForm.tsx#L185) は
`parent_id: undefined` 固定で、**返信を作成する UI は未実装**。

本設計では、X (Twitter) 風の UX で返信作成 + スレッド表示をフロントエンドに追加する。

## 非スコープ

- チェーン (pallet) の変更 — 一切しない
- Storage node の変更
- 新規 RPC / extrinsic の追加
- ページネーション / 無限スクロール (現状の 50 件上限はそのまま)
- 引用ポスト / リポスト
- 返信通知 (push notification)

## 振る舞い

### Timeline

- メインタイムラインは「root 投稿」を新着順 50 件まで表示
- root の定義: parent チェーンを遡って到達した最上位の post 自身
  - `parent_id === null` の場合は自分が root
  - 親が posts セット内に存在しない (削除等) 場合、その post 自身を root として扱う
- 各 root の `replyCount` は `posts.filter(p => rootIdOf(p) === root.id && p.id !== root.id).length`
- `rootIdOf(post)` は parent チェーンを遡るヘルパー (BFS / メモ化、orphan は self を返す)

### PostItem (トップレベル)

footer 配置:

```
[Post #N] [Reply to #M (if reply)]   [💬 3] [♥ 12] [👎 1]   [Reply]
```

- `[💬 N]`: `replyCount > 0` のときのみ表示。クリックで返信エリア展開/折りたたみ
- `[Reply]`: ボタン。クリックでカード直下にインライン PostForm を展開
- 展開された返信エリアは `<div className={styles.replyThread}>` で左罫線 + 軽インデント

### 返信のネスト表示

- 同じ root に属する全返信を、深さに関係なく **1 階層フラット** で親カード直下に並べる
- 各返信カードは通常 PostItem を再利用するが、`isNested` flag で:
  - footer の `[Reply]` ボタンは出さない (返信の返信は親の Reply ボタンから)
  - 自身の返信エリア展開機能 OFF
  - インデントスタイル適用
- 並び順: ブロック番号昇順 (古い→新しい、会話の流れ順)
- 各返信に元の `Reply to #M` バッジは残す (誰宛か分かるように)

### インライン返信フォーム

- `<PostForm parentId={postId} onCancel={...} onPostSuccess={...} />`
- ヘッダー: `Replying to #{parentId}` 表示
- キャンセルボタン (X アイコン or テキスト)
- 投稿成功後: 自動で閉じる + Timeline をリフレッシュ
- 失敗時: フォームは開いたままエラー表示 (既存の status 処理を流用)

## 実装

### コンポーネント変更

#### `PostForm.tsx`

新規 props 追加:

```ts
interface Props {
  unsafeApi: any
  signer: PolkadotSigner | null
  storageSigner: StorageSigner | null
  onPostSuccess?: () => void
  parentId?: number       // ← 追加: 返信先 post_id
  onCancel?: () => void   // ← 追加: インライン用キャンセル
}
```

- `parent_id` を `tx.Post.create_post` に渡す:
  `parent_id: parentId !== undefined ? parentId : undefined` (BigInt 化は PAPI が自動)
- `parentId` 設定時はフォーム上部に `Replying to #{parentId}` ヘッダー表示
- `onCancel` 設定時はキャンセルボタン表示

#### `PostItem.tsx`

新規 props:

```ts
interface Props {
  // ...既存
  replyCount?: number       // ← 追加
  replies?: ReplyData[]     // ← 追加: 既に解決済みの返信データ
  isNested?: boolean        // ← 追加: ネスト表示モード
  storageSigner?: StorageSigner | null  // ← 追加: 返信フォーム用
}
```

- `[💬 N]` ボタン: 返信エリア展開トグル
- `[Reply]` ボタン: 返信フォーム展開トグル
- ローカル state: `repliesExpanded`, `replyFormOpen`
- 返信フォーム展開時: カード下部に PostForm をレンダリング
- 返信エリア展開時: `replies` を map して `<PostItem isNested replyCount={0} />` でレンダリング

#### `Timeline.tsx`

- 既存の posts 取得は変更なし (全投稿を取る)
- 取得後の処理:
  ```ts
  const rootMap = computeRootMap(allPosts)  // postId -> rootId
  const repliesByRoot = groupBy(allPosts, p => rootMap.get(p.id))
  const topLevel = allPosts.filter(p => p.parentId === null)
  ```
- `topLevel` を `createdAt` 降順で並べて 50 件
- 各トップレベルに `replyCount` と `replies` (block 昇順 sort 済) を渡す

`computeRootMap` ヘルパーは [Timeline.tsx](../../../apps/frontend/src/components/Timeline.tsx) 内に inline 定義する
(他で使わないため共通化不要)。

### CSS (Timeline.module.css)

追加クラス:

- `.replyThread` — 親カード直下のラッパー、`border-left: 2px solid var(--accent)`、`padding-left: 16px`
- `.nestedPost` — ネスト返信用、フォントサイズ若干縮小、背景色変化
- `.replyCount` — 返信数バッジ (♥ と同じ系統のスタイル)
- `.replyForm` — インライン PostForm ラッパー (上下マージンとボーダー)
- `.replyFormHeader` — `Replying to #N` ラベル
- `.cancelButton`

### i18n

`apps/frontend/src/i18n/translations/{en,ja,zh}.json` に追加:

```jsonc
{
  "post.reply": "Reply" / "返信" / "回复",
  "post.replyTo": "Replying to #{id}" / "#{id} に返信" / "回复 #{id}",
  "post.viewReplies": "View {count} replies" / "返信を{count}件表示" / "查看 {count} 条回复",
  "post.hideReplies": "Hide replies" / "返信を隠す" / "隐藏回复",
  "post.cancelReply": "Cancel" / "キャンセル" / "取消"
}
```

`error.parentPostNotFound` は既存 ([PostForm.tsx:28](../../../apps/frontend/src/components/PostForm.tsx#L28)) のままで足りる。

### コスト

返信は既存 `create_post` を使うため通常投稿と同じ:
`PostBaseCost (10 MORAL) + content_bytes × PostByteCost (0.1 MORAL/byte)`。
表示も `usePostCost` フックそのまま流用。

## エッジケース

| ケース | 振る舞い |
|---|---|
| 親が削除済み (orphan reply) | その投稿自身を root として Timeline に表示。`Reply to #M` バッジは残るがリンクは効かない |
| 自己返信 | チェーンは許可。フロントも特別扱いしない |
| 返信に対する返信 | 全部 root の下にフラットに並ぶ。`Reply to #M` バッジで誰宛か区別 |
| 返信投稿失敗 (例: ParentPostNotFound) | 既存の `parseError` で `error.parentPostNotFound` 表示 |
| 返信フォーム展開中に親が削除された | 送信時にチェーンが `ParentPostNotFound` で reject、エラー表示 |
| 返信中に同 post に別の返信が増える | 次の refresh で反映 (現行と同じポーリング/リフレッシュトリガ) |

## テスト

### 単体 (Jest)

- `PostForm` に `parentId={42}` を渡したとき、`tx.Post.create_post` の `parent_id` 引数が `42` (BigInt) で呼ばれる
- `parentId` 未指定時は `parent_id: undefined`
- `onCancel` 呼び出しでフォームがクリアされる
- Timeline の root 計算ヘルパー (`computeRootMap`) が:
  - フラットな post 群でトップレベルだけ self-root を返す
  - 親 → 子 → 孫の鎖を root に正しく解決する
  - orphan (parent が posts に存在しない) は `null` を返す

### E2E (Playwright)

`apps/frontend` の Playwright 環境を使い、以下を 1 つの spec で検証:

1. 通常投稿 A を作成
2. 投稿 A の Reply ボタンを押す → インラインフォーム出現
3. 返信 B を投稿 → 自動で閉じる
4. 投稿 A の `[💬 1]` バッジが出現
5. バッジ or "View 1 replies" クリック → カード下に B がネスト表示される
6. B の `Reply to #A_id` バッジが見える

詳細は `playwright-e2e` skill のパターンに従う。

## マイルストーン

1. **PostForm** — `parentId`/`onCancel` props 追加、ヘッダー & キャンセル UI、`parent_id` 送信
2. **PostItem** — Reply ボタン、💬 バッジ、インラインフォーム埋め込み、ネスト返信レンダー、`isNested` 分岐
3. **Timeline** — root 計算 + topLevel フィルタ + replies グルーピング
4. **CSS** — `.replyThread`, `.nestedPost`, `.replyCount`, `.replyForm` 追加
5. **i18n** — 5 キーを 3 言語に追加
6. **テスト** — Jest 単体 + Playwright E2E

各マイルストーンは独立して PR 化せず、1 ブランチでまとめてコミットする (機能単位として小さい)。
