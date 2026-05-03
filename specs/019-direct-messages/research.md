# Phase 0 Research: Direct Messages

**Feature**: 019-direct-messages
**Date**: 2026-04-20

仕様書 (spec.md) で明示的/暗黙的に残っていた技術的判断事項を列挙し、MVP の実装前に確定させる。各項目は **Decision / Rationale / Alternatives** の形で記録する。

---

## R1. 送信者ステルスアカウントの導出方式

**Question**: FR-024 で必須となった「送信者もステルスアカウント経由で DM を発行する」を、どのようなキーマテリアルから生成するか。

**Decision**:
- 送信者は DM 送信ごとに **新鮮な Sr25519 キーペアをクライアント側で生成**（`schnorrkel::Keypair::generate_with(OsRng)` 相当を wasm-engine 側で実装）。キーペアは受信者側の stealth 導出（Ed25519 Edwards 点加算）とは独立であり、導出計算を伴わない純粋ランダム生成である。
- キーペアは**一時的**で、送信完了後にブラウザメモリから破棄。永続化しない。
- 送信者メインアカウントからこの新鮮ステルスアカウントへの送金は **既存の `pallet_stealth::send_to_stealth`** を流用（ephemeral_pubkey は pre-fund 用ダミーとして純粋ランダム X25519 公開鍵を指定。既存 stealth reward スキャナは DH 後 `expected_stealth != stored` で弾くので誤検知しない）。
- `send_to_stealth` のトラフィックはステルス報酬と DM pre-fund の双方を含むため、**オンチェーン観測者から pre-fund と stealth reward の区別がつかない**。これが匿名集合を広げる副次効果を持つ。

**鍵タイプ整理 (重要)**:

| 要素 | 鍵種別 | 役割 | 所有者 |
|------|--------|------|--------|
| 送信者メインアカウント | Sr25519 | tx1 (`send_to_stealth`) 署名・envelope 内 FR-004 認証署名 | 送信者 |
| 送信者ステルス鍵 (本 R1) | **Sr25519 (新鮮)** | tx2 (`send_dm`) 署名専用 | 送信者が一時生成・破棄 |
| 受信者 DM メタアドレス `scan_pub` | X25519 | 送信側 ECDH 入力 | 受信者公開 |
| 受信者 DM メタアドレス `spend_pub` | Ed25519 (compressed Edwards) | 受信者 stealth address の Edwards 点加算起点 | 受信者公開 |
| 受信者 stealth address (on-chain) | Ed25519 (AccountId32) | `pallet-messaging::DmDispatch.recipient_stealth` | 送信者が導出、受信者のみ秘密鍵を再構成可 |
| エフェメラル鍵 `eph_pub`/`eph_priv` | X25519 | ECDH 源 | 送信者が DM ごとに生成、`eph_priv` は即時破棄 |

**Rationale**:
- 再利用型 stealth account (sender が複数 DM を同じアカウントから送る) は、送信者の参加グラフを観測者に部分的に復元させるため却下。
- 送信者ステルスは「受信者に紐づかず送信者の main とも紐づかない」独立ランダム Sr25519 で十分、かつ Substrate 標準 signer (polkadot.js / Nova) が Sr25519 を直接扱えるため wallet signer を経由した tx2 署名フローが成立する。Ed25519 で統一する案は wallet signer 対応が薄く却下。
- 受信者側 stealth の Ed25519 は既存 016 (`packages/wasm-engine/src/stealth/address.rs`) と完全互換にするため変更しない。Substrate `AccountId32` は Sr25519/Ed25519 のいずれの 32 バイト公開鍵も受け付ける。
- 既存 `pallet-stealth::send_to_stealth` を使うことで pallet を新設せずに済み、かつステルス送金トラフィックと mix できる。

**Alternatives Considered**:
- **HD ウォレット様の階層派生（送信者のシード + nonce）**: キーの再生成が可能になる利点があるが、送信者シードからの派生は「送信者主アカウントとステルスアカウントを結びつける鍵マテリアル」を生み、漏洩時の事後追跡を可能にする。MVP ではキーペアそのものを破棄する方針のほうがシンプルかつ安全。
- **既存のステルス spend key ペアを流用**: 送金用と DM 送信用で鍵を分ける原則に反する。却下。
- **plain `pallet_balances::transfer_keep_alive` で pre-fund**: ephemeral_pubkey 登録をスキップできるが、ステルス送金と見分けがつくため匿名集合を縮める。採用せず。

---

## R2. 受信者ステルスアドレスの導出方式

**Question**: 受信者側のステルスアドレスをどう導出するか (既存 016-stealth-address との互換性含む)。

**Decision**:
- 既存 `packages/wasm-engine/src/stealth/address.rs::derive_stealth_address(meta_address: &str)` の **EIP-5564 互換導出**をそのまま流用。ただし既存 API は `ephemeral_pub` を関数内部で生成するので、DM では「ephemeral_pub を外部から与えられる薄いラッパ」を新設する (`dm_derive_recipient_stealth(scan_pub, spend_pub, ephemeral_pub)` — W5 参照)。内部計算は `P_stealth = K_spend + H(X25519(eph_priv, K_scan)) * G` で既存と完全一致。
- 受信側の stealth 検証には既存の `packages/wasm-engine/src/stealth/scan.rs::scan_transaction(view_key, ephemeral_pub, stealth_pub, spend_pub)` を流用 (シグネチャが DM の必要と一致)。
- 導出結果の `stealth_pub` は**Ed25519 compressed Edwards point (32 bytes)**。Substrate `AccountId32` 互換。受信者のみが `spend_priv` から stealth の Ed25519 秘密鍵を再構成できる (既存実装と同じ)。
- DM 専用に**受信メタアドレスを別途発行**する。理由は用途分離（R5 参照）。
- 送信側クライアントは `pallet-messaging::DmReceptionKeys(recipient_account)` から受信者の DM 用メタアドレスを取得し、fresh ephemeral を生成して stealth address を導出する。

**Rationale**:
- 暗号プリミティブとエンコーディングは 016 で既にテスト済み。再実装はバグ面で危険。
- 用途別メタアドレス分離により、DM 鍵漏洩が stealth reward 鍵まで波及しない（逆も同様）。

**Alternatives Considered**:
- **ステルス報酬のメタアドレスを DM でも共用**: 鍵管理を 1 本化できるが、上記の cross-contamination リスクあり。却下。

---

## R3. E2EE の対称暗号スイート

**Question**: DM 本文の暗号化に使う対称暗号とパディング。TODO.md §3.3 では ChaCha20-Poly1305 + HKDF + 固定サイズパディングが挙がっている。Constitution Technology Stack は AES-256-GCM を記載。

**Decision**:
- **AES-256-GCM**（Constitution 準拠）。既存の `packages/wasm-engine/src/stealth/backup.rs` が既に AES-256-GCM を WASM 経由で使用しており、同じクレートを再利用。
- **鍵導出 (KDF)**: HKDF-SHA256。入力: X25519 DH 共有秘密。info パラメータに `"anarchy-dm-v1"` + recipient stealth address + sender ephemeral pubkey をバインドし、ドメイン分離を確保。
- **nonce**: X25519 共有秘密から HKDF で派生する 96bit nonce（送信 1 DM = 1 暗号化なので決定論的派生で可、ただし domain-separated して stealth reward の nonce と衝突しないようにする）。
- **AAD**: recipient stealth address || sender ephemeral pubkey || padded_length || protocol_version。

**Rationale**:
- Constitution と既存コードベースの整合。ChaCha20-Poly1305 に切り替えると新規依存 (chacha20poly1305 クレート) が必要で、監査・SBOM にも影響。
- GHASH の定数時間実装は `aes-gcm` クレートで担保済み。

**Alternatives Considered**:
- **ChaCha20-Poly1305 (TODO.md §3.3 の原案)**: タイミングサイドチャネル耐性が高いとされるが、Constitution が AES-256-GCM を標準にしており、WASM コードサイズと監査複雑度の観点で MVP 採用は見送り。将来スイートを増やす場合は `protocol_version` 切り替えで対応可能な設計にする。

---

## R4. 固定サイズパディングのバケット設計 (FR-026)

**Question**: 送信本文のパディング先となる canonical size セットの具体値。

**Decision**:
- **バケット**: `{1 KB, 4 KB, 16 KB, 64 KB, 256 KB}` (5 段階、4 倍刻み)。
- **選択規則**: envelope (SCALE 符号化, ~105 byte overhead) + ISO 7816-4 padding + AES-GCM tag (16 B) の総和が最小バケット ≥ となるものを選ぶ。具体的には `padded_plaintext_len + 16 <= bucket` を満たす最小バケット。
- **パディング方式**: ISO/IEC 7816-4 スタイル (0x80 + `0x00` 詰め)。復号後は末尾の 0x80 までを除去して平文を得る。
- **上限超過**: 256 KB バケットを超える本文は FR-013 に従って送信前に拒否 (既存 post パイプラインのフラグメント上限と一致)。

**Effective Body Capacity** (参考):

| Bucket (ciphertext_len) | Padded plaintext | Envelope fixed overhead* | Max body bytes | 日本語 UTF-8 換算 |
|-------------------------|------------------|--------------------------|----------------|-------------------|
| 1 KB (1024) | 1008 | ~108 | ~899 | ~300 文字 |
| 4 KB (4096) | 4080 | ~108 | ~3971 | ~1323 文字 |
| 16 KB (16_384) | 16_368 | ~108 | ~16_259 | ~5419 文字 |
| 64 KB (65_536) | 65_520 | ~108 | ~65_411 | ~21_803 文字 |
| 256 KB (262_144) | 262_128 | ~108 | ~262_019 | ~87_339 文字 |

*Envelope fixed overhead = version(1) + sender_account(32) + timestamp(8) + body_len prefix(SCALE compact, 1–4 B) + signature(64) + 1-byte ISO 7816 terminator ≒ 108 bytes。実測値は実装時に固定する。

**Rationale**:
- **256 B バケットは不採用**: envelope 固定オーバーヘッド ~108 B + AES-GCM tag 16 B で実効本文が `256 - 108 - 16 - 1 ≈ 131` byte、日本語約 43 文字しか入らない。このバケットを独立クラスとして観測されると「短文クラス＝メッセージ性が薄い」を第三者に推測される弱いリーク源となり、かつユーザー便益が限定的。1 KB に統合することで実効 ~900 byte (日本語 ~300 文字) が下限となり、「1 sentence 程度の DM」から「複数段落」までを 1 クラスにまとめられる。
- 4 倍刻みは観測者から見える長さ情報を 2–3 bit に抑えつつ、典型 DM (数十〜数千 byte) で無駄帯域を抑えるバランス点。
- 既存 post パイプラインが 256 KB/フラグメントなので、最大バケットを 256 KB に揃えると分散ストレージ側で特別扱いが不要。
- ISO 7816-4 パディングは AES-GCM の付加データと干渉しない（GCM は任意長入力を取る）ので、padding と暗号化の順序を「pad → encrypt」に固定できる。

**Alternatives Considered**:
- **256 B を残す**: 却下理由は上記。再検討するなら envelope から `signature` 等を外部メタデータに出す必要があるが、FR-003 (参加者の匿名集合) を壊すため不可。
- **常に 256 KB にパディング**: 最大の匿名性だが、短い DM でも帯域/fee コストが跳ね上がる。UX/経済性で現実的でない。
- **2 倍刻みバケット (512, 1K, 2K, 4K, ...)**: 刻みが細かく漏洩情報が増える。4 倍刻みで十分。

---

## R5. 受信鍵の公開機構と pallet 分離

**Question**: DM 受信鍵 (メタアドレス) をどのようにチェーンに公開するか。`pallet-stealth` を拡張するか新 pallet を作るか。

**Decision**:
- **新規 `pallet-messaging` に独立した StorageMap を持たせる**: `DmReceptionKeys: StorageMap<AccountId, DmMetaAddress>`。
- 公開/更新は `publish_dm_key(meta_address)` extrinsic、取消は `revoke_dm_key()` extrinsic。どちらも送信者メインアカウントが署名する（公開鍵は公開情報なので送信者ステルス経由は不要）。
- `DmMetaAddress` 型は既存 `packages/wasm-engine/src/stealth/types.rs` と同じ frame-encoded 表現 (scan_pub: [u8; 32], spend_pub: [u8; 32])。

**Rationale**:
- `pallet-stealth` の責務はステルス送金。DM 受信鍵の publication はメッセージング層の責務であり、pallet 分離が Substrate のイディオムに沿う。
- 独立すればテスト・アップグレード・重み定義が干渉しない。
- 受信鍵が公開情報であるため、送信者と関連付けられても構わない (メタアドレスは受信者が公開を望むもの)。

**Alternatives Considered**:
- **`pallet-stealth` に同居**: パレット責務が曖昧になり、ステルス送金単独のテスト・アップグレードが困難になる。却下。
- **`pallet-identity` 相当を別途新設して「サービス鍵レジストリ」にする**: 将来他の用途 (暗号化音声等) にも共有できるが MVP スコープ外。FR-019 に従い 1:1 DM のみなので専用 pallet で十分。

---

## R6. 送信者認証の実現手段 (FR-004)

**Question**: 受信者が復号後に「Alice が本当に送ったか」を検証する仕組み。

**Decision**:
- **inner envelope に Sr25519 署名を同梱**。署名対象: `blake2b(0x01 || sender_main_account || recipient_stealth || sender_ephemeral_pubkey || timestamp_ms || blake2b(body))` (W6 で決定論的計算)。署名鍵は送信者メインアカウントの Sr25519 秘密鍵 (既存 Polkadot.js / Nova / Talisman ウォレット signer の `signRaw` 経由で取得、app 層に生鍵を持たない — Constitution II 準拠)。
- 受信者は復号後に envelope から `sender_main_account` と `signature` を取り出し、オンチェーンのアカウント存在検証 (`frame_system::Account` で存在確認) と **Sr25519** signature verify (`schnorrkel::verify`) を行う。
- **匿名送信の扱い**: MVP では許さない (必ず送信者メインアカウントの署名を含む)。将来、匿名 DM (zk proof ベース等) を入れる場合は新 extrinsic `send_dm_v2` で追加する (プロトコルバージョニングは call_index で分岐、`send_dm` 引数には含めない — M2 参照)。

**三つの鍵系統が共存することの整理 (N1 対応)**:

本機能には用途の異なる 3 つの鍵タイプが並走する。実装者はこれらを混同しないこと。

| 鍵 | タイプ | 用途 | 生成/管理 | 使用するライブラリ |
|----|--------|------|-----------|-------------------|
| 送信者メインアカウント | **Sr25519** | envelope 内部署名 (FR-004) / tx1 (`send_to_stealth`) 署名 | ウォレット (Polkadot.js / Nova / Talisman) が所有 | `schnorrkel` (署名検証のみ wasm-engine) |
| 送信者ステルス鍵 | **Sr25519** (新鮮) | tx2 (`send_dm`) 署名専用、1 回限り | wasm-engine `dm_generate_sender_stealth` が `OsRng` から生成 | `schnorrkel` (wasm-engine 内部のみ) |
| 受信者 DM スキャン鍵 | **X25519** | ECDH (shared secret 導出) | wasm-engine `stealth::keys` で生成、暗号化バックアップで端末間移送 | `x25519_dalek` |
| 受信者 DM spend 鍵 | **Ed25519** (compressed Edwards) | stealth address 導出の K_spend | wasm-engine `stealth::keys` で生成 | `curve25519_dalek` + `ed25519_dalek` |
| 受信者 stealth address (on-chain) | **Ed25519** compressed Edwards (32B) | `pallet-messaging::DmDispatch.recipient_stealth` (AccountId32) | 送信者が EIP-5564 で導出 | `curve25519_dalek` (既存 `stealth::address`) |
| ephemeral 鍵 | **X25519** | DH ソース | 送信 1 件ごとに wasm-engine 内部で生成・即時破棄 (FR-021) | `x25519_dalek` |

**重要**: 受信者の stealth 導出は **Ed25519 基底 (既存 `packages/wasm-engine/src/stealth/address.rs::derive_stealth_address`)** を変更せず流用する。FR-003 の「pallet-stealth と同じ導出ロジック」要件を満たすため、DM では新しい導出プリミティブを導入しない。envelope 署名 (Sr25519) と stealth 導出 (Ed25519) が別の曲線を使うのは Substrate の標準パターンと一致する (メインアカウントは Sr25519、stealth 関連はステルス reward と同様 Ed25519 基底)。

**Substrate AccountId32 との整合**: AccountId32 は 32 バイトの public key バイナリであり、Sr25519 / Ed25519 のいずれの公開鍵もそのまま代入できる。受信者 stealth (Ed25519) と送信者メイン (Sr25519) の同居は Substrate の既存設計で許容されている。

**Rationale**:
- 既存ウォレット (polkadot.js / Nova) の signer 経由で署名できる Sr25519 は、Constitution の Minimal Key Exposure に適合する唯一の現行パス。
- envelope は ciphertext の内側なので、署名および送信者アカウント情報は第三者には見えず、FR-003 を損なわない。
- 署名対象に recipient_stealth_address と sender_ephemeral_pubkey を含めることで、envelope を別の DM に流用する (copy) 攻撃を防ぐ。

**Alternatives Considered**:
- **Ring signature / zk proof**: 完全匿名送信が可能だが MVP スコープ外。後続機能で検討。
- **MAC (共有秘密ベース)**: 送受信者間の一意性は保てるが、受信者が第三者に「Alice が送った」と証明できない (MAC は対称鍵)。UX 上は問題だが、DM では第三者証明は不要なのでこれも選択肢。ただし Sr25519 署名を使ったほうが外部の UI (例: 報告機能) と整合しやすいので採用せず。

**Wallet Signer 実現性 (要確認項目)**:

本方式は「送信者メインアカウントの Sr25519 秘密鍵で envelope 内部ハッシュを raw-message 署名」する。ウォレット signer 側でこの raw-message 署名が可能かはウォレットごとに差があるため、MVP 対応と妥協オプションを整理する。

| Signer | Raw-message Sr25519 署名の可否 | MVP 時の扱い |
|--------|--------------------------------|--------------|
| Polkadot.js extension | `signer.signRaw({ data, type: 'bytes', address })` が利用可。extension が対応している。 | **主サポート** |
| Nova Wallet / Talisman | `signRaw` 相当を実装しており、Polkadot.js 互換 API で利用可 (2025 時点で確認済み)。 | サポート |
| WebAuthn / Secure Enclave based signer (Constitution II 長期目標) | 現行スタックでは生 blake2b ハッシュに対する Sr25519 署名をエンクレーブから直接取得する API が未整備。ハードウェアエンクレーブは ECDSA-P256 が一般的で、Sr25519 用の専用実装が必要。 | **MVP では未対応。代替として後述の "offline-signed envelope" オプション。** |

**WebAuthn/SE signer 未対応時のフォールバック (将来拡張)**:
- `protocol_version = 2` で `signature_scheme` フィールドを envelope に追加し、ECDSA-P256 (WebAuthn) やリング署名スイートを選択できるよう設計余地を残す。
- MVP では Polkadot.js / Nova / Talisman の 3 ウォレットをサポート対象とし、上記以外では DM 送信機能を UI で無効化する。
- `contracts/wasm-engine-dm-api.md` W6 で決定論的ハッシュ計算を提供することで、どの signer 実装でも同一ハッシュを署名すれば良く、signer 切替時の影響を最小化。

**実装時のテスト要件**:
- `apps/frontend/src/lib/dm/sender.ts` のサポート signer 判定ロジックに対する Jest ユニットテスト。
- quickstart.md § 4 に「wallet 選択が Polkadot.js / Nova / Talisman のいずれかであることを前提」と追記する (quickstart 更新は Phase 1 末尾で実施)。

---

## R7. スキャナーの性能設計 (SC-002 / SC-004)

**Question**: 受信者が自分宛 DM を見つけるためのスキャン戦略。

**Decision**:
- **既存 `apps/frontend/src/lib/stealth/scanner.ts` + `worker.ts` を継承し、DM 用モジュール `apps/frontend/src/lib/dm/scanner.ts` を派生させる**。
- ブロック単位で `pallet-messaging::EphemeralKeys(block_number)` と `pallet-stealth::EphemeralKeys(block_number)` の両方を取得し、ephemeral_pubkey × recipient scan_priv で stealth address を導出 → 自分の DM メタアドレス由来と一致すれば自分宛。
- **スキャン範囲**: 初回ログイン時はユーザー指定 (既定: 過去 7 日分 ≒ 600,600 ブロック程度を想定、要計測)。継続スキャンはラストスキャン位置から finalized head まで。
- **インデックスキャッシュ**: `IndexedDB` (既存ステルス scanner と同じ) に { block_number, matched_stealth_addresses[] } を保存し、再ログインで再スキャンしない。
- **バックグラウンドスキャン**: Web Worker 内で実行。Page Visibility API でタブがフォアグラウンドでない場合はスキャン間隔を 30 秒 → 5 分に自動低減 (既存パターン、Reaction mining と同じ考え方)。

**Rationale**:
- DM ephemeral と stealth reward ephemeral を分けて登録することで、スキャナがどちらも同じループで処理できる反面、片方のみ興味がある場合の無駄が出ない。
- 1 ブロック 100 ephemeral × X25519 DH ≈ <100 ms/ブロック (既存計測に近い数値)。1000 ブロックでも 100 秒のフル処理。継続スキャンでは差分のみなので SC-002 の 60 秒以内視化は十分達成可能。

**Alternatives Considered**:
- **中央集約インデクサ**: プライバシー原則に反するため却下。
- **チェーン側で受信者フィルタリング**: FR-003 を破る (受信者が誰かを pallet が知ってしまう)。却下。

---

## R8. 送信経路とトランザクションフロー

**Question**: 送信者クライアントが DM を送るときの厳密な順序とエラーハンドリング。

**Decision**:

順序:
1. 送信者クライアントで Sr25519 fresh keypair を生成 (sender_stealth)
2. padded ciphertext を wasm-engine の `dm_encrypt()` で生成、fragments (k/n SSS + Merkle) を計算
3. フラグメントを storage-node 群に並列アップロード (既存 post パイプライン経由、`pallet-storage` の submit_fragment)
4. MerkleRoot / k / n / total_size が storage layer から確定したら、`pallet-stealth::send_to_stealth(sender_stealth_address, random_ephemeral, pre_fund_amount)` を送信者メインアカウントから発行 (tx1)
5. tx1 の finalization を待つ (一般に 2 ブロック ≈ 6 秒)
6. `pallet-messaging::send_dm(recipient_stealth_address, sender_ephemeral_pubkey, content_ref)` を sender_stealth アカウントから発行 (tx2)
7. tx2 finalize で完了。UI は "sent" ステータス表示。

**Rationale**:
- tx1 → tx2 の逐次実行が必要 (tx1 完了後でないと sender_stealth に残高がない)。
- SC-001 (≤15 秒) は 2 ブロック × 2 = 約 12 秒を想定。マージン 3 秒。
- フラグメントアップロード失敗時の処理: 送信前に k 個以上のノードが ACK を返すまでリトライ、ACK が集まらなければクライアント側で abort (主アカウントからの MORAL 消費なし)。tx1 送信後の失敗はステルスアカウント上の残高が残る (ユーザーが後で再利用 / 無視)。

**Error Handling**:
- tx1 失敗 (残高不足など): UI でエラー、state = "failed-prefund"。MORAL は消費されない。
- tx2 失敗 (たとえば受信鍵が revoke された、コンテンツ参照重複など): UI は warn、MORAL は tx1 ぶんロック済み。ロックされた MORAL は sender_stealth に残るが、本人しか取り戻せない (既存ステルスアカウントの仕組みで spend 可能)。

**Alternatives Considered**:
- **フラグメントアップロードと tx1 を並列化**: 完了タイミングが噛み合わず、race condition 発生リスク。MVP では逐次。
- **tx1 / tx2 を 1 つの extrinsic に合成**: 送信者メインアカウントが tx1 を署名しなくてはならず、FR-024 を破る。却下。

---

## R9. Block リストのストレージ位置

**Question**: FR-011 のブロックリストをオンチェーンに置くかローカルに置くか。

**Decision**:
- **ローカルのみ** (ブラウザ IndexedDB)。暗号化バックアップ (FR-022) の中に同梱してエクスポート。
- pallet-messaging にはブロックリストに関する storage / extrinsic を一切追加しない。

**Rationale**:
- ブロックリストをオンチェーンに置くと「X が Y をブロックした」がパブリックレコードとなり、FR-003 の「参加者エニュメレート不可」を破る。
- クライアント側でフィルタリングすれば透明性・配送ロジックを汚さずに実装できる。

**Alternatives Considered**:
- **暗号化してオンチェーンに置く**: 鍵管理の複雑度が上がるうえ、オンチェーンストレージコストが発生。メリットなし。

---

## R10. 既存スクリプト / SBOM / 運用への影響

**Question**: testnet スクリプトや SBOM に影響する追加項目。

**Decision**:
- `pnpm testnet:start` / `stop` には変更不要（新 pallet はランタイム内）。
- `pnpm test:integration` に `test:dm` を追加し、新規シェルテスト 3 本を登録。
- 新規 Rust クレート依存: 既に workspace に存在 (ark-bls12-381, rs_merkle, aes-gcm, x25519-dalek)。追加依存はなし。
- wasm-engine に `dm/` モジュールを追加するが、`wasm-pack build` のエントリは不変。

**Rationale**:
- 新規暗号依存を入れないので SBOM 変更は最小。
- フロントエンド側は既存 polkadot-api 依存の範囲内。

**Alternatives Considered**: (なし)

---

## Summary of Open Items → 解決済

spec.md に残っていた以下の技術的曖昧点は、本 research で全て決定済み:

| 項目 | 参照 FR | 決定 |
|------|---------|------|
| 送信者ステルス導出 | FR-024 | R1: Sr25519 fresh keypair + 既存 `send_to_stealth` で pre-fund |
| 受信者ステルス導出 | FR-003 | R2: 016 と同じ EIP-5564 導出、DM 専用メタアドレス |
| 対称暗号スイート | FR-014/020 | R3: AES-256-GCM + HKDF-SHA256 |
| パディングバケット | FR-026 | R4: {256B, 1K, 4K, 16K, 64K, 256K} 4 倍刻み |
| 受信鍵公開場所 | FR-015 | R5: 新 `pallet-messaging::DmReceptionKeys` |
| 送信者認証 | FR-004 | R6: inner envelope 内 Sr25519 署名 |
| スキャナ設計 | SC-002/004 | R7: 既存 stealth scanner 派生 + Web Worker |
| 送信フロー | FR-024/025 | R8: 2 段 extrinsic (pre-fund → send_dm) |
| ブロックリスト配置 | FR-011 | R9: ローカルのみ + 暗号化バックアップに同梱 |
| 運用影響 | — | R10: 追加暗号依存なし、integration test 3 本追加 |

Phase 1 (Design & Contracts) に進める状態。
