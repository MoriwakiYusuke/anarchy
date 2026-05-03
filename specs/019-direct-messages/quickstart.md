# Quickstart: Direct Messages (DM)

**Feature**: 019-direct-messages
**Phase**: 1 (Design output)
**Audience**: 開発者（本機能を初めて触る人）

実装完了後にローカル testnet で DM を一往復させるまでの手順。実装中は該当コンポーネントが未完のためステップごとに TDD で埋めていく。

---

## 前提

- リポジトリ `main` で `pnpm install` と `wasm-pack build` が完了している (`packages/wasm-engine/pkg` 生成済)。
- Rust toolchain (`apps/blockchain/rust-toolchain.toml`) が解決できる状態。
- 2 つの Anarchy アカウント (Alice / Bob) を testnet で保有。初期 MORAL: Alice=100, Bob=10（Alice が送信、Bob が受信）。

---

## 1. ブランチとブロックチェーンビルド

```bash
git checkout 019-direct-messages

# pallet-messaging を含むランタイムをビルド
pnpm build:blockchain

# テストネット起動 (3 ノード)
pnpm testnet:start
pnpm testnet:status
```

---

## 2. Bob の DM 受信鍵を公開 (FR-015)

フロントエンドで Bob としてログインし `/dm/settings` を開く。`DmKeyManager` の「DM を受信可能にする」を押すと:
1. ローカルで DM 用 `DmMetaAddress` が生成される (wasm-engine `stealth::keys::generate_meta_address` 流用)
2. `pallet_messaging.publish_dm_key(meta_address)` が発行される
3. 確認通知とともに "公開済み" 状態になる

確認:

```bash
# ブロック finalize 後、runtime API で取得できる
curl -X POST http://127.0.0.1:9944 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"state_call","params":["DmScanApi_reception_key", "<SCALE-encoded Bob AccountId>"]}'
# => `Some(DmMetaAddress { ... })`
```

UI の Developer ツールで `await publishDmKey()` を呼び出した結果、`pallet_messaging.DmKeyPublished { account: Bob }` イベントがダンプされることを確認。

---

## 3. Bob のバックアップをエクスポート (FR-022)

`/dm/settings` の "バックアップをエクスポート" をクリック。パスワードを求められるので入力し、`.dm-backup.bin` をダウンロード。

検証:

```bash
# 同一ブラウザで一度ログアウト → 再ログイン → 「鍵が読み込まれていません」表示 (FR-023)
# 続いて「バックアップをインポート」 → 復元 → 公開済み状態が復活
```

---

## 4. Alice が Bob に DM を送信 (US1)

フロントエンドで Alice としてログインし `/dm` に移動 → "新規 DM" → 受信者欄に Bob の AccountId → 本文 "こんにちは Bob" を入力 → 送信。

UI の進捗表示 (FR-025):

```
[✓] コンテンツを暗号化中…
[✓] 分散ストレージへ断片を送信中… (5/5 完了)
[✓] ステルスアカウントを準備中… (MORAL 前送金中)
[✓] DM を発行中…
完了
```

期待イベント:

```
- pallet_stealth.StealthTransfer { sender: Alice, stealth_address: AS, amount: pre_fund }
- pallet_messaging.DmDispatched { message_id: 0, recipient_stealth: RS, ephemeral_pubkey: E, content_hash: MR }
```

検証:

```bash
# オンチェーンで Alice から RS への直接アクションが一切無いことを確認
curl ... state_getStorage  pallet_messaging.DmDispatchesByBlock(<block>)
# → recipient_stealth は Bob の AccountId ではなく, derived stealth
```

---

## 5. Bob が DM を受信 (US1)

Bob 側のブラウザで `/dm` を開くと Web Worker が `scanDmInbox()` を起動。10〜30 秒以内に `<ConversationList />` に "Alice" との新規会話が現れる。開くと本文 "こんにちは Bob" が表示される。

デバッグコマンド:

```
# Web Worker コンソールで
await scanDmInbox({ fromBlock: <Alice 送信前のブロック> })
// => { newMessages: [{ counterparty: AliceAccount, body: "こんにちは Bob", ... }] }
```

送信者認証 (FR-004) の確認:

```
Web Worker コンソール
> store.conversations.get(AliceAccount).messages[0].signatureValid
true
```

署名無効時のふるまい確認:

```
# テスト用ツール: envelope の signature を 1 bit 反転して再送信
# → scan 結果に含まれないこと（drop される）を確認
```

---

## 6. Bob → Alice の返信 (US3)

Bob の `<ConversationView />` から返信 "やあ Alice!" を送信。Alice の `/dm` が 30 秒以内に会話末尾を更新する。

---

## 7. ブロック機能 (FR-011)

Alice が Bob の会話を "ブロック" に追加 → Bob からの新規 DM は `<ConversationList />` に出なくなる（スキャナは受信しストアには入るが UI でフィルタ）。

検証:

```
> store.conversations.get(BobAccount).blocked
true
> <ConversationList /> の DOM に Bob のエントリが無いこと
```

---

## 8. 多端末同期 (FR-022)

1. Bob がステップ 3 でエクスポートした `.dm-backup.bin` を別ブラウザに持ち込み
2. `/dm` → "バックアップをインポート" → パスワード入力
3. `/dm` の `<ConversationList />` に既存会話（Alice との履歴）が表示される

---

## 9. 失敗シナリオの手動確認

| シナリオ | 期待動作 |
|---------|---------|
| Bob が鍵を revoke 済みの状態で Alice が送信試行 | Alice UI で "相手はまだ DM を受け付けていません" 表示、MORAL 消費なし (FR-006) |
| Alice 残高不足 (10 MORAL 未満) | "残高不足" 赤バナー、tx 発行なし (FR-006) |
| 本文サイズ 300 KB | 「メッセージが大きすぎます」エラー (FR-013) |
| tx1 送信後にノード停止 | UI にリトライ可能状態で残る、ステルスアカウントに残高は残存 (R8) |

---

## 10. 統合テスト

`pnpm test:integration` の中に `test:dm` を登録済み。以下を自動化:

- `dm-send-receive.sh`: 本 Quickstart の 4–5 を 2 ノード構成で再現
- `dm-stealth-linkage.sh`: 送受信双方のメイン AccountId がオンチェーンに現れないことを Merkle/storage 全量スキャンで確認
- `dm-multi-device.sh`: ブラウザ切替なしにバックアップインポート相当のロジックを CLI で再現し、復号が通ること

```bash
pnpm test:dm
```

---

## 11. 片付け

```bash
pnpm testnet:stop
pnpm testnet:purge
```

---

## トラブルシュート

- **送信が "DM を発行中…" で止まる**: tx2 が finalize していない。`pnpm testnet:status` でブロック生成を確認。
- **受信が 60 秒以上経っても表示されない**: Web Worker が走っていない可能性。`navigator.serviceWorker.controller` やブラウザタブ可視性を確認。
- **バックアップインポート後も会話が空**: インポート直後は `lastScannedBlock = 0` でないこと、または手動で `scanDmInbox({ fromBlock: 0n })` を呼ぶ。
