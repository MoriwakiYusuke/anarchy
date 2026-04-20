# `DmMessagesByRoot` Storage Growth Operations Guide (T086)

**対象機能**: 019-direct-messages
**該当 storage**: `pallet_messaging::DmMessagesByRoot`
**仕様参照**: `specs/019-direct-messages/data-model.md` §1.4 (M1)
**最終更新**: 2026-04-20

---

## 1. 課題サマリ

`DmMessagesByRoot` は **MerkleRoot ([u8; 32]) → message_id (u64) の StorageMap** で、
DM の "storage-layer replay 防止" のために存在する (同一 MerkleRoot を持つ
ciphertext が 2 回 `send_dm` されることを pallet レベルで拒否する)。

このマップは MVP では **単調増加** する:

- 1 エントリ = 32 B (MerkleRoot) + 8 B (message_id) ≒ **40 B**
- マップ overhead や trie 系 metadata を含む実効サイズは 1 エントリあたり **~80–120 B** と見積もる
- mainnet で **年間 1000 万 DM** を仮定した場合の年間増分:
  - 純データ: 約 **400 MB / 年**
  - trie overhead 込み: 約 **800 MB – 1.2 GB / 年**

`DmDispatchesByBlock` には Phase 3.4 の popularity-driven GC (FR-018) が入る予定だが、
**`DmMessagesByRoot` の GC は MVP では実装されていない**。Phase 3.4 で `on_initialize`
内に「GC 済み DmDispatch に対応する DmMessagesByRoot エントリも削除する」ロジックを
同時導入することで、`DmMessagesByRoot` の寿命を `DmDispatchesByBlock` に追随させる。

---

## 2. 運用上のオペレータガイダンス (Phase 3.4 までの暫定)

### 2.1. ノード種別ごとの選択

| ノード種別 | 推奨設定 | 理由 |
|------------|----------|------|
| **Validator (block author)** | フルノード + state pruning OFF | `DmMessagesByRoot::contains_key` を `send_dm` extrinsic 内で参照するため、最新 state は必須。 |
| **Archive node** | フルノード + `--state-pruning archive` | DM 履歴の長期保管。**`DmMessagesByRoot` の単調増加分** + `DmDispatchesByBlock` (GC まで) を抱える。年 1〜2 GB の disk 増分を見込む。 |
| **Light client (smoldot)** | 既定 | フロント側 (`apps/frontend`) は smoldot 経由で `DmScanApi::dispatches_at` のみを叩くため、`DmMessagesByRoot` 全体を保持しない。影響なし。 |
| **Storage node (apps/storage-node)** | 既存通り | DM 鍵フラグメントは持つが、pallet ストレージは保持しない。本ガイドの対象外。 |

### 2.2. ディスク容量プランニング

mainnet 想定 (年 1000 万 DM):

```text
DmMessagesByRoot:        ~ 400 MB / 年 (純データ)
+ trie overhead:         ~ 400-800 MB / 年
DmDispatchesByBlock:     ~ 256 KB × 256 dispatches/block × 14400 blocks/day
                         ≈ 上限到達時 940 MB / 日 (実際は ciphertext_len 次第で大きく下回る)
```

**結論**: archival operator は最低 **年 50 GB** の disk 余裕を確保する (`DmDispatchesByBlock` GC 前)。

### 2.3. 監視メトリクス (推奨)

ノードに Prometheus exporter を入れている運用者向け:

- `substrate_storage_state_db_size_bytes` で state DB の総サイズを継続観測
- 月次で `DmMessagesByRoot` のキー数を `state.getKeysPaged` で counting し、想定線 (3-4 万件/日) を超えていないか確認

### 2.4. 警告発火条件 (推奨ルール)

| 条件 | 推奨アクション |
|------|----------------|
| state DB 増分 > 5 GB / 月 | `DmMessagesByRoot` のキー数を実測。想定の 2x を超えていれば調査。 |
| state DB > disk 容量の 80% | archive node の disk を拡張、または non-archive (state pruning ON) ノードへ切替を検討。 |

---

## 3. Phase 3.4 GC 移行時のオペレータ向け変更点

Phase 3.4 で popularity-driven GC が入ると `DmMessagesByRoot` も GC 対象になる:

- GC 後は **過去の MerkleRoot に対する replay 防止が外れる** → 同じ MerkleRoot を持つ ciphertext を再度 `send_dm` できる状態になる。
- これは「本質的に同一の暗号文を再投稿できる」だけで、ユーザ視点では新しいメッセージとして扱われる。spec 上は許容される挙動 (FR-018 と整合)。
- archival operator は GC 前後の state 比較を取って、historical scan 経路に retention policy を別途定義する。

詳細実装は 019 後継 spec / Phase 3.4 リリースノートで案内予定。

---

## 4. 関連ドキュメント・コード

- `specs/019-direct-messages/data-model.md` §1.4 — DmMessagesByRoot のスキーマと M1 リスク記述
- `apps/blockchain/pallets/messaging/src/lib.rs` — Storage 宣言と `send_dm` での `contains_key` 参照
- `apps/blockchain/docs/tor-deployment.md` — ノード起動と pruning 設定の汎用ガイド
- `docs/security/dm-key-exposure.md` — DM 全体のセキュリティノート (T085)

---

## 5. 変更履歴

| 日付 | 変更内容 |
|------|----------|
| 2026-04-20 | 初版作成 (T086, MVP 時点の単調増加ガイダンス) |
