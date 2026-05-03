# Feature Specification: PoW Faucet（匿名アカウント初期化）

**Feature Branch**: `007-pow-faucet`  
**Created**: 2026-02-09  
**Status**: Draft  
**Input**: User description: "PoW Faucet - 匿名アカウント初期化のためのProof-of-Work Faucetを実装する。ユーザーがブラウザでPoW計算を行い、初期$moralトークンを取得できる仕組み。KYC不要・IPログなし。"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - 新規ユーザーの初期トークン取得 (Priority: P1)

新規ユーザーが初めてAnarchyに参加するとき、KYCやメールアドレスなしに初期$moralトークンを取得できる。ブラウザ上でProof-of-Work計算を完了するだけで、匿名のまま投稿を始められる状態になる。

**Why this priority**: Anarchyの根幹である「匿名性」と「参入障壁の低さ」を両立する最も重要なフロー。これがないと新規ユーザーはトークンを持つ既存ユーザーから貰うしかない。

**Independent Test**: WalletConnect内の残高表示下にあるFaucetボタンを押してPoW計算を開始し、完了後にウォレット残高が0から初期$moralに増加することを確認。

**Acceptance Scenarios**:

1. **Given** ユーザーが残高0の新規アカウントを持っている, **When** Faucetボタンを押してPoW計算を完了する, **Then** 指定量の初期$moralがアカウントに付与される
2. **Given** ユーザーがPoW計算を開始した, **When** ブラウザタブを閉じずに待機する, **Then** 計算進捗が表示され、完了までの目安時間がわかる
3. **Given** ユーザーのデバイスが低スペックである, **When** PoW計算を行う, **Then** 時間はかかるが完了可能であり、タイムアウトしない

---

### User Story 2 - シビル攻撃の防止 (Priority: P1)

悪意あるユーザーが大量のアカウントを作成して$moralを不正に蓄積しようとしても、各アカウントに対してPoW計算コストが発生するため、経済的に非合理な攻撃となる。

**Why this priority**: トークン経済の健全性を保つための必須要件。PoWなしでは無限にトークンを発行でき、システムが破綻する。

**Independent Test**: 同一アカウントで2回目のFaucet請求を試行し、拒否されることを確認。また、100アカウント分のトークン取得には100回分のPoW計算時間が必要なことを確認。

**Acceptance Scenarios**:

1. **Given** ユーザーが既にFaucetを利用済み, **When** 同じアカウントで再度Faucet請求を行う, **Then** 「既に利用済み」エラーが返される
2. **Given** 攻撃者が自動化スクリプトでFaucetを回す, **When** 各リクエストでPoW解を提出する, **Then** 各解の計算に数十秒かかり、大量取得が非効率になる
3. **Given** ネットワーク全体でのFaucet利用が増加, **When** 難易度閾値を超える, **Then** PoW難易度が自動調整されてインフレを抑制する

---

### User Story 3 - 匿名性の保持 (Priority: P2)

Faucet利用時に個人情報（IPアドレス、メールアドレス、電話番号など）が一切記録されない。Tor経由でのアクセスでも問題なく利用できる。

**Why this priority**: Anarchyの核心的価値である匿名性を担保。IP制限やCaptchaを使わないことで、検閲耐性を維持する。

**Independent Test**: Tor Browser経由でFaucetを利用し、正常に$moralを取得できることを確認。

**Acceptance Scenarios**:

1. **Given** ユーザーがTor Browser経由でアクセス, **When** Faucetを利用する, **Then** 正常に$moralを取得できる
2. **Given** ユーザーがFaucetを利用した, **When** チェーン上のデータを確認する, **Then** IPアドレスやタイムスタンプの詳細な紐付け情報は存在しない
3. **Given** Faucetのエクストリンシックが送信される, **When** ログを確認する, **Then** IPアドレスは記録されていない

---

### User Story 4 - エラーハンドリング (Priority: P1)

既にFaucetを利用済みのユーザーがボタンを押しても、適切なエラーメッセージが表示される。フロントエンドではボタンを何度でも押せるが、ブロックチェーン側で正しく拒否される。

**Why this priority**: UX必須要件。エラー時にユーザーが状況を理解できないと混乱する。

**Independent Test**: 既にFaucet利用済みのアカウントでボタンを押し、「既に利用済み」エラーが表示されることを確認。

**Acceptance Scenarios**:

1. **Given** ユーザーが既にFaucetを利用済み, **When** Faucetボタンを押す, **Then** 「既に利用済み」エラーが日本語/英語で表示される
2. **Given** PoW計算が完了したがチャレンジが期限切れ, **When** 提出する, **Then** 「チャレンジ期限切れ」エラーが表示される
3. **Given** ネットワークエラーが発生, **When** 提出に失敗, **Then** リトライを促すエラーメッセージが表示される

---

### Edge Cases

- **チャレンジ有効期限切れ**: PoW計算中にブロックが進み、チャレンジが古くなった場合 → 一定ブロック数（例: 100ブロック）以内なら有効とする
- **ブラウザクラッシュ**: 計算途中でブラウザが落ちた場合 → 再開は不可。新しいチャレンジを取得して再計算が必要
- **難易度の急変**: 難易度調整中に計算を開始したユーザー → チャレンジ取得時の難易度で検証する
- **残高がある状態でのFaucet利用**: 既に$moralを持っているアカウントでも1回は使えるか？ → 残高に関係なく「1アカウント1回のみ」に制限
- **Genesis時点での難易度**: ネットワーク開始直後は過去のFaucet利用データがない → デフォルト難易度を設定

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST `pallet-faucet`を実装し、PoWチャレンジの生成・検証・報酬付与を行う
- **FR-002**: System MUST ブロックハッシュをベースにPoWチャレンジを生成する（予測不可能性の担保）
- **FR-003**: System MUST nonce検証により正しいPoW解のみを受け入れる
- **FR-004**: System MUST 各アカウントにつき1回のみFaucet利用を許可する
- **FR-005**: System MUST PoW解の検証に成功した場合、設定された初期$moral量をアカウントに付与する
- **FR-006**: System MUST 難易度をFaucet利用済みアカウント数に応じて動的に調整する（`difficulty = base + floor(log2(1 + claims / scaling_factor))`）
- **FR-007**: System MUST チャレンジの有効期限（ブロック数）を設定可能にする
- **FR-011**: System MUST 難易度の上限（max_difficulty）を設定し、計算時間が無限に増加しないようにする
- **FR-008**: フロントエンドはWeb Workerを使用してPoW計算を行い、メインスレッドをブロックしない
- **FR-009**: フロントエンドは残高表示の下にFaucetボタンを配置する（WalletConnect内）
- **FR-010**: System MUST IPアドレスやその他の個人識別情報をログに記録しない
- **FR-012**: フロントエンドはボタンを何度でも押せる状態とし、重複制限はブロックチェーン側で行う
- **FR-013**: フロントエンドはブロックチェーンからのエラーを適切にローカライズして表示する（AlreadyClaimed, ChallengeExpired, InvalidProof）

### Key Entities *(include if feature involves data)*

- **Challenge**: PoWパズルの問題。ブロックハッシュから導出され、targetDifficulty（満たすべきハッシュ条件）を含む
- **Solution**: ユーザーが計算したnonce値。Challenge + nonceのハッシュがtargetDifficultyを満たす必要がある
- **FaucetClaim**: 特定アカウントがFaucetを利用した記録。二重利用防止に使用
- **DifficultyConfig**: 動的難易度設定。base_difficulty（初期難易度）、scaling_factor（難易度上昇の倍率）、max_difficulty（上限）を含む
- **TotalClaims**: Faucet利用済みアカウント総数。難易度計算に使用

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 平均的なデバイス（2020年代のミドルレンジPC/スマートフォン）でPoW計算が完了する（初期: 3-10秒、成熟期: 60-180秒）
- **SC-002**: 新規ユーザーがFaucetボタンを押してから$moralを取得するまでの全体フローが5分以内に完了する（成熟期でも）
- **SC-003**: 同一アカウントでの二重請求が100%ブロックされる
- **SC-004**: Tor Browser経由でのFaucet利用成功率が95%以上
- **SC-005**: PoW計算中のCPU使用率が高くてもUIがフリーズしない（メインスレッドのブロック時間が100ms未満）

### Testing Requirements

#### Pallet Tests (Rust)
すべてのFunctional RequirementsをRustユニットテストでカバーする:

- **T-001**: 正しいPoW解でclaimが成功し、残高が増加する
- **T-002**: AlreadyClaimed - 同一アカウントで2回目のclaimが拒否される
- **T-003**: ChallengeExpired - 期限切れブロック番号で拒否される
- **T-004**: InvalidProof - 難易度を満たさないnonceで拒否される
- **T-005**: BlockNotFound - 存在しないブロック番号で拒否される
- **T-006**: 動的難易度 - TotalClaimsに応じて難易度が正しく計算される
- **T-007**: 難易度上限 - max_difficultyを超えないことを確認
- **T-008**: TotalClaimsカウンタ - claim成功時に+1される

#### Frontend Tests (Jest)
- **T-101**: Faucetボタンが残高表示の下に表示される
- **T-102**: ボタンクリックでWorkerが起動しPoW計算が開始される
- **T-103**: 計算成功後にトランザクションが送信される
- **T-104**: AlreadyClaimedエラーが日本語で表示される
- **T-105**: ChallengeExpiredエラーが日本語で表示される
- **T-106**: 計算中はローディング状態が表示される
- **T-107**: エラー後もボタンは再度押せる状態になる

#### Integration Tests
- **T-201**: E2E: 新規アカウントでFaucet利用→残高増加
- **T-202**: E2E: 利用済みアカウントでFaucet利用→エラー表示

## Assumptions

- ブラウザはWeb Workers APIをサポートしている（モダンブラウザ）
- Blake2bのJavaScript実装（blakejs）が利用可能
- 難易度はFaucet利用数に応じて自動調整される
- 報酬量は固定値（100 MORAL）とする

## Out of Scope

- Captchaや他のボット対策手法との併用
- Faucet利用回数の緩和（1回限り→期間ベース等）
- 法定通貨との交換レート考慮
- 反応マイニング・ストレージ報酬との連携（別仕様で検討）
