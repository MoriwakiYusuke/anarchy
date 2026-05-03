# Feature Specification: WebAuthn署名検証

**Feature Branch**: `002-webauthn-verification`  
**Created**: 2026-02-07  
**Status**: Draft  
**Input**: User description: "WebAuthn署名検証の実装: Rust署名検証ライブラリ、COSE公開鍵パーサー、ES256署名検証、Substrate統合"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - 投稿時のWebAuthn署名検証 (Priority: P1)

ユーザーがパスキーを使って投稿を行う際、その署名がオンチェーンで検証され、本人の意図した投稿であることが証明される。ユーザーはデバイスの生体認証（指紋・顔認証）を行うだけで、裏側ではWebAuthn署名が生成・検証される。

**Why this priority**: これがWebAuthn検証機能の中核。投稿という主要アクションで署名を検証することで、なりすましやリプレイ攻撃を防止し、WYSIWYS（What You See Is What You Sign）を実現する。

**Independent Test**: テスト用のWebAuthn署名データを用意し、オンチェーンで署名検証が成功/失敗するシナリオを実行可能。

**Acceptance Scenarios**:

1. **Given** ユーザーがIdentityを登録済み（公開鍵がオンチェーンに保存済み）, **When** 正しいWebAuthn署名付きで投稿を送信, **Then** 署名検証が成功し、投稿がオンチェーンに記録される
2. **Given** ユーザーがIdentityを登録済み, **When** 不正な署名（改ざんされた署名、異なる秘密鍵で署名）で投稿を送信, **Then** 署名検証が失敗し、投稿は拒否される
3. **Given** ユーザーがIdentityを登録済み, **When** challengeに含まれる投稿ハッシュと実際の投稿内容が一致しない, **Then** 署名検証が失敗し、投稿は拒否される（WYSIWYS保証）

---

### User Story 2 - COSE公開鍵の解析と保存 (Priority: P2)

ユーザーがパスキーを登録する際、WebAuthnから返される COSE フォーマットの公開鍵が正しくパースされ、後の署名検証に使用できる形式で保存される。

**Why this priority**: 公開鍵のパースは署名検証の前提条件。正しくパースできなければ検証自体が不可能になる。

**Independent Test**: 様々なフォーマットのCOSE公開鍵（ES256、RS256等）を入力し、ES256が正しくパースされることを検証。

**Acceptance Scenarios**:

1. **Given** ユーザーがWebAuthnでパスキーを作成, **When** ES256（P-256曲線）のCOSE公開鍵が返される, **Then** 公開鍵が正しくパースされ、x座標・y座標が抽出されてオンチェーンに保存される
2. **Given** 不正なフォーマットのCOSE公開鍵, **When** 公開鍵の登録を試みる, **Then** パースエラーが返され、登録は拒否される
3. **Given** サポート外のアルゴリズム（RS256等）のCOSE公開鍵, **When** 登録を試みる, **Then** 「サポートされていないアルゴリズム」エラーが返される

---

### User Story 3 - authenticatorDataとclientDataJSONの検証 (Priority: P3)

WebAuthn署名に含まれるauthenticatorDataとclientDataJSONが正しく検証され、リプレイ攻撃やオリジン偽装を防止する。

**Why this priority**: セキュリティ強化のための追加検証。P1の署名検証だけでも基本機能は動作するが、本格運用には必須。

**Independent Test**: 様々なauthenticatorDataとclientDataJSONのパターン（正常、改ざん、リプレイ）でテスト実行。

**Acceptance Scenarios**:

1. **Given** 正しいauthenticatorDataを含む署名, **When** 検証を実行, **Then** rpIdHashが一致し、フラグが正しいことが確認され、検証成功
2. **Given** clientDataJSONのchallengeが期待値と異なる, **When** 検証を実行, **Then** チャレンジ不一致エラーで検証失敗
3. **Given** userPresentフラグが立っていないauthenticatorData, **When** 検証を実行, **Then** ユーザー不在エラーで検証失敗

---

### Edge Cases

- 署名のDER形式とraw形式の両方に対応する（自動検出）
- 公開鍵は非圧縮形式（65バイト: 0x04 || x || y）のみサポート
- challengeのbase64urlエンコーディングのパディング有無を正規化して対応
- オンチェーン実行時間に制約がある場合、署名検証が時間内に完了するか？（SC-003で6秒以内を目標）

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: システムは、COSE形式のES256（P-256曲線）公開鍵をパースし、x座標とy座標を抽出できなければならない
- **FR-002**: システムは、ES256アルゴリズムによるECDSA署名を検証できなければならない
- **FR-003**: システムは、authenticatorDataをパースし、rpIdHash、フラグ（UP、UV）、signCountを抽出できなければならない
- **FR-004**: システムは、clientDataJSONをパースし、type、challenge、originフィールドを抽出・検証できなければならない
- **FR-005**: 署名検証時、challengeに投稿コンテンツのハッシュが含まれていることを確認し、WYSIWYS（What You See Is What You Sign）を保証しなければならない
- **FR-006**: 不正な署名、改ざんされたデータ、不一致のチャレンジに対しては、明確なエラーを返して拒否しなければならない
- **FR-007**: 署名検証はSubstrateランタイム内でオンチェーン実行できなければならない
- **FR-008**: 署名はDER形式とraw形式（r||s、64バイト）の両方を自動検出して対応しなければならない
- **FR-009**: 公開鍵は非圧縮形式（65バイト: 0x04 || x || y）のみをサポートする
- **FR-010**: base64urlエンコーディングはパディング有無を正規化して両方に対応しなければならない

### Key Entities

- **COSE公開鍵**: WebAuthnから返される公開鍵のフォーマット。kty（キータイプ）、alg（アルゴリズム）、crv（曲線）、x座標、y座標を含む
- **WebAuthn署名データ**: authenticatorData + clientDataHash に対するECDSA署名。r値とs値で構成される
- **authenticatorData**: rpIdHash（32バイト）、フラグ（1バイト）、signCount（4バイト）、およびオプションの拡張データ
- **clientDataJSON**: type（"webauthn.get"）、challenge（base64url）、origin（リクエスト元）を含むJSON

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 正当なWebAuthn署名の検証成功率が100%である
- **SC-002**: 改ざんされた署名・データは100%拒否される
- **SC-003**: 署名検証を含むエクストリンシックがブロック生成時間内（目標6秒）で処理される
- **SC-004**: ユーザーはパスキーによる生体認証のみで投稿でき、秘密鍵を直接扱う必要がない

## Assumptions

- WebAuthnの署名アルゴリズムはES256（P-256曲線）のみをサポートする。RS256等の他のアルゴリズムは初期スコープ外
- フロントエンドがWebAuthn APIを正しく呼び出し、署名データを正しくエンコードして送信することを前提とする
- Substrateランタイムでの暗号計算に必要なクレート（p256、ecdsa等）がno_std環境で動作可能
- rpIdはプロジェクトで定義された固定値を使用（将来的にマルチオリジン対応は検討）

## Clarifications

### Session 2026-02-07

- Q: WebAuthn署名はDER形式とraw形式のどちらをサポートするか？ → A: DER形式とraw形式の両方を自動検出して対応
- Q: P-256公開鍵の圧縮形式と非圧縮形式のどちらをサポートするか？ → A: 非圧縮形式のみサポート（WebAuthn標準出力形式）
- Q: base64urlエンコーディングのパディング有無にどう対応するか？ → A: パディング有無を正規化して両方に対応
