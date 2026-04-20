# DM 鍵露出に関するセキュリティノート (T085)

**対象機能**: 019-direct-messages
**Constitution 該当条項**: II. *No raw private keys for users*
**Status**: 条件付例外 (CT-1)
**最終更新**: 2026-04-20

---

## 1. 概要

Direct Message (DM) 送信パスは、`packages/wasm-engine/src/dm/encrypt.rs` の
`dm_generate_sender_stealth` (W3) において、新たに生成した sender stealth
Sr25519 keypair の **生 seed (`[u8; 32]`) を JS 境界へ返している**。

これは Constitution II *"ユーザに raw private key を露出させない"* の文面と
構造的に緊張するため、本ドキュメントで例外の理由・補償コントロール・
将来の解消パスを記録する。

仕様側の根拠:

- `specs/019-direct-messages/plan.md` §Complexity Tracking → CT-1
- `specs/019-direct-messages/contracts/wasm-engine-dm-api.md` §W3
- Constitution Check 表 (`specs/019-direct-messages/plan.md` §Constitution Check)
  の II. 欄に ⚠ 条件付 マーク済み

---

## 2. なぜ例外を受け入れるか

| 観点 | 内容 |
|------|------|
| **鍵の寿命** | tx2 (`send_dm`) 1 回の署名のみ。署名直後に JS 側で `Uint8Array.fill(0)` (zeroize) する。永続化、ディスク書き出しなし。 |
| **保護対象の縮小** | sender stealth 鍵が漏洩しても、攻撃者が触れるのは「既に送信済みの sender stealth アカウント上の残高」だけ。送信者のメインアカウント (Sr25519) の残高・識別子・他 pallet 上のステートには到達しない。 |
| **代替の不存在 (MVP 時点)** | polkadot-api の signer API は wallet 保有の main Sr25519 鍵のみを前提とし、「fresh keypair を WASM 側で保持したまま外部 signer に注入する」ルートが MVP 時点で実装されていない。 |
| **影響範囲の局所性** | 例外は DM 送信経路の **1 関数 (W3) に閉じる**。他機能 (ステルス送金、投稿、リアクション) の鍵取扱いは Constitution II の原則を維持。 |

---

## 3. 補償コントロール

CT-1 を受け入れる代わりに、以下のコントロールを実装で担保する。

### 3.1. 短寿命と zeroize

- `dm_generate_sender_stealth` の戻り値 `secret_seed` は、`apps/frontend/src/lib/dm/sender.ts` の `sendDm` 完了時 (成功・失敗いずれも) に `seed.fill(0)` で消去する。
- 関連テスト: `apps/frontend/src/lib/dm/__tests__/sender.zeroize.test.ts`。

### 3.2. メインアカウントとの分離

- sender stealth アカウントは pre-fund tx1 で必要最小残高のみを受け取り、tx2 で全消費する。残高が事後に残らないため、key 漏洩時の被害は実質ゼロ。
- on-chain 観測者から見て、tx1→tx2 の結合は anonymity-set 込みのステルス送金経由で隠蔽される。

### 3.3. プレーンテキスト保持禁止

- WASM 内では `secret_seed` 以外 (signing key 派生物、shared secret 等) を JS 境界へ漏出させない。
- 平文メッセージ・stealth 受信鍵 (`x_sk`) は **常に WASM スコープ内** で生成し、署名・暗号化処理を完結させる。

---

## 4. 将来の解消パス

CT-1 は MVP 上の妥協であり、Polish フェーズ以降で解消する。優先度順:

### Option A (優先): `SubstrateSignerWasm` トレイトの導入

- `packages/wasm-engine/src/dm/` 内に `SubstrateSignerWasm` トレイトを実装する。
- WASM 側で seed を保持したまま、JS 側へは `sign_extrinsic_payload(bytes) -> signature` のみを露出する。
- 採用条件: polkadot-api の signer 統合方式が固まり、外部 signer インジェクションのインタフェースが安定したタイミングで再評価。

### Option B: pallet-messaging へ delegate origin 拡張

- `pallet-messaging::send_dm` の origin を「メインアカウントが delegate 委任」で受け付けるよう、runtime に `pallet-proxy` / `pallet-multisig` 類似機構を追加する。
- これにより tx2 をメインアカウント署名で発行できる (sender stealth 鍵不要)。
- 検討課題: on-chain 観測者から「main → DM」の結合が見える可能性があり、FR-024 (送信元匿名性) と慎重に照合する必要がある。

---

## 5. レビュー判定とトリガ

CT-1 例外は **以下のいずれか** が満たされた時点で再レビュー必須とする。

- polkadot-api が WASM 内 signer API (`SubstrateSignerWasm` 相当) を公式サポートした
- `pallet-proxy` / `pallet-multisig` 類似の delegate 機構が runtime に導入された
- DM 送信経路で seed 寿命が「tx2 1 回」を超える要件が発生した (例: バッチ送信)

---

## 6. GA gating

GA (General Availability) 公開前に **外部暗号レビュー** (SC-003) を完了することを必須とする。
本ドキュメントは GA gating の一次資料として参照され、レビューの観点と完了条件は以下のチェックリストで管理する。

### 6.1. 外部レビュー観点 (SC-003 — T096 トラッキング)

- [ ] **Padding bucket leakage**: `DM_PADDING_BUCKETS` の bucket 選択が body length / sender 識別をリークしないか。
  - 確認対象: `packages/wasm-engine/src/dm/padding.rs::select_padding_bucket`
- [ ] **AAD construction**: `dm_encrypt_and_pad` の AAD (Additional Authenticated Data) が `DM_PROTOCOL_VERSION`、recipient stealth address、ephemeral public key を含み、cross-protocol confusion を防いでいるか。
  - 確認対象: `packages/wasm-engine/src/dm/encrypt.rs`
- [ ] **KDF inputs**: HKDF の `salt` / `info` が `DM_PROTOCOL_VERSION` でドメイン分離されているか。recipient stealth と sender stealth の派生 path が衝突しないか。
  - 確認対象: `hkdf_okm` (`packages/wasm-engine/src/dm/encrypt.rs`)
- [ ] **Sender-stealth seed lifecycle**: 本ドキュメント §3.1〜§3.2 の補償コントロールがコード上で確実に発動するか (zeroize, ephemeral 残高ゼロ化)。
  - 確認対象: `apps/frontend/src/lib/dm/sender.ts`、`apps/frontend/src/lib/dm/__tests__/sender.zeroize.test.ts`
- [ ] **Receipt body framing**: read/delivered receipt の MAGIC + kind + refMessageId フォーマットが受信側で confusion 攻撃を生まないか。
  - 確認対象: `apps/frontend/src/lib/dm/receipt.ts`

### 6.2. 完了条件

- 上記 5 項目すべてに **independent reviewer (Anarchy コミッタ外) のサインオフ** がある。
- レビュー結果と finding は本ドキュメントに追記する。
- CRITICAL finding が残っている間は GA をブロックする。

GA gating トラッキングイシューは T096 を参照。
