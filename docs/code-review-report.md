# Anarchy プロジェクト 包括的コードレビューレポート

**レビュー日**: 2026-02-11
**対象ブランチ**: `009-post-storage-migration`
**レビュー範囲**: リポジトリ全体（Blockchain Pallets, Storage Node, Frontend, WASM Engine, Scripts, Runtime）

---

## サマリー

| 重要度 | 件数 | 内訳 |
|--------|------|------|
| CRITICAL | 16 | Pallets: 5, Storage Node: 5, Frontend: 2, WASM/Scripts/Runtime: 4 |
| HIGH | 22 | Pallets: 8, Storage Node: 7, Frontend: 7, WASM/Scripts/Runtime: 7 |
| MEDIUM | 27 | Pallets: 9, Storage Node: 8, Frontend: 10, WASM/Scripts/Runtime: 8 |
| LOW | 15 | Pallets: 4, Storage Node: 5, Frontend: 6, WASM/Scripts/Runtime: 4 |
| **合計** | **80** | |

---

## 最優先対応事項 (Top 10)

以下はプロジェクト全体で最も緊急に対応すべき項目:

| # | ID | 領域 | 問題 | リスク |
|---|-----|------|------|--------|
| 1 | RT-C03 | Runtime | トランザクション手数料が完全に0 | 全extrinsicに対するDoS攻撃が無コストで可能 |
| 2 | RT-C04 | Runtime | `pallet_sudo` が本番ランタイムに含まれている | 単一キーで全権限掌握可能 |
| 3 | RT-H05 | Runtime | `ExistentialDeposit = 1` (実質0) | 手数料0と合わせてダストアカウント攻撃がコストゼロ |
| 4 | SN-C01 | Storage Node | CORS全オリジン許可 + 認証なし | 外部から無認証でストレージ操作可能 |
| 5 | PS-C01 | pallet-storage | `register_fragment` にアクセス制御なし | コストなしで無制限にオンチェーンストレージ消費可能 |
| 6 | PP-C02 | pallet-post | MerkleRoot の重複チェックなし | データ整合性の破壊 |
| 7 | PP-C03 | pallet-post | コスト計算の型変換失敗時にコスト0になる | 無料投稿が可能になる |
| 8 | SN-C03 | Storage Node | RPCリクエストボディサイズ制限なし | 巨大リクエストによるOOM |
| 9 | SN-C04 | Storage Node | 秘密鍵ファイルのパーミッション未設定 | 他ユーザーから秘密鍵読み取り可能 |
| 10 | SC-C02 | Scripts | `sudo-mint.mjs` が本番環境で実行可能 | 任意のトークン発行 |

---

## 1. Blockchain Pallets

### 1.1 pallet-post (`apps/blockchain/pallets/post/src/lib.rs`)

#### CRITICAL

**PP-C01: NextPostId オーバーフローによるデータ上書きリスク** (行 221-222)
- `NextPostId` は `u64` + `saturating_add(1)` のため、`u64::MAX` 到達後は同一IDで上書きされる
- 修正: `checked_add` を使いオーバーフロー時にエラーを返す

**PP-C02: MerkleRoot の重複チェックが無い** (行 243)
- `MerkleRootToPostId` への挿入前に既存キーの確認がない。同一 `merkle_root` で2回投稿すると逆引きが上書きされる
- 修正: `ensure!(!MerkleRootToPostId::<T>::contains_key(merkle_root), Error::<T>::DuplicateMerkleRoot)`

**PP-C03: コスト計算の型変換失敗でコスト0** (行 201-209)
- `BalanceOf<T>` → `u128` の `try_into()` 失敗時に `unwrap_or(0)` でコストが0になる
- 最終的な `total_cost` → `BalanceOf<T>` 変換失敗時は基本コストのみ課金
- 修正: 変換失敗時はエラーを返す。`BalanceOf<T>` 型のまま計算を行う

#### HIGH

**PP-H04: `total_size` に上限チェックが無い** (行 186)
- `MaxContentLength` が Config に定義されているが `create_post` で未使用（デッドコード）
- 修正: `ensure!(total_size <= T::MaxContentLength::get() as u64, Error::<T>::ContentTooLong)`

**PP-H05: Weight計算が不正確** (行 180)
- `reads_writes(3, 4)` だが実際は reads=4以上、writes=5。CPU重みもなし。ベンチマークなし
- 修正: ベンチマークを作成して正確なweight値を算出

**PP-H06: `n` パラメータに上限チェックが無い** (行 192)
- SSS断片数に上限なし。`n = u32::MAX` が指定可能
- 修正: 合理的上限（例: `n <= 255`）を追加

#### MEDIUM

**PP-M07: `MaxContentLength` が未使用** (行 94-95)
- Config trait に定義されているが `create_post` で参照されていない

**PP-M08: UserPostsの上限チェックがトークン焼却より後** (行 212-248)
- トークン焼却後にUserPosts上限（1000件）チェック。Substrate のアトミック巻き戻しで実害はないが、実行順序が非効率

**PP-M09: ベンチマークが存在しない**

#### LOW

**PP-L10: `sha2` クレートが依存に含まれるが未使用** (Cargo.toml 行 21)

**PP-L11: イベントに V2 固有の情報が不足** (行 141-147)
- `PostCreated` に `k`, `n`, `size` が含まれていない

---

### 1.2 pallet-faucet (`apps/blockchain/pallets/faucet/src/lib.rs`)

#### CRITICAL

**PF-C01: validate_unsigned でのリプレイ攻撃防止が不完全** (行 225-230)
- `and_provides` が `(account, block_number)` のみ。同じ `nonce` で異なる `block_number` を指定可能
- 修正: `and_provides` に `nonce` も含める

#### HIGH

**PF-H02: PoW 計算の CPU 重み未計上** (行 127)
- `blake2_256` のCPU重みが含まれていない。低コストでバリデータに重い計算を強制するDoSベクトル

**PF-H03: `count_leading_zero_bits` のオーバーフロー** (行 280-291)
- `count: u8` が全ゼロハッシュ(32バイト)で `256` になりオーバーフロー
- 修正: `u16` に変更するか `saturating_add` を使用

#### MEDIUM

**PF-M04: ベンチマークが存在しない**

**PF-M05: `pallet_balances` が通常依存に含まれている** (Cargo.toml 行 22)
- ソースコード内で直接使用されていない。`[dev-dependencies]` に移動すべき

**PF-M06: `validate_unsigned` と `claim` でロジックが重複** (行 130-234)
- 同じ検証ロジックが2箇所にコピー。メンテナンスリスク

---

### 1.3 pallet-storage (`apps/blockchain/pallets/storage/src/lib.rs`)

#### CRITICAL

**PS-C01: `register_fragment` にアクセス制御が無い** (行 219-247)
- 任意のアカウントが無制限にフラグメントメタデータを登録可能。トークンコスト/デポジットなし
- `Fragments` ストレージマップに上限なし
- 修正: デポジット要求、Post パレットからの内部呼び出し限定、またはユーザーあたり制限

#### HIGH

**PS-H02: `declare_holding` で冪等性処理時にもイベントが発行** (行 358-386)
- 同じフラグメントを何度もdeclareすると状態変更なしにイベントが毎回発行される
- 修正: 新規追加時のみイベント発行

**PS-H03: `FragmentHolders` と `NodeHoldings` の整合性が保証されない** (行 358-381)
- 両ストレージの更新間に不整合が生じる可能性

**PS-H04: `validate_peer_id` のバリデーションが弱すぎる** (行 426-429)
- PeerID検証が `len() >= 2` のみ。libp2p PeerIDは通常38-52バイト
- 修正: 最小長38バイト程度に設定、multihashプレフィックス検証

**PS-H05: Weight計算が不正確** (行 218等)
- 全extrinsicで `Weight::from_parts(10_000, 0)` の固定値。ベンチマーク未反映

#### MEDIUM

**PS-M06: `register_fragment` の creator に権限チェックなし** (行 224)

**PS-M07: `unregister_node` で `NodeHoldings` が削除されない** (行 334-336)

**PS-M08: ベンチマークの `WeightInfo` が未実装**

**PS-M09: テストファイルで mock.rs が重複定義**

---

### 1.4 クロスパレット

#### HIGH

**PX-H01: Post パレットと Storage パレット間の連携が未実装**
- `create_post` で記録されたフラグメントが Storage パレットに実際に登録されているかの検証なし

#### MEDIUM

**PX-M02: テストカバレッジのギャップ**
- Post: UserPosts上限到達時、total_size=0/u64::MAX、MerkleRoot重複
- Faucet: validate_unsigned の直接テスト
- Storage: MaxHoldersPerFragment/MaxFragmentsPerNode到達時

---

## 2. Storage Node (`apps/storage-node/`)

#### CRITICAL

**SN-C01: CORS全オリジン許可 + 認証なし** (`src/rpc/mod.rs` 行 90-102)
- `CorsLayer` が `allow_origin(Any)`, `allow_methods(Any)`, `allow_headers(Any)`
- `0.0.0.0:3030` で無認証HTTPサーバーが公開
- 修正: CORS無効化 or 制限、`127.0.0.1` バインド、認証メカニズム追加

**SN-C02: HTTP RPCに認証・認可なし** (`src/rpc/mod.rs` 行 90-102)
- ネットワーク上の誰でもフラグメントの保存・取得が可能

**SN-C03: RPCリクエストボディサイズ制限なし** (`src/rpc/mod.rs` 行 105-108)
- 巨大JSONリクエストによるOOM攻撃が可能
- 修正: `DefaultBodyLimit::max(512 * 1024)` を追加

**SN-C04: 秘密鍵ファイルのパーミッション未設定** (`src/identity.rs` 行 46)
- `keypair.bin` がデフォルトパーミッション(0644)で作成。他ユーザーから読み取り可能
- 修正: `OpenOptions` で `mode(0o600)` を設定

**SN-C05: P2P Put リクエストのメッセージサイズ制限が過大** (`src/network/mod.rs` 行 64-65, 313)
- 10MBまでのメッセージ受信可能。悪意あるピアがメモリ消費攻撃可能
- 修正: `MAX_FRAGMENT_SIZE + オーバーヘッド` (約512KB) に制限

#### HIGH

**SN-H01: 自作base64エンコーダ/デコーダ** (`src/rpc/mod.rs` 行 271-336)
- `base64` クレートを使わず独自実装。バグリスクあり
- 修正: `base64` クレートを使用

**SN-H02: `serde_json::to_value(...).unwrap()` の使用** (`src/rpc/mod.rs` 行 188, 236)
- 本番コードでのunwrapはパニックによるサービスクラッシュ

**SN-H03: `.expect()` によるHTTPサーバークラッシュ** (`src/main.rs` 行 93)

**SN-H04: 容量カウンタの競合状態 (TOCTOU)** (`src/storage/mod.rs` 行 88-115)
- 容量チェックと更新間にロックなし。`Ordering::Relaxed` で可視性も保証されない
- 修正: `compare_exchange` ループまたはMutexを使用

**SN-H05: `calculate_usage` でディスクを2回走査** (`src/storage/mod.rs` 行 359-380)

**SN-H06: `hash()` メソッドが未使用** (`src/storage/mod.rs` 行 354-356)

**SN-H07: Metricsモジュールが未統合** (`src/metrics.rs`)
- Metrics構造体が定義済みだがmain.rsで未使用

#### MEDIUM

**SN-M01: P2P通信で送信元の検証なし** (`src/network/mod.rs` 行 313-324)

**SN-M02: JSON-RPC の jsonrpc バージョン未検証** (`src/rpc/mod.rs` 行 105-134)

**SN-M03: retrieve で読み込みサイズが無制限** (`src/storage/mod.rs` 行 136-137)

**SN-M04: ハートビートで接続状態を管理していない** (`src/main.rs` 行 134-139)

**SN-M05: `register_with_blockchain` のHTTPクライアントを毎回生成** (`src/chain/mod.rs` 行 252)

**SN-M06: `RpcRequest.id` が u32 固定** (`src/rpc/mod.rs` 行 25)

**SN-M07: `Ordering::Relaxed` の一貫性のない使用** (`src/storage/mod.rs` 行 89, 115)

**SN-M08: `connected` フィールドが常に `false`** (`src/chain/mod.rs` 行 73, 89)

#### LOW

**SN-L01: main関数が174行** (`src/main.rs` 行 40-174)

**SN-L02: base64モジュールのhexフォールバックが文書化されていない**

**SN-L03: エッジケーステスト不足**

**SN-L04: 統合テストのカバレッジ不明**

**SN-L05: `store_post_fragment` と `store` でバリデーションロジック重複** (`src/storage/mod.rs` 行 73-124, 186-248)

---

## 3. Frontend (`apps/frontend/`)

#### CRITICAL

**FE-C01: `unsafeApi` の `any` 型の多用**
- `useApi.ts`, `PostForm.tsx`, `WalletConnect.tsx`, `FaucetButton.tsx`, `useFaucet.ts`, `useMoralBalance.ts`, `usePostCost.ts` 全てで `any` 型
- 型安全性が完全に失われ、実行時エラーのリスク
- 修正: 使用するquery/tx/constantsのインターフェースを定義して部分的に型付け

**FE-C02: `navigator.clipboard.writeText` のエラーハンドリング欠如** (`WalletConnect.tsx` 行 100, 108)
- 非HTTPSコンテキストや権限拒否時に未処理例外
- 修正: `try/catch` で囲む

#### HIGH

**FE-H01: `console.log` / `console.warn` のプロダクション残留**
- `PostForm.tsx`, `PostItem.tsx`, `Timeline.tsx`, `useApi.ts`, `useFaucet.ts`, `usePostCost.ts`(6箇所), `useStorage.ts`(5箇所), `useMoralBalance.ts`, `crypto.ts`, `context.tsx`
- 修正: プロダクションビルドで除去するか、ロガーを導入

**FE-H02: `PostForm.tsx` の `parseError` 関数の型安全性** (行 42)
- `any` 型 + `TranslateFunc` キャスト。存在しないキーを渡す可能性

**FE-H03: `useFaucet` の `startMining` 関数が約180行** (`useFaucet.ts` 行 79-263)
- ブロック情報取得、難易度計算、Worker管理、トランザクション送信が全て1関数内
- 修正: ステップごとに関数分割

**FE-H04: `useEffect` 内の非同期処理でメモリリーク** (`PostItem.tsx` 行 46-88)
- コンポーネントアンマウント後に `setState` が呼ばれる可能性
- 修正: `AbortController` またはキャンセルフラグを追加

**FE-H05: `client._request` のプライベートAPI使用** (`useFaucet.ts` 行 98, 211)
- PAPIの非公開内部APIで、バージョンアップで破壊される可能性

**FE-H06: `useFaucet` の `status` が依存配列に含まれている** (`useFaucet.ts` 行 264)
- `status` 変更のたびに `startMining` 再生成 → 不要な再レンダリング

**FE-H07: エラーバウンダリが存在しない**
- Wasm初期化失敗、WebSocket切断等で白画面になる
- 修正: `app/layout.tsx` レベルでのError Boundary実装

#### MEDIUM

**FE-M01: ハードコードされた日本語文字列**
- `PostItem.tsx`, `page.tsx`, `useSeedPhrase.ts`, `useMoralBalance.ts`, `useApi.ts`, `Timeline.tsx`
- i18nシステムがあるのに未使用箇所あり

**FE-M02: `textarea` の `maxLength` とバイト数制限の不一致** (`PostForm.tsx` 行 195)
- `maxLength={10000}` は文字数制限。日本語(3bytes/char)では約3,333文字でバイト上限到達

**FE-M03: `shortenAddress` が毎レンダリング再生成** (`Timeline.tsx` 行 158-161)

**FE-M04: `ContentRef` インターフェースの重複定義** (`PostItem.tsx` 行 8-13, `Timeline.tsx` 行 9-14)

**FE-M05: `<html lang="en">` のハードコード** (`layout.tsx` 行 24)
- i18n言語切替時にHTML lang属性が更新されない

**FE-M06: `postEntries.map` の型が `any`** (`Timeline.tsx` 行 125)

**FE-M07: `setTimeout` のクリーンアップ欠如**
- `PostForm.tsx`, `useFaucet.ts`, `WalletConnect.tsx`, `FaucetButton.tsx`

**FE-M08: `userScalable: false` によるアクセシビリティ違反** (`layout.tsx` 行 9)
- WCAG 2.1 Level AA 準拠違反

**FE-M09: `textarea` にラベルがない** (`PostForm.tsx` 行 190-197, `WalletConnect.tsx` 行 210-219)

**FE-M10: `useStorage` の `merkleCache` がメモリリークの可能性** (`crypto.ts` 行 13)
- Worker内のMapが際限なく成長

#### LOW

**FE-L01: `contentHash` が `PostItem` に渡されているが未使用** (`PostItem.tsx` 行 19)

**FE-L02: `useStorage` の `recoverContent` で `n` パラメータ未使用** (`useStorage.ts` 行 266-267)

**FE-L03: `WalletConnect` コンポーネントが293行**

**FE-L04: `RPC_ENDPOINT` のプロトコル変換が安全でない** (`useStorage.ts` 行 8)

**FE-L05: テストカバレッジの不足**
- `PostForm`, `WalletConnect`, `PostItem`, `useApi`, `useMoralBalance`, `usePostCost`, `useSeedPhrase` のテストなし

**FE-L06: `useSeedPhrase` フックが未使用** (`useSeedPhrase.ts`)
- `WalletConnect.tsx` が独自にシードフレーズ管理しており重複

**ポジティブな点:**
- `dangerouslySetInnerHTML` 未使用 (XSS安全)
- APIキーやシークレットの露出なし
- シードフレーズが接続後にクリアされている
- Web Worker による暗号処理のオフロードが適切
- Worker のクリーンアップ (`terminate()`) が実装済み

---

## 4. WASM Crypto Engine (`packages/wasm-engine/`)

#### CRITICAL

**WE-C01: SSS分割でのRNG安全性の暗黙的依存** (`src/sss.rs` 行 43-44)
- `sharks` クレートが `thread_rng()` を使用。Wasm環境では `getrandom` の `js` featureによりCSPRNGに委譲されるが、この依存関係が文書化されていない

#### HIGH

**WE-H01: SSSの `k` と `n` パラメータに上限値チェック欠如** (`src/sss.rs` 行 38-41)
- 大きなnで `dealer.take(n)` を呼ぶとOOMの可能性
- 修正: `n <= 20` の上限追加

**WE-H02: Merkle Proofの `total_leaves` パラメータの検証不足** (`src/merkle.rs` 行 159-176)
- `total_leaves == 0` や `leaf_index >= total_leaves` のバリデーションが欠如

#### MEDIUM

**WE-M01: `serde` 依存が宣言されているが未使用** (Cargo.toml 行 30)
- Wasmバイナリサイズが不要に増加

**WE-M02: テストカバレッジ不足 - エッジケース**
- `k == n`, `k == 1`, 空データ, 単一リーフMerkleTree のテストなし

**WE-M03: `console_error_panic_hook` がfeatureゲートに依存** (`src/lib.rs` 行 13-16)

---

## 5. Scripts (`scripts/`)

#### CRITICAL

**SC-C01: `sudo-mint.mjs` が本番環境で実行可能**
- 環境チェックなし。`pallet_sudo` が残存する限り本番ノードで任意トークン発行可能
- 修正: 環境チェック追加、長期的には `pallet_sudo` 削除

#### HIGH

**SC-H01: シードフレーズの部分的ログ出力** (`mint-moral-seed.mjs` 行 73)
- 最初の3単語がコンソールに出力。検索空間が79ビットに減少
- 修正: 単語数のみ表示

**SC-H02: 全スクリプトで `DEV_PHRASE` + 外部接続可能** (`mint-moral.mjs`, `sudo-mint.mjs`, `transfer-native.mjs`, `mint-moral-seed.mjs`)
- `WS_ENDPOINT` 環境変数で本番ノードに接続可能
- 修正: ローカルホスト以外の場合に警告表示

#### MEDIUM

**SC-M01: `testAccounts` マッピングが3スクリプトで重複**

**SC-M02: アドレスバリデーション欠如** (`mint-moral.mjs`, `transfer-native.mjs`)

**SC-M03: `process.exit(0)` がエラー時にも実行** (`mint-moral.mjs`, `mint-moral-seed.mjs`)

#### LOW

**SC-L01: `transfer-native.mjs` のコメントとデフォルト値が不一致** (行 10 vs 20)

---

## 6. Runtime Configuration (`apps/blockchain/runtime/src/lib.rs`)

#### CRITICAL

**RT-C01: トランザクション手数料が完全に0** (行 178-180)
- `WeightToFee` と `LengthToFee` が `ConstU128<0>`。`ChargeTransactionPayment` も削除済み
- Post以外のextrinsic（`System.remark`, `Balances.transfer` 等）が無料で実行可能
- **攻撃ベクトル**: システムコールスパム、ストレージ攻撃、mempool飽和
- 修正: 最低限のトランザクション手数料を設定し `ChargeTransactionPayment` を復活

**RT-C02: `pallet_sudo` が本番ランタイムに含まれている** (行 186-190, 239)
- 分散型SNSの匿名性・分散化原則に反する
- 修正: メインネット前に削除計画を策定

#### HIGH

**RT-H01: `ExistentialDeposit = 1` (実質0)** (行 155)
- 12桁精度で `0.000000000001 MORAL`。ダストアカウント攻撃が可能
- 手数料0と合わせてコストゼロで大量アカウント作成可能
- 修正: `ConstU128<1_000_000_000_000>` (1 MORAL) 程度に設定

**RT-H02: `BlockWeights` / `BlockLength` がデフォルト値で未カスタマイズ** (行 109-110)
- `MAXIMUM_BLOCK_WEIGHT` / `MAXIMUM_BLOCK_LENGTH` が定数定義されているが未使用（デッドコード）
- `proof_size = u64::MAX` も問題

**RT-H03: GRANDPA Equivocation報告が無効化** (行 131-139, 377-393)
- バリデータの二重署名に罰則なし

#### MEDIUM

**RT-M01: `WeightInfo = ()` が全パレットで使用** (複数箇所)
- ベンチマーク未実行。Weight値が実際の計算コストと乖離

**RT-M02: Post palletの `MaxContentLength = 10,000` の意味の不整合** (行 195)
- MerkleRootのみオンチェーン保存する設計との整合性が不明

#### LOW

**RT-L01: `MaxNominators = 0` のコメント不足** (行 135)

**RT-L02: `SS58Prefix = 42` が開発用デフォルト** (行 118)
- メインネットデプロイ時に独自プレフィックスを登録すべき

---

## 推奨対応ロードマップ

### Phase 1: 緊急対応（セキュリティ）
1. Runtime: トランザクション手数料の導入 (RT-C01)
2. Runtime: ExistentialDeposit を適切な値に設定 (RT-H01)
3. Storage Node: CORS制限 + バインドアドレスをlocalhostに変更 (SN-C01, SN-C02)
4. Storage Node: リクエストボディサイズ制限追加 (SN-C03)
5. Storage Node: 秘密鍵ファイルパーミッション設定 (SN-C04)
6. pallet-post: MerkleRoot重複チェック追加 (PP-C02)
7. pallet-post: コスト計算の型変換修正 (PP-C03)
8. pallet-storage: register_fragment にアクセス制御追加 (PS-C01)

### Phase 2: 重要な品質改善
1. 全パレット: ベンチマーク作成とWeight値の修正
2. Storage Node: `base64` クレート導入、unwrap除去
3. Frontend: Error Boundary実装
4. Frontend: console.log の除去
5. Scripts: 環境チェックとシード漏洩防止

### Phase 3: メインネット準備
1. Runtime: `pallet_sudo` 削除計画
2. Runtime: GRANDPA Equivocation報告の有効化
3. Runtime: SS58Prefix の独自登録
4. 全体: テストカバレッジの改善
5. Frontend: 型安全性の改善（`any` 削減）

---

## 付録: ポジティブな評価点

- XSS対策: `dangerouslySetInnerHTML` 未使用、投稿コンテンツはテキストノードとして表示
- クライアントサイド暗号: SSS/Merkle処理がWasm + Web Workerで適切にオフロード
- Worker管理: `terminate()` によるクリーンアップが実装済み
- シードフレーズ管理: 接続後にメモリからクリア
- i18n: 3言語（日/英/中）サポートが概ね完了
- P2P基盤: libp2pベースの分散ストレージアーキテクチャ
- SSS実装: Shamir Secret Sharingによるコンテンツ断片化が動作
