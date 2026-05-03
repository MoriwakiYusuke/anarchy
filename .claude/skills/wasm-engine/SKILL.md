---
name: wasm-engine
description: anarchy-wasm-engine (packages/wasm-engine/) の開発パターン。KZG-VSS hybrid scheme (ark-bls12-381), Merkle tree (rs_merkle), ステルスアドレス (EIP-5564 互換), DM 暗号化 (encrypt/decrypt/padding/envelope) の実装、wasm-bindgen 境界設計、wasm-pack ビルド手順、frontend/Worker からの consume、テスト方針を含む。暗号プリミティブを追加・修正する際、wasm-bindgen 型シグネチャを書く際、新しい DM/stealth/storage-proof 機能を実装する際に使用する。
---

# Wasm Engine — Anarchy Crypto Engine

`packages/wasm-engine/` は **ブラウザで動く Rust 暗号エンジン**。ark-bls12-381 の KZG-VSS、AES-256-GCM + SSS のハイブリッド方式、EIP-5564 風ステルスアドレス、DM E2E 暗号化を提供する。フロントエンドは `"anarchy-wasm-engine": "file:../../packages/wasm-engine/pkg"` として消費する。

## モジュール構成

```
packages/wasm-engine/src/
├── lib.rs          # pub mod + re-export (API の公式 surface)
├── merkle.rs       # legacy: rs_merkle ラッパ (SSS v1)
├── kzg/            # KZG-VSS hybrid (v2)
│   ├── mod.rs
│   ├── commit.rs, vss.rs, proof.rs, srs.rs, compress.rs
│   └── wasm.rs     # wasm_bindgen 境界 (hybrid_split / kzg_vss_* / kzg_init_srs)
├── stealth/        # EIP-5564 互換
│   ├── keys.rs, address.rs, scan.rs, hash.rs, backup.rs, types.rs
│   └── mod.rs
└── dm/             # 019-direct-messages
    ├── encrypt.rs  # dm_encrypt_and_pad, dm_generate_sender_stealth
    ├── decrypt.rs  # dm_decrypt_scan
    ├── envelope.rs # dm_compute_inner_signed_hash
    ├── padding.rs  # 固定バケット 1K/4K/16K/64K/256K
    └── types.rs
```

`lib.rs` で `pub use` して JS 側にフラットなシンボルを露出する — **新 API を追加したら lib.rs の re-export を忘れない**。

## wasm-bindgen 境界ルール

### 入力
| Rust 型 | JS 型 | 注意 |
|---|---|---|
| `&[u8]` | `Uint8Array` | 最適 |
| `Vec<u8>` | `Uint8Array` | copy コスト |
| `String` / `&str` | `string` | UTF-8 強制 |
| `u32` / `u64` | `number` | **u64 は 2^53 超えると精度落ちる — 大きな値は `BigInt64Array` / `Uint8Array` 経由** |
| 構造体 | `#[wasm_bindgen] struct` | getter メソッド経由で読み出し |
| `Option<T>` | `T \| undefined` | JS 側は nullable チェック必須 |

### 出力
- 単純値 → そのまま return
- 複合値 → `#[wasm_bindgen] pub struct` に getter 関数。derive Serialize + `serde-wasm-bindgen::to_value` でも可だが、大きな構造は struct + getter の方がパフォーマンス良い

### エラー
```rust
#[wasm_bindgen]
pub fn vss_split(data: &[u8], threshold: u32, total: u32) -> Result<WasmVssSplitResult, JsError> {
    do_vss_split(data, threshold, total)
        .map_err(|e| JsError::new(&format!("vss_split: {}", e)))
}
```
- `Result<T, JsError>` を return すると JS 側で `throw` になる
- panic させない: `unwrap()` は non-test パスに絶対置かない (wasm panic はトラップ、状態破損)

### 命名規則
- JS 側は camelCase が慣用だが、Anarchy では **snake_case を維持** (Rust と揃える)
- `dm_encrypt_and_pad`, `kzg_vss_split`, `derive_stealth_address` 等

## KZG-VSS Hybrid Scheme

### 概念
1. **AES-256-GCM** で plaintext を暗号化 (fast, memory-efficient)
2. AES 鍵を **BLS12-381 scalar** として扱い、**VSS で n 個のシェアに分割** (threshold k)
3. 各シェアに **KZG commitment + proof** を付与 (ストレージノードが保持していることを後で証明可能)
4. ciphertext は Reed-Solomon 風に断片化 (shard_size 統一)

### 主要 API
| 関数 | 役割 |
|---|---|
| `kzg_init_srs(bytes)` | SRS (ceremony 結果) をロード |
| `hybrid_split(plaintext, k, n)` → `WasmHybridSplitResult` | 分割 (encrypt + VSS + KZG proof) |
| `hybrid_recover(shards, k, n)` | k 個以上のシェアから復元 |
| `kzg_verify_proof(commitment, proof, index, value)` | 個別シェアの所持証明 |
| `kzg_vss_split` / `kzg_vss_recover` | 鍵のみ VSS (ciphertext 分離したい場合) |
| `regenerate_share(existing_shares, target_index, k, n)` | 生存シェアから失われたシェアを再生 |

### SRS (Structured Reference String)
- bundle 時に固定バイナリとして同梱。ceremony をやり直すと互換性破壊
- テスト用 `init_test_srs()` は `test-utils` feature 有効時のみ露出 (本番バイナリに混入しない)

### 実装時の落とし穴
- `ark-bls12-381` は `default-features = false` 必須 (std 無効化、wasm size 削減)
- serialize は `ark-serialize` の compressed form を使う (非 compressed は 2 倍サイズ)
- Scalar の rejection sampling は constant-time でないので key derivation では使わない (blake2b → 直接 bytes)

## Stealth (EIP-5564)

- `generate_stealth_keys()` → `(spend_priv, scan_priv, meta_address)`
- 送信者側: `derive_stealth_address(meta_address)` で ephemeral keypair + stealth address 算出
- 受信者側: `scan_transaction(ephemeral_pub, scan_priv)` で自分宛か判定 — 高速パス (ECDH のみ)
- 鍵導出一致を保証するため、**scalar reduction / hash-to-scalar の仕様を変えない**。`hash.rs` を修正する際は必ず known-answer test 追加

### バックアップ
- `encrypt_backup(keys, password)` → AES-256-GCM + PBKDF2-SHA256 (高反復)
- `decrypt_backup(ciphertext, password)` は失敗時に **partial state を残さない** (`Result::Err` で丸ごと drop)

## DM (019-direct-messages)

### フロー
1. 送信者: `dm_generate_sender_stealth()` で自分側 ephemeral + view ハンドル
2. `dm_derive_recipient_stealth(recipient_meta_address, ephemeral_priv)` で stealth アドレス
3. `dm_encrypt_and_pad(plaintext, shared_secret)` で AES-256-GCM + **固定バケット padding** (1K/4K/16K/64K/256K)
4. `dm_compute_inner_signed_hash(envelope)` で inner signature 生成 (偽装防止)
5. `dm_fragment_ciphertext(ciphertext, k, n)` で storage 層向け分割
6. Frontend が runtime extrinsic `Messaging::send_dm` に merkle_root / ephemeral_pubkey / ciphertext_len を送信
7. 受信者: `dm_decrypt_scan(dispatches_at_block, scan_priv)` で自分宛復号試行

### Padding バケット
chain 側 `pallet-messaging` の `DM_PADDING_BUCKETS` と**必ず一致**: `[1_024, 4_096, 16_384, 65_536, 262_144]`。変更する場合は pallet 側と同時に bump。

### Inner signature
ciphertext 偽装攻撃 (R5) 対策で ciphertext 内に署名を内包する。署名対象は envelope hash — `envelope.rs` の仕様を変える際は spec contract と整合性確認必須。

## ビルド

```bash
cd packages/wasm-engine
cargo install wasm-pack                            # 初回のみ
wasm-pack build --target web --out-dir pkg        # 必須モード
```

- `--target web` (ES modules): frontend consume 用
- `--target nodejs`: サーバ側テストしたい場合のみ
- `pkg/` ディレクトリが生成され、frontend の file dep が解決できるようになる
- `pnpm install` **の前に** 実行しないと frontend の postinstall (copy-wasm.sh) が失敗

### テスト
```bash
cd packages/wasm-engine
cargo test                                         # Rust unit (arkworks 全機能使える)
cargo test --features test-utils                   # init_test_srs を使うテスト
# wasm-bindgen-test を書く場合:
# wasm-pack test --headless --chrome
```

- 各モジュールの `tests/` もしくは `#[cfg(test)] mod tests` に known-answer test を入れる
- **KZG / stealth / DM は 1 ビット改変で壊れる** — fixed test vector 必須
- Fuzz が欲しい場合は `cargo-fuzz` だが、wasm 境界を経由した fuzz は jsdom 上では難しい

## Frontend からの利用

### 直接 import (メインスレッド許容ロジック)
```typescript
import init, { dm_encrypt_and_pad } from 'anarchy-wasm-engine'
await init()                         // 一度だけ
const out = dm_encrypt_and_pad(plaintext, secret, 4096)
```

### Worker 経由 (重い処理は必須)
KZG proof 生成、VSS split、DM scan (複数ブロック) は必ず `workers/crypto.ts` → `WorkerPool` 経由で呼ぶ。main thread でやると UI がフリーズする。

### 初期化の特殊ケース
Next.js だと `import.meta.url` が bundle 時に壊れるので、`keyManager.ts` は `public/wasm/anarchy_wasm_engine_bg.wasm` を明示 fetch → `initSync({ module })` で初期化する。新規コード追加時も同じパターンを踏襲。

## 型同期 (Rust ↔ TS) チェックリスト

新しい wasm-bindgen API を追加したら:

- [ ] `lib.rs` の `pub use` に追加
- [ ] `wasm-pack build` で `pkg/anarchy_wasm_engine.d.ts` を再生成
- [ ] TS 側で `anarchy-wasm-engine` を import しているファイルに型補完が効くことを確認
- [ ] u64/u128/bigint 周りでの精度落ちが発生しないか確認
- [ ] 失敗系 (JsError) が TS 側で `try/catch` できるか確認
- [ ] bundle size が急増していないか (`pkg/anarchy_wasm_engine_bg.wasm` のサイズ前後比較)

## よくある失敗

| 症状 | 原因 |
|---|---|
| `TypeError: wasm.memory is undefined` | `init()` / `initSync()` 呼び忘れ |
| KZG proof verification が失敗 | SRS mismatch (test-utils 版のまま production init) |
| DM decrypt が全ブロック NG | padding bucket 不一致 / ephemeral_pub の endian 誤り |
| 鍵導出が再現しない | hash-to-scalar 実装差 (ark 版バージョン up 時) |
| bundle size 激増 | `ark-*` 依存の `default-features` を true のままにしている |
| `panic: attempt to subtract with overflow` (wasm trap) | Scalar 減算 negative — `checked_sub` + Error に |

## 参考実装
- `packages/wasm-engine/src/kzg/wasm.rs` — wasm-bindgen 境界の典型例
- `packages/wasm-engine/src/dm/encrypt.rs` — padding + AES-GCM + ephemeral 生成
- `packages/wasm-engine/src/stealth/backup.rs` — AES-256-GCM + PBKDF2 バックアップ
- `apps/frontend/src/lib/stealth/keyManager.ts` — initSync による明示ロードのパターン
