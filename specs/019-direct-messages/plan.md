# Implementation Plan: Direct Messages (DM)

**Branch**: `019-direct-messages` | **Date**: 2026-04-20 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/019-direct-messages/spec.md`

## Summary

Anarchy における 1:1 の E2E 暗号化ダイレクトメッセージを、既存のステルスアドレス基盤 (`pallet-stealth`, `packages/wasm-engine/src/stealth/`, フロントエンドの `lib/stealth/` + `components/stealth/`) と投稿パイプラインの分散ストレージ (`pallet-storage` + storage-node) を流用して実装する。新設する `pallet-messaging` はコンテンツ参照 (MerkleRoot/k/n/size + ephemeral pubkey) のみをオンチェーンに記録し、本文は投稿と同様に分散ストレージに断片化して保存する。送信者・受信者の両方をステルスアドレスで隠蔽 (FR-003/FR-024)、暗号化は送信ごとに新鮮な ephemeral 鍵 × 受信者長期鍵の DH + HKDF + AES-256-GCM (FR-020)、本文は送信前に固定サイズバケットへパディング (FR-026)、受信者キーは既存のステルス用バックアップと同じ暗号化バックアップ機構で多端末間を移動する (FR-022)。cover-traffic (FR-027) と Phase 3.4 の人気度ベース GC (FR-018) は本フィーチャーから明示的に外れる。

## Technical Context

**Language/Version**: Rust stable (pallet + wasm-engine, toolchain: `wasm32v1-none` + `rust-src` per `apps/blockchain/rust-toolchain.toml`) / TypeScript (Next.js 14 App Router, React 18)
**Primary Dependencies**: Polkadot SDK stable2503 (FRAME), PAPI (polkadot-api), `ark-bls12-381` (既存 KZG 基盤), `rs_merkle`, `aes-gcm` + `x25519-dalek` (既存ステルス基盤), `wasm-pack`
**Storage**: オンチェーン (SCALE エンコードの StorageMap: DM メタアドレス・メッセージ参照・ディスパッチ履歴) / オフチェーン (分散ストレージノード: 既存 pallet-storage フラグメント)
**Testing**: `cargo test -p pallet-messaging` (pallet 単体 + mock runtime), `wasm-pack test` (wasm-engine の新 DM モジュール), Jest (フロントエンド), `pnpm test:integration` の新規追加 (複数ノード間での送受信)
**Target Platform**: Linux (ブロックチェーンノード), WASM (wasm-engine), モダンブラウザ (Frontend); モバイル対応は別フィーチャー
**Project Type**: 既存 pnpm + Cargo 二層モノレポ (blockchain / storage-node / frontend / wasm-engine)
**Performance Goals**: 送信完了 ≤ 15 秒 (stealth pre-funding 込み, SC-001), 受信可視化 ≤ 60 秒 (SC-002), 1000 会話の受信箱読み込み ≤ 3 秒 (SC-004), 単一会話で 10k 件まで順序破綻なし (SC-005)
**Constraints**: Constitution **NON-NEGOTIABLE**: Network Anonymity (mainnet で Tor 強制), Minimal Key Exposure (アプリ層に生鍵を持たせない — セッションメモリのみ + 暗号化バックアップ経由の移送), Client-Side Completion (暗号化・パディング・署名・断片化はすべてクライアント側)。秘密鍵はバックアップファイル (AES-256-GCM + PBKDF2 100k) を経由してのみ端末間を移動し、ネットワークには出さない。
**Scale/Scope**: コンテンツパイプラインは既存仕様を継承 (1 フラグメント 256KB 上限, >1MB はチャンク分割)。ユーザーあたりインボックス最大 1,000 会話 × 会話あたり最大 10,000 メッセージを SC が保証するライン。mainnet 全体での DM TPS は別途チューニング。

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| 原則 | 該当要件 | 準拠の根拠 | 結果 |
|------|---------|----------|------|
| I. Network Anonymity (NON-NEGOTIABLE) | FR-003, 送受信双方のステルス化 | libp2p 層 Tor は既存継承。DM 固有のネットワーク追加処理なし。送信者もステルス経由のため主アカウントは on-chain に現れない。 | ✅ |
| II. Minimal Key Exposure (NON-NEGOTIABLE) | FR-014, FR-022 | DM 受信秘密鍵はセッションメモリのみ、端末間移送は暗号化バックアップ経由。既存 stealth 用 keyManager / backup と同じパスを再利用。アプリコードに生鍵/シードは保持させない。**例外**: 送信者ステルス鍵 (Sr25519 fresh seed) は tx2 署名のため WASM→JS 境界を越える。Complexity Tracking に記録済み。 | ⚠ 条件付 |
| III. Client-Side Completion (NON-NEGOTIABLE) | FR-002, FR-014, FR-020, FR-026 | 暗号化 (AES-256-GCM) / 署名 (Sr25519) / パディング / 断片化 (SSS+Merkle) はすべて wasm-engine 内でクライアント完結。サーバ (blockchain node / storage-node) は ciphertext のみを受け取る。 | ✅ |
| IV. Zero-Trust Hydra | FR-004 (inner signature), FR-012 (key binding verification) | 悪意フロントエンドでも、受信側は復号後に内部署名を検証できるため偽装不可能。フロントが差し替えた受信鍵はキーバインディング検証で弾く。 | ✅ |
| V. Economic Autonomy | FR-005, pallet-post 準拠の fee flow | 投稿と同等の base + byte-cost モデル。80% storage reward pool / 10% reaction reward pool / 10% burn の既存フローを DM にも適用。経済的忘却は Phase 3.4 で一貫適用。 | ✅ |
| VI. Test-First Development | 仕様書の全 FR にテスタブルな受け入れ基準あり | pallet 単体 → wasm-engine 単体 → フロントエンド Jest → 2 ノード統合 の 4 層でテスト先行。 | ✅ |

**Gate 結果**: 違反なし。Complexity Tracking は空のまま。

## Project Structure

### Documentation (this feature)

```text
specs/019-direct-messages/
├── spec.md              # Feature specification (完了済)
├── plan.md              # 本ファイル (/speckit-plan の出力)
├── research.md          # Phase 0 出力
├── data-model.md        # Phase 1 出力
├── quickstart.md        # Phase 1 出力
├── contracts/           # Phase 1 出力
│   ├── pallet-messaging-extrinsics.md
│   ├── wasm-engine-dm-api.md
│   └── runtime-api.md
├── checklists/
│   └── requirements.md
└── tasks.md             # Phase 2 (/speckit-tasks で生成 — 本コマンドでは作らない)
```

### Source Code (repository root)

```text
apps/blockchain/
├── pallets/
│   └── messaging/                    # 新規パレット
│       ├── Cargo.toml
│       ├── src/
│       │   ├── lib.rs                # Config, extrinsics (send_dm, publish_dm_key, revoke_dm_key), storage, events, errors, runtime API decl
│       │   ├── types.rs              # DmContentRef, DmMetaAddress, MessageRecord
│       │   ├── weights.rs
│       │   ├── mock.rs               # mock runtime (pallet-balances, pallet-stealth, pallet-storage の最小構成)
│       │   └── tests.rs              # 受け入れシナリオ → ユニットテスト (TDD)
├── runtime/
│   └── src/lib.rs                    # construct_runtime! に pallet-messaging を追加 + Config impl

packages/wasm-engine/
├── src/
│   ├── dm/                           # 新規モジュール (stealth, kzg, merkle の 4 番目)
│   │   ├── mod.rs                    # pub re-export
│   │   ├── envelope.rs               # inner envelope (sender pubkey + signature + ts + body) の encode/decode
│   │   ├── encrypt.rs                # dm_encrypt() — 固定サイズ padding → AES-256-GCM
│   │   ├── decrypt.rs                # dm_decrypt() — scan 済み ephemeral を受け取って復号 + 署名検証
│   │   ├── padding.rs                # canonical size buckets (FR-026)
│   │   ├── types.rs
│   │   └── tests.rs
│   └── lib.rs                        # `pub mod dm;` を追記

apps/frontend/
├── src/
│   ├── lib/
│   │   └── dm/                       # 新規
│   │       ├── index.ts
│   │       ├── api.ts                # PAPI 経由の pallet-messaging 呼び出し
│   │       ├── sender.ts             # 送信オーケストレーション (pre-fund → fragment+upload → send_dm)
│   │       ├── scanner.ts            # 受信側スキャナー (既存 lib/stealth/scanner.ts を継承した DM 用スキャン)
│   │       ├── store.ts              # 会話状態のストア (Zustand、既存パターン踏襲)
│   │       ├── keyManager.ts         # DM 受信鍵の publish / revoke / backup (再エクスポート主体)
│   │       ├── worker.ts             # バックグラウンドスキャン (Web Worker)
│   │       └── types.ts
│   ├── components/
│   │   └── dm/                       # 新規
│   │       ├── index.ts
│   │       ├── ConversationList.tsx
│   │       ├── ConversationView.tsx
│   │       ├── MessageComposer.tsx
│   │       ├── DmKeyManager.tsx      # 受信鍵の発行・取り消しUI
│   │       ├── BlockListManager.tsx
│   │       └── *.module.css
│   └── app/
│       └── dm/                       # ルート (/dm, /dm/[conversationId]) — Next.js App Router

apps/blockchain/tests/integration/
└── dm/                               # 新規シナリオ (pnpm test:integration に組込)
    ├── dm-send-receive.sh
    ├── dm-stealth-linkage.sh         # 送受信が主アカウントと紐付かないことを確認
    └── dm-multi-device.sh            # バックアップ経由の多端末再現
```

**Structure Decision**:
- 既存モノレポ構造を踏襲し、新規ディレクトリは `apps/blockchain/pallets/messaging`, `packages/wasm-engine/src/dm`, `apps/frontend/src/lib/dm`, `apps/frontend/src/components/dm`, `apps/frontend/src/app/dm`, `apps/blockchain/tests/integration/dm` の 6 箇所。
- `pallet-messaging` と `pallet-stealth` は**分離**を維持（ステルス送金 ≠ DM、鍵用途と pallet 責務の分離）。DM は pallet-stealth を依存として利用し、独自の DM 受信メタアドレスと独自の ephemeral key 記録を持つ。
- wasm-engine の DM モジュールは `stealth::keys` / `stealth::hash` / `stealth::address` / `stealth::backup` / `merkle` を呼び出すのが基本方針で、**新しい暗号プリミティブは導入しない** (Constitution V / Technology Stack の整合性のため)。ただし既存 API の形状が DM の用途と完全一致しないため、以下の**薄いラッパ関数のみ新設**する。どれも既存プリミティブの組み換えにとどまり、新しい曲線・新しい AEAD・新しい KDF を導入しない:
  - `dm_encrypt_and_pad` / `dm_decrypt_scan` — envelope + ISO 7816-4 padding + AES-256-GCM の組み合わせ層 (contracts/wasm-engine-dm-api.md W1/W2)
  - `dm_generate_sender_stealth` — `OsRng` → Sr25519 新鮮 keypair の 1 行ラッパ (W3)
  - `dm_fragment_ciphertext` — 既存 `merkle::split` の型シグネチャ合わせ (W4)
  - `dm_derive_recipient_stealth` — 既存 `stealth::address` の導出計算を「`eph_priv` を外部から与える」形に開いた薄いラッパ (W5) — **N3 対応**: 既存 `derive_stealth_address(&str)` は `eph_priv` を内部生成するため、同一の `eph_priv` を使って `shared_secret` も得たい DM の用途には直接使えない。
  - `dm_compute_inner_signed_hash` — FR-004 の署名対象ハッシュを決定論的に計算する純関数 (W6)。実装は `blake2b_256` の組合せのみ。
- フロントエンドは既存の `lib/stealth/scanner.ts` と `components/stealth/BackupImportDialog.tsx` をライブラリとして再利用する (鍵管理 UX の一貫性)。

## Complexity Tracking

### CT-1. Sender stealth Sr25519 seed の WASM→JS 境界越え (Constitution II 条件付例外)

**該当箇所**: `packages/wasm-engine/src/dm/` の `dm_generate_sender_stealth` (W3) が `secret_seed: [u8; 32]` を JS へ返す。Constitution II *"No raw private keys for users"* の文面と構造的に緊張する。

**なぜ受け入れるか**:

| 観点 | 内容 |
|------|------|
| 鍵の寿命 | tx2 (`send_dm`) 1 回の署名のみ。数秒で JS 側が `Uint8Array.fill(0)` で破棄。永続化・ディスク出力なし。 |
| 保護対象 | sender_stealth 鍵が漏洩しても、攻撃者は「既に送信済みの sender_stealth アカウント上の残高」しか触れない。送信者メインアカウント (Sr25519) の残高・識別子へは到達しない。 |
| 代替の不存在 | 既存 polkadot-api signer API はウォレット所有の main Sr25519 鍵のみを前提にしており、「fresh keypair を WASM 側で保持したまま外部 signer に挿入する」ルートが MVP 時点で存在しない。 |
| 影響範囲 | 例外は DM 送信経路の 1 関数 (W3) に局所化。他機能 (ステルス送金、投稿、リアクション) の鍵取扱いは既存原則のまま。 |

**将来の解消パス**:
- **Option A** (優先): `SubstrateSignerWasm` トレイトを `packages/wasm-engine` 内に実装し、seed を WASM スコープから出さずに `sign_extrinsic_payload(bytes) -> signature` だけを JS に露出。Phase 3 で polkadot-api 統合方式が固まった時点で再評価。
- **Option B**: `pallet-messaging::send_dm` の origin を「送信者メインアカウントが delegate 委任」で受ける拡張 (`pallet-proxy` / `pallet-multisig` 類似) を runtime に追加し、tx2 をメインアカウント署名で発行できるようにする。ただしオンチェーン観測者に「main→DM」の結合が見える可能性があり、FR-024 と慎重に照合が必要。

**記録の所在**:
- `contracts/wasm-engine-dm-api.md` W3 "Constitution II (Minimal Key Exposure) との整合" 節に技術的詳細。
- Constitution Check 表の II. 欄に ⚠ 条件付 マークを付与済み。
- tasks.md 生成時に「CT-1 解消検討」タスクを Phase 3 マイルストンに予約する想定。
