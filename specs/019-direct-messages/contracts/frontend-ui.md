# Contract: Frontend DM Library & UI

**Feature**: 019-direct-messages
**Target**: `apps/frontend/src/lib/dm/`, `apps/frontend/src/components/dm/`, `apps/frontend/src/app/dm/`
**Phase**: 1 (Design)

フロントエンドから見た DM 機能の公開 API（ライブラリ）と UI コンポーネント契約。

---

## 1. Library API (`apps/frontend/src/lib/dm/`)

### 1.1 `sender.ts`

```typescript
export interface SendDmParams {
  recipientAccountId: AccountId;   // UI 入力された受信者メインアカウント
  body: Uint8Array;                // 本文 (UTF-8 encode 済みテキスト or バイナリ)
  k?: number;                      // 既定 3
  n?: number;                      // 既定 5
}

export interface SendDmResult {
  messageId: bigint;
  blockNumber: bigint;
  recipientStealth: AccountId;
  merkleRoot: Uint8Array;
  paddingBucket: number;
  totalCostMoral: bigint;          // 実際に消費した MORAL (base + byte cost + tx fee)
}

export async function sendDm(params: SendDmParams): Promise<SendDmResult>;
```

**Behavior (失敗モード付き)**:
1. `recipientAccountId` の `DmReceptionKeys` を runtime API で取得 → 未公開なら `DmError.RecipientKeyNotPublished` throw。
2. wasm-engine: sender_signature を外部で作成 (ウォレット signer 経由)。
3. wasm-engine: `dm_encrypt_and_pad` で ciphertext + eph_pub + stealth を取得。
4. wasm-engine: `dm_fragment_ciphertext` → fragments。
5. storage-node 群へ並列アップロード。k 個 ACK で継続、足りなければ 30 秒以内にリトライ、なお駄目なら `DmError.StorageInsufficient` throw（MORAL 消費なし）。
6. wasm-engine: `dm_generate_sender_stealth` で sender stealth 生成。
7. PAPI: `pallet_stealth.send_to_stealth(sender_stealth, random_eph, pre_fund_amount)` を main account 署名で送信 & finalize 待ち。
8. PAPI: `pallet_messaging.send_dm(...)` を sender_stealth 鍵で署名し送信 & finalize 待ち。
9. 戻り値を構成。sender_stealth の seed はステップ 8 直後に JS 側でゼロクリア。

**Errors**:
- `DmError.RecipientKeyNotPublished`
- `DmError.StorageInsufficient`
- `DmError.MainAccountInsufficientBalance`
- `DmError.TransactionDropped` (tx1/tx2 いずれか finalize されず)
- `DmError.BodyTooLarge`

### 1.2 `scanner.ts`

```typescript
export interface ScanDmResult {
  scannedFromBlock: bigint;
  scannedToBlock: bigint;
  newMessages: DmMessageRecord[];
}

export async function scanDmInbox(opts?: {
  fromBlock?: bigint;              // 省略時は lastScannedBlock + 1
  toBlock?: bigint;                // 省略時は best head
}): Promise<ScanDmResult>;
```

**Behavior**:
- `DmScanApi::dispatches_range` を 1024 ブロックずつページング。
- 各 `DmDispatch` に対し `dm_decrypt_scan` を試行。`Some` で `signature_valid == true` のもののみ `newMessages` に積む。
- `IndexedDB` の `lastScannedBlock` を更新。

### 1.3 `keyManager.ts`

```typescript
export async function publishDmKey(): Promise<void>;
  // ローカルで DmMetaAddress を生成 (既存 stealth keyManager を呼出) して pallet_messaging.publish_dm_key を発行
export async function revokeDmKey(): Promise<void>;
export async function exportDmBackup(password: string): Promise<Uint8Array>;
  // DM 受信秘密鍵 + スキャンインデックス + ブロックリストを AES-GCM + PBKDF2 100k で封入
export async function importDmBackup(file: Uint8Array, password: string): Promise<void>;
```

**Behavior**:
- 秘密鍵はセッションメモリ (Zustand store) のみ。IndexedDB には暗号化済みの状態でのみ永続化。
- `exportDmBackup` のフォーマットは既存 `stealth/backup.rs` のスキーマに従うが、追加フィールド `"dm_scan_index"` と `"dm_block_list"` を含める。

### 1.4 `store.ts` (Zustand)

```typescript
interface DmStore {
  conversations: Map<string, ConversationState>;  // key = counterparty AccountId
  blockList: Set<AccountId>;
  lastScannedBlock: bigint;
  isScanning: boolean;

  addIncoming(message: DmMessageRecord): void;
  addOutgoing(message: DmMessageRecord): void;
  markAsRead(counterparty: AccountId, messageId: bigint): void;
  blockSender(account: AccountId): void;
  unblockSender(account: AccountId): void;
}
```

### 1.5 `worker.ts`

- Web Worker で `scanDmInbox` をループ実行。
- フォアグラウンド: 15 秒間隔、バックグラウンド: 5 分間隔（Page Visibility API）。
- scan 中は `isScanning = true` をストアに反映。

---

## 2. UI Components (`apps/frontend/src/components/dm/`)

### 2.1 `<ConversationList />`

**Props**: none (ストアから購読)
**Renders**: 会話一覧（相手アカウント、最終受信時刻、未読バッジ）。ブロック済みは非表示。
**Acceptance** (spec.md US2 受入シナリオを再掲):
- 3 人からのメッセージがストアにあるとき 3 スレッド表示。
- ブロック中のアカウントは非表示。

### 2.2 `<ConversationView conversationId={AccountId} />`

**Props**: `conversationId`
**Renders**: 時系列のメッセージ本文、`<MessageComposer />` を下部に。
**Acceptance**: 20 件ある会話でスクロールしてすべて chronological に表示、欠落・重複なし。

### 2.3 `<MessageComposer counterparty={AccountId} />`

**Props**: `counterparty`
**Actions**: 送信ボタン押下 → `sendDm` 呼出 → 成功時にストアへ `addOutgoing`。
**UX for FR-025 (pre-fund 透明化)**: 送信中に以下を順に表示:
1. "コンテンツを暗号化中…"
2. "分散ストレージへ断片を送信中… (x/n 完了)"
3. "ステルスアカウントを準備中… (MORAL 前送金中)"
4. "DM を発行中…"
5. 完了

**Acceptance**:
- 残高不足で `DmError.MainAccountInsufficientBalance` → 赤バナー表示、MORAL 消費なし、メッセージ履歴に残さない。
- 相手鍵未公開 → 「相手はまだ DM を受け付けていません」メッセージ。

### 2.4 `<DmKeyManager />`

**Props**: none
**Renders**:
- 公開状態 (public / revoked)。
- "公開する" / "取り消す" ボタン。
- "バックアップをエクスポート" / "インポート" ボタン (既存 `<BackupImportDialog>` を再利用)。

**Acceptance**:
- 未公開状態で他者から DM 送信を試みると相手側で失敗することを integration test で確認。

### 2.5 `<BlockListManager />`

**Props**: none
**Renders**: ブロック中アカウント一覧、追加・解除ボタン。
**Acceptance**:
- 追加後、新着スキャンで該当アカウントの会話が `<ConversationList />` に現れない。

### 2.6 `<MissingBackupNotice />` (FR-023)

**Props**: none (ストアから derived: DM 鍵が未ロードかつバックアップインポート未了)
**Renders**: 「このブラウザでは DM を復号する鍵がまだ読み込まれていません。バックアップファイルをインポートするか、新しい鍵を発行してください」と案内。警告的ではなくインフォメーション調。
**Acceptance**: DM ページ `/dm` 初回訪問時に鍵が無い場合に表示、バックアップインポート or `publishDmKey` 呼出で自動消失。

---

## 3. Routing (`apps/frontend/src/app/dm/`)

- `/dm` — ルート。鍵状態に応じて `<ConversationList />` もしくは `<MissingBackupNotice />` / `<DmKeyManager />` を出す。
- `/dm/[conversationId]` — `<ConversationView />`。
- `/dm/settings` — `<DmKeyManager />` + `<BlockListManager />`。

---

## 4. 依存ライブラリ

- 既存: `polkadot-api`, `@polkadot-api/descriptors`, `zustand`, `anarchy-wasm-engine` (file dep).
- 新規追加: なし。`wasm-engine` 側に DM モジュールを追加するのみで、フロントエンドからの import path は `anarchy-wasm-engine` 単一。
