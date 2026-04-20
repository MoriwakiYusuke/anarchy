---
name: playwright-e2e
description: Anarchy フロントエンド (Next.js 14 + PAPI/smoldot + anarchy-wasm-engine) で新機能の E2E テストを書くためのスキル。Playwright と Playwright MCP は導入済み。新機能 (新ページ / 新 extrinsic / 新 wasm 暗号 / 新スキャナ / 新 UI フロー) を追加した際に「ブラウザ上で本当に動くか」を検証するためのテスト観点・配置・パターンを提供する。「E2E テスト追加して」「機能が動くか確認して」「Playwright で検証して」依頼に使用。
---

# Playwright E2E — Anarchy Frontend

Playwright は `apps/frontend/` に devDependency として導入済み。実行は **Playwright MCP** を介して行うため本 skill には CLI 手順を載せない。本 skill の責務は **「新機能が end-to-end で本当に動くか」を漏れなく確認するための観点とパターンを定義すること**。

## このスキルが扱うレイヤ

| 層 | テストツール | 責務 |
|---|---|---|
| Pallet 単体 | `cargo test -p pallet-xxx` | extrinsic ロジック、Storage/Event/Error |
| Wasm 単体 | `cargo test --lib` / `wasm-pack test` | 暗号プリミティブ、SCALE encode |
| Frontend 単体 | Jest + Testing Library | hooks / store / コンポーネント (jsdom) |
| Integration shell | `pnpm test:dm` 等 | 多ノード testnet 上のチェーン整合 |
| **E2E (本 skill)** | **Playwright** | **実ブラウザ上で smoldot + Wasm + Worker + UI を一気通貫で検証** |

E2E が無いと、Jest で worker 層までは通るのに **本物のブラウザに載せた瞬間 Wasm init が壊れる / smoldot が hang する / SSR ガード忘れで `window is not defined` が出る** といった統合バグを catch できない。

## 配置と命名

```
apps/frontend/
├── e2e/
│   ├── fixtures/          # 共有 fixture (chain client / signer / wasm init / dev-only test hook)
│   ├── helpers/           # ページオブジェクト風ヘルパ (login, send, scan...)
│   └── <feature>.spec.ts  # 機能ごと 1 ファイル
├── playwright.config.ts   # 設定済み (workers=1, timeout 120s, webServer=pnpm dev)
```

- Jest は `tests/` と `__tests__/` を拾い、`e2e/` は除外設定済み (`jest.config.js`)。
- spec ファイル名は **機能名で 1 ファイル**: `transfer.spec.ts`, `dm-send.spec.ts`, `stealth-scan.spec.ts`, `nickname.spec.ts`...
- helpers/fixtures は **そのまま再利用** できるよう機能横断で書く (チェーン接続待ち、key inject、scan 完了待ち等)。

## 「新機能を追加したとき」に書くべき E2E チェックリスト

新機能タイプ別。最低でもこの観点を網羅する spec を 1 本書く。

### A. 新ページ / 新ルートを追加した

- [ ] ルート (`/foo`) に直接遷移して 200 + `<h1>` 等の要識別要素が描画される
- [ ] SSR と CSR の両方で壊れない (`page.goto` 直後 + `reload()` 後 両方確認)
- [ ] チェーン接続前 (skeleton) → 接続後 (本データ) の状態遷移が UI 上で見える
- [ ] i18n: `LanguageSwitcher` で言語切替 → 主要文字列が切り替わる
- [ ] エラーステート (RPC timeout / wasm init 失敗) で空白画面ではなく明示的メッセージが出る

### B. 新 extrinsic を追加した (pallet / runtime / 送信フォーム)

- [ ] フォームから extrinsic を実際に送信し、`signAndSubmit` が成功する (txHash が返る)
- [ ] `BigInt` 渡し漏れ (`u64`/`u128`) で decode error にならない
- [ ] 入力 validation のフィードバックがリアルタイムで出る (送信前に弾ける)
- [ ] **失敗ケース**: 残高不足 / 重複送信 / 不正入力 で extrinsic が `Error::*` を返したとき、UI に i18n 化されたエラーが出る (raw RPC message が露出していない)
- [ ] **finalize 待ち**: pending → in-block → finalized の状態遷移が表示され、ハングしない (30s タイムアウト)
- [ ] 送信後に関連 storage/event の値が UI に反映される (例: 残高 / 投稿一覧 / nickname)

### C. 新 wasm 暗号 / 重い処理を追加した

- [ ] 実ブラウザで `init()` が成功し、Wasm 関数が呼べる (Worker 経由でも main thread 経由でも)
- [ ] 重い処理中も UI がフリーズしない (`expect(button).toBeEnabled()` を計算中に取れる)
- [ ] Worker pool 経由のとき、connector の cleanup が走り leak しない (test 終了後 `await context.close()` で warning が出ない)
- [ ] **秘密情報の DOM/storage 漏洩チェック**: `localStorage` `sessionStorage` `IndexedDB` `document.cookie` に生鍵 / seed / 平文ボディ が残っていない (security-review skill 参照)

### D. 新スキャナ (storage / chain / DM 等) を追加した

- [ ] スキャン開始 → progress 表示 → 完了 までの状態遷移が UI に出る
- [ ] スキャン結果 0 件・1 件・大量 (1000+) のいずれでも崩れない (仮想化されている場合は scroll で全件アクセスできる)
- [ ] **再開動作**: ページ reload 後、`last_scanned_block` から差分スキャンになる (全件再スキャンしない)
- [ ] **タブ非アクティブ動作**: Page Visibility API でフォアグラウンド時のみ動く処理 (PoW reaction 等) が hidden で停止する
- [ ] **マッチしないデータの混入**: 自分宛でない dispatch を流しても decrypt 失敗扱いで UI に出ない

### E. 鍵生成 / 鍵管理 UI を追加した

- [ ] 鍵生成 → ページ reload で **再ログインが必要** になる (session-only 原則)
- [ ] エクスポート → AES + パスフレーズ暗号化された JSON が download される (中身が平文 seed でない)
- [ ] インポート: 正パスフレーズで復元 / 誤パスフレーズで明確なエラー
- [ ] **localStorage / IndexedDB に生鍵が書き込まれていないこと** を `page.evaluate` で必ず assert する

### F. マルチデバイス / マルチ AccountId が絡む機能を追加した

- [ ] `context.newPage()` で 2 つの独立した擬似ユーザを起動し、片方のアクションが他方に正しく届く (DM 送受信、reaction 等)
- [ ] 同一 AccountId を 2 タブで開いてもチェーン側 nonce 競合で extrinsic が失敗しない (推奨は per-test に新規鍵)

### G. 共通 (どの新機能でも見る)

- [ ] **スクリーンショット regression が無い**: 隣接機能のレイアウトが崩れていない (主要ページの `await expect(page).toHaveScreenshot()` を 1 枚ずつ)
- [ ] **コンソールエラー 0 件**: `page.on('console', msg => msg.type() === 'error' && fail())` を fixture に仕込む
- [ ] **未捕捉 Promise rejection 0 件**: `page.on('pageerror', ...)`
- [ ] **Tor/I2P 等 transport 設定が外せていない**: 設定画面の該当トグルが default ON

## 共通 fixture が提供すべきもの

`e2e/fixtures/` に集約する。新機能ごとに重複実装しないこと。

- `chain` — `chain-status` data-state 属性 が `connected` になるまで待つ helper
- `signer` — per-test に sr25519 鍵を生成 + faucet/sudo-mint で残高付与 + テスト終了で session 破棄
- `dmKey` / `stealthKey` — 必要に応じて on-chain key publish も済ませた状態を提供
- `noConsoleErrors` — beforeEach で console / pageerror リスナを張り afterEach で assert
- `noKeyLeak` — afterEach で `localStorage` `sessionStorage` `indexedDB` を走査し、鍵らしき長さの hex/base64 が残っていないことを assert

## 待ち方の原則 (flaky 排除)

- **絶対に** `waitForTimeout(N)` でスリープしない。`expect.poll` か data-testid + role-based locator + `toBeVisible({ timeout })` を使う
- チェーン接続は `data-testid="chain-status"[data-state="connected"]` を待つ (画面側で属性を露出させる責務がある)
- extrinsic finalize は **画面の status 表示** を待つ (内部 promise を直接 await せず UI 経由で観測)
- Wasm 重処理 (KZG / SSS / decrypt scan) は `expect(...).toBeVisible({ timeout: 60_000 })` まで許容
- スキャン結果は **件数の安定** で判定 (`await expect(list.locator('[role=listitem]')).toHaveCount(3, { timeout: 60_000 })`)

## 鍵注入とテスト用フック

production build に絶対残さない dev-only window フック経由で seed を入れる:

```ts
// e2e 側
await page.evaluate((seed) => (window as any).__anarchyTest__?.injectSeed(seed), seed)
```

frontend 側は `process.env.NEXT_PUBLIC_E2E === 'true'` でガードして expose する (production bundle から dead-code elimination されることを必ず確認する)。**localStorage に書き込む実装に変えてはいけない** — session-only 原則 (CLAUDE.md §Security Principles 2) を E2E で破ってはならない。

## チェーン側の前提

E2E は `pnpm dev:node` (single dev) または `pnpm testnet:start` (3-node) が事前に起動している前提。Playwright config の `webServer` が立ち上げるのは **Next.js だけ**。チェーンを止めたまま E2E を走らせると `chain-status` が `connected` にならず全 fixture が timeout する。

storage-node が必要な機能 (post media, DM ciphertext fragments 等) は `pnpm storage:start` も事前に必要。

## Playwright MCP 経由で実行するときの注意

- spec 追加後 / fixture 変更後は **必ず一度 MCP に走らせて green を確認** してから「実装完了」と言う (CLAUDE.md AI Agent Rules 3, 4)
- MCP 経由でも `playwright.config.ts` の `workers: 1` 設定は尊重される。並列したくなっても **smoldot singleton 制約** を理由に維持
- 失敗時は MCP が返す trace / screenshot を起点に原因特定 (raw stdout だけ見て推測しない)

## 既存テスト戦略との棲み分け

| 検証したい性質 | 第一選択 |
|---|---|
| pallet ロジック単体 | Rust pallet test (`cargo test -p pallet-xxx`) |
| 多ノードでのチェーン整合・Storage P2P | shell integration (`apps/blockchain/tests/integration/`) |
| TypeScript pure logic / store / 純粋コンポーネント | Jest + Testing Library |
| **ブラウザ上の Wasm init / smoldot 接続 / Worker 連携 / UI フロー全体** | **Playwright (本 skill)** |

E2E に何でも詰めると遅くなる。**E2E でしか検出できないバグ** にフォーカスし、Jest / cargo test / shell integration で済むものはそちらに置く。

## 互換性方針との関係

CLAUDE.md §互換性方針 のとおり前後方互換は不要。**スナップショットや stored snapshot 系のテスト ([toMatchSnapshot] / `toHaveScreenshot`) は積極的に作り直してよい** — 古いスナップショットを残すために実装を歪めない。chainspec / IndexedDB スキーマが変わったら fixture と E2E 双方を作り直す。

## アンチパターン

- `page.waitForTimeout(5000)` を散りばめる → flaky の温床。data-testid + `expect.poll` で書く
- production build に test hook (`__anarchyTest__`) を expose したまま出荷する → セキュリティ事故
- 同一 AccountId を全テストで使い回す → nonce 競合。fixture で per-test 鍵生成
- Jest 用 `tests/` 配下に `*.spec.ts` を置く → testMatch が拾って混乱
- E2E で pallet エラー全パターンを網羅しようとする → cargo test の責務。E2E は **「画面に正しく出る」** だけ確認

## 関連 skill

- 鍵 / 通信 / 認証の不変条件チェック: [security-review](../security-review/SKILL.md)
- 画面側パターン (data-testid 設置場所、SSR ガード, Worker pool): [frontend-patterns](../frontend-patterns/SKILL.md)
- 重 wasm 暗号の挙動: [wasm-engine](../wasm-engine/SKILL.md)
- TDD 全体方針: [tdd-workflow](../tdd-workflow/SKILL.md)
- chain / frontend の起動コマンド: [dev-command](../dev-command/SKILL.md)
