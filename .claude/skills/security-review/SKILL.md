---
name: security-review
description: Anarchy の非妥協セキュリティ原則 (Tor/I2P 強制、client-side のみ暗号、秘密鍵 session-only、foreground PoW、X-Chain-Auth) のチェックリスト。認証・秘密鍵処理・新規 RPC/extrinsic 追加・Storage node 通信・ユーザー入力処理時、PR レビュー時に使用する。プロジェクト固有ルールをカバーし、一般的な OWASP チェックも補完的に含む。
---

# Security Review — Anarchy

Anarchy は **L1 blockchain + 分散ストレージで構築された匿名 SNS**。一般的な Web セキュリティよりも厳しい非妥協ルールを `CLAUDE.md` が定めている。このスキルはそれらを実装・レビューで verify するためのチェックリスト。

---

## 🔴 非妥協原則 (Non-Negotiable Principles)

1. **Network anonymity** — libp2p transport 層で Tor/I2P を enforce、IP メタデータ漏洩ゼロ
2. **Client-side key management** — 秘密鍵は session memory のみ。永続化しない。エクスポートはパスワード暗号化バックアップのみ
3. **Client-side only crypto** — 暗号化 / SSS 断片化 / メタデータ除去は送信前にクライアント側で完了
4. **Foreground PoW** — reaction mining はタブが foreground のときのみ (Page Visibility API)

これらに違反するコードは **merge してはならない**。仕様上どうしても必要なら spec/constitution を先に更新。

---

## 1. Tor / I2P Transport

### 実装箇所
- `apps/blockchain/node/src/cli.rs` — `--tor-mode {off|outbound-only|forced}` フラグ
- `apps/blockchain/node/src/command.rs` — `apply_tor_mode()` で torsocks 検証

### チェック項目
- [ ] mainnet chain spec (`apps/blockchain/node/src/chain_spec.rs`) で `TorMode::Forced` が強制される
- [ ] `TorMode::Off` は dev build でのみ許容、release プロファイルで warning
- [ ] `outbound-only` モードが新規インバウンドリスナを無効化していることを確認
- [ ] libp2p 層に直接 TCP listener を **追加していない** (`/ip4/.../tcp` アドレスを hard-code しない)
- [ ] Storage node 側 (`apps/storage-node/`) も同じ transport 方針 (onion / i2p) を適用

### 禁止パターン
```rust
// ❌ Tor をバイパスする TCP 直結
let config = sc_network::config::NetworkConfiguration {
    listen_addresses: vec!["/ip4/0.0.0.0/tcp/30333".parse()?],
    ..
};
```
必ず `apply_tor_mode()` 経由で `/onion3/...` / i2p multiaddr に変換する。

---

## 2. 秘密鍵 Session-Only Management

### 実装箇所
- `apps/frontend/src/lib/stealth/keyManager.ts` — StealthKeyManager
- `apps/frontend/src/lib/dm/keyManager.ts` — DM KeyManager
- `packages/wasm-engine/src/stealth/backup.rs` — 暗号化バックアップ生成

### チェック項目
- [ ] 秘密鍵 (seed phrase, spend_priv, scan_priv, signing key) を `localStorage` / `sessionStorage` / `IndexedDB` に **plain で書き込んでいない**
- [ ] `beforeunload` イベントで `secureWipe()` / `zeroize()` 呼び出しあり
- [ ] React state / zustand の persist middleware に秘密情報が乗っていない (`partialize` で除外)
- [ ] console.log / logger に鍵内容を出していない (`JSON.stringify(keyManager)` も危険)
- [ ] エクスポートは AES-256-GCM + PBKDF2 (高反復) で暗号化された json のみ
- [ ] インポート時にパスワード強度チェック + 失敗時に鍵残骸をメモリから消去

### コードレベル検査の grep
```bash
# 疑わしいパターンを探す
grep -rn "localStorage.setItem.*[Kk]ey" apps/frontend/src/
grep -rn "JSON.stringify.*[Pp]riv" apps/frontend/src/
grep -rn "console.log.*[Ss]eed" apps/frontend/src/
```

---

## 3. Client-Side Only Crypto

### 原則
- ciphertext / stealth address / Merkle fragment は **送信前に完成** している
- サーバ (blockchain node / storage node) は ciphertext を復号できてはならない (E2E)
- メタデータ (EXIF, 送信者 IP, Accept-Language) は送信前に除去

### チェック項目
- [ ] 画像アップロードは `lib/mediaProcessor.ts` で EXIF/GPS を除去してから wasm-engine に渡す
- [ ] DM / Post の暗号化は `workers/crypto.ts` で完了してから PAPI 送信
- [ ] Storage node API は ciphertext と MerkleRoot のみ受け取り、**平文を要求するエンドポイントを作らない**
- [ ] blockchain node の RPC は暗号化前データを受け付けない (pallet が ciphertext_len / merkle_root だけを格納)
- [ ] 新規 extrinsic に plaintext 引数 (例: `text: Vec<u8>`) を追加していない

### 違反サイン
```rust
// ❌ 平文を runtime に渡している
pub fn post_plaintext(origin: OriginFor<T>, content: Vec<u8>) -> DispatchResult { ... }

// ✅ 正しい: ciphertext 参照のみ
pub fn create_post_v2(
    origin: OriginFor<T>,
    merkle_root: [u8; 32],
    ciphertext_len: u64,
    k: u32, n: u32,
) -> DispatchResult { ... }
```

---

## 4. Foreground PoW Enforcement

### 実装箇所
- `apps/frontend/src/hooks/useReactionMining.ts`
- `apps/frontend/src/workers/` の mining worker

### チェック項目
- [ ] PoW ワーカー開始時に `document.hidden` を確認し、hidden なら未起動
- [ ] `visibilitychange` で hide → pause、 show → resume
- [ ] requestIdleCallback / setTimeout 等で hidden 状態でも 1 サイクル通す実装がない
- [ ] Service Worker / background fetch に PoW ロジックを置いていない

---

## 5. X-Chain-Auth (blockchain node ↔ storage node)

### 実装箇所
- `apps/blockchain/node/src/rpc/storage.rs` — sr25519 署名生成 / 検証
- `apps/storage-node/src/auth/` — 受信側検証

### 原則 (最近 commit a6fbc1d で全 request 必須化)
署名対象文字列: `"chain-auth:{unix_timestamp}:{method}"`

### チェック項目
- [ ] 全 storage-node エンドポイントに `X-Chain-Auth` middleware が適用されている
- [ ] timestamp の許容ウィンドウは ±60 秒以内 (replay 防止)
- [ ] ノンス / method 名を署名対象に含め、method 横断リプレイを防ぐ
- [ ] 公開鍵リストは chain state から取得 (hard-code しない)
- [ ] 失敗時は 401 を返し、詳細エラー内容を body に出さない (情報漏洩防止)

### 注意
X-Chain-Auth は **なりすまし耐性ではなく軽量認証** (chain から既知のノードであることのみ証明)。ユーザー秘密鍵保護には関与しない。

---

## 6. Extrinsic / RPC レビュー時チェック

新 extrinsic・runtime API を追加する場合:

- [ ] 入力サイズに `BoundedVec<_, MaxN>` + Config const の上限
- [ ] 重複送信検知 (merkle_root / nonce を storage key に)
- [ ] rate limit (MaxDispatchesPerBlock 等で DoS 耐性)
- [ ] 範囲クエリ runtime API は上限 (例: `to_block - from_block > 1024` で empty 返却)
- [ ] origin チェック: `ensure_signed`, `ensure_root`, 必要に応じて `ensure_signed_or_root`
- [ ] 料金徴収と storage 変更の順序 (burn → mutate → emit event)
- [ ] overflow 算術は全て `checked_*` + `Error::Overflow`
- [ ] Event / Error 名が spec の contracts/ ファイルと一致

---

## 7. ユーザー入力バリデーション (Frontend)

- [ ] SS58 アドレスは `lib/addressValidation.ts` の `validateSS58Address` を必ず通す
- [ ] ニックネーム / post 本文は最大長チェック + 制御文字 / zero-width chars 除去
- [ ] media upload の MIME / magic byte 検査 (`lib/mediaValidator.ts`)
- [ ] URL 表示時は `target="_blank" rel="noopener noreferrer"` 強制
- [ ] React の `dangerouslySetInnerHTML` は使わない。どうしても必要なら DOMPurify 経由

---

## 8. 一般的な OWASP 関連 (補完)

Anarchy は traditional Web app ではないが、dashboard UI 系で:

- [ ] CSP ヘッダ (next.config で strict) — script-src は self のみ、wasm-unsafe-eval は WASM 初期化のみ
- [ ] XSS: Template に user-supplied string を textContent で入れる (innerHTML 禁止)
- [ ] CSRF: extrinsic は署名が必要なので CSRF は構造的に不要、ただしローカル RPC プロキシ (dev) はトークン確認
- [ ] Dependency: `pnpm audit` を定期実行、`cargo audit` も。blast radius が大きいのは wasm-engine 依存

---

## 9. PR レビュー時クイックチェック (Top-10)

新規 PR を見るとき、この順で grep / 目視:

1. `localStorage.setItem` で鍵情報を書いていないか
2. `console.log` / `logger.info` に private 情報が載っていないか
3. 新 extrinsic が ciphertext 以外を格納していないか
4. runtime API に範囲クエリガード ≤1024 block 等の上限があるか
5. Tor モード設定をバイパスする hard-code アドレス / TCP listener がないか
6. `BoundedVec` 無しで `Vec<_>` を Storage に入れていないか
7. `checked_*` 使わず素の `+` / `*` を runtime コードで使っていないか
8. PoW / mining worker が Page Visibility を見ているか
9. X-Chain-Auth middleware がバイパスされていないか
10. テストが mock のみで実装コードを検証していない状況になっていないか (CLAUDE.md #6)

---

## 10. インシデント時の対応

鍵リーク / プロトコル欠陥を見つけた場合:
- 該当コミット ID と影響範囲を特定し、private channel で報告 (public issue にしない)
- rotate: chain-auth 公開鍵リストを governance で更新
- 必要なら runtime upgrade で該当 extrinsic を temporary disable + `spec_version` bump
- user-facing 告知は暗号学的影響を明記 (「これ以降送信された DM は安全」等)

## 参考
- CLAUDE.md の "Security Principles (non-negotiable)" セクション
- `specs/019-direct-messages/spec.md` FR-014 (session memory + encrypted backup)
- 既存スキル: `wasm-engine` (暗号プリミティブ選択), `backend-patterns` (pallet extrinsic validation 順序)
