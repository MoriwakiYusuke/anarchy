# Anarchy Constitution

> **支配なき秩序（Order without Masters）**
> 中央集権的な管理者を介さず、数学的・経済的メカニズムによってユーザーの言論の自由を保護する

## Core Principles

### I. Network Anonymity（ネットワーク秘匿）【NON-NEGOTIABLE】
libp2pトランスポート層にTor/I2Pを**強制統合**し、IPアドレス等のメタデータを物理的に遮断する。
- 「オプションとしての匿名」ではなく、プロトコルレベルで「匿名以外を許可しない」設計
- ノード間通信は必ず匿名化レイヤーを経由
- フロントエンドへのIP露出は「許容」するが、オンチェーンデータとの紐付けは「数学的に切断」

### II. Keyless UX（秘密鍵の排除）【NON-NEGOTIABLE】
ユーザーに秘密鍵（シードフレーズ）を扱わせない。
- WebAuthn（パスキー）+ アカウント抽象化（AA）でSecure Enclave署名を前提
- 秘密鍵はハードウェアから一歩も出さない
- 1 Identity ID → N Passkeys（マルチデバイス対応）
- パスワードやシードフレーズを排除し、Web2.0同等の利便性で暗号学的安全性を実現

### III. Client-Side Completion（クライアントサイド完結）【NON-NEGOTIABLE】
暗号化、断片化（SSS）、メタデータ削除は**必ずクライアント側で実行してから送信**。
- フロントエンドが悪意を持っていても、プロトコルに書き込まれる時点で浄化（クレンジング）
- ステルスアドレスとZKPによって実名との紐付けを数学的に切断

### IV. Zero-Trust Hydra（ゼロトラスト・フロントエンド）
悪意あるフロントエンド（ハイドラ）の存在を許容しつつ、プロトコル層で数学的に無効化する。
- フロントエンドを「信頼しない」前提で設計
- WYSIWYS（What You See Is What You Sign）でなりすまし防止
- 「入り口は自由、出口は浄化」パラダイム

### V. Economic Autonomy（経済的自律性）
参加者全員が「自分の利益」を追求することが、結果としてネットワークの安全と成長につながる。
- 正直者が最も得をする報酬設計（バリデーター報酬）
- ハイドラ（フロント）の自由競争による高品質サービス供給
- 需要のないデータは報酬停止により自然消滅（経済的忘却）
- 報酬計算: `Reward = Σ(Reaction × Power_cpu) × γ`

### VI. Test-First Development
全ての機能はテストから始まる。
- パレット単体テスト: `cargo test -p pallet-xxx`
- 統合テスト: ブロック同期、コンセンサス、ノードリカバリ、スケーラビリティ
- フロントエンド: E2Eテスト（将来）

## Technology Stack

| レイヤー | 技術 |
|---------|------|
| L1 Core | Rust + Polkadot SDK (stable2503) |
| Consensus | Aura (dev) → NPoS (production) |
| Networking | libp2p + Tor/I2P (Arti) |
| Frontend | Next.js 15 + TypeScript + PAPI |
| Crypto | WebAuthn, SSS, X25519, ZKP (Circom/Noir) |

**重要**: @polkadot/api は使用禁止。メタデータv16対応の PAPI (polkadot-api) を使用すること。

## Security Requirements

| 信頼の対象 | セキュリティの根拠 |
|-----------|------------------|
| フロントエンド | 信頼しない（IP/投稿内容は一時的に露出） |
| 秘密鍵 | ハードウェア（Passkey）で物理的に保護 |
| オンチェーンデータ | ステルスアドレスとTor/I2Pで切断 |
| システム全体 | SBOMによる検証可能性 |

## Development Workflow

1. **仕様定義**: speckit でスペック作成
2. **テスト作成**: 受け入れ条件に基づくテストを先に書く
3. **実装**: テストをパスする最小限のコードを書く
4. **レビュー**: Constitution準拠を確認
5. **統合テスト**: マルチノード環境でのテスト実行

## Governance

- この Constitution は他の全ての慣行に優先する
- 原則 I〜III（NON-NEGOTIABLE）の変更は禁止
- 修正には: ドキュメント更新、影響分析、マイグレーション計画が必要
- 全ての PR/レビューは Constitution 準拠を検証すること

**Version**: 1.0.0 | **Ratified**: 2026-02-07 | **Last Amended**: 2026-02-07
