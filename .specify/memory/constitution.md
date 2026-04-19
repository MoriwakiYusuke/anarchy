# Anarchy Constitution

> **支配なき秩序（Order without Masters）**
> 中央集権的な管理者を介さず、数学的・経済的メカニズムによってユーザーの言論の自由を保護する

## Core Principles

### I. Network Anonymity（ネットワーク秘匿）【NON-NEGOTIABLE】
ノード間通信を匿名化レイヤー経由で行い、IPアドレス等のメタデータがオンチェーンデータと紐付かないようにする。
- **mainnet では匿名通信を強制**: `chain_id` に `mainnet` を含む場合 `TorMode::Forced` を自動適用（開発/テストネットは `Off | OutboundOnly | Forced` から選択可）
- 実装: システム Tor デーモン + torsocks 方式（①外向きロック: `ANARCHY_RUNNING_UNDER_TORSOCKS` 環境変数チェック、②内向きロック: 127.0.0.1 バインド強制、③Onion Service 対応）
- Arti（Rust Tor）は no_std 非対応・実験的段階のため 2026-02-08 に採用を見送り
- フロントエンドへの IP 露出は「許容」するが、オンチェーンデータとの紐付けは「数学的に切断」

### II. Minimal Key Exposure（秘密鍵のアプリ非露出）【NON-NEGOTIABLE】
秘密鍵をアプリケーション層から直接扱わせない。署名は既存ウォレット（`polkadot-api` の signer インターフェース）経由のみ。
- シードフレーズベースの Substrate AccountId 認証を採用（polkadot.js / Nova wallet 等と互換）
- フロントエンド／バックエンドのコードで生の秘密鍵やシードフレーズを保持・参照しない
- 署名は都度 WYSIWYS（What You See Is What You Sign）で検証
- 将来的な WebAuthn / Account Abstraction への移行は選択肢として維持（v1.0.0 で前提としていた WebAuthn 強制は、ブラウザ互換性と COSE/CBOR 実装の複雑性を理由に 2026-02-08 に取り下げ）

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
| Consensus | Aura (dev) → NPoS/PoW (Phase 4.7 で最終決定) |
| Networking | libp2p + システム Tor + torsocks（mainnet 強制） |
| Light Client | smoldot（ブラウザ内トラストレス接続） |
| Frontend | Next.js 14 + TypeScript + PAPI |
| Crypto | Sr25519（署名）, SSS + Reed-Solomon（断片化）, AES-256-GCM（暗号化）, KZG-BLS12-381（保持証明）, X25519（ステルスアドレス鍵交換）, Blake2b（PoW/ハッシュ）, ZKP（将来: Circom/Noir） |

**重要**: @polkadot/api は使用禁止。メタデータv16対応の PAPI (polkadot-api) を使用すること。

## Security Requirements

| 信頼の対象 | セキュリティの根拠 |
|-----------|------------------|
| フロントエンド | 信頼しない（IP/投稿内容は一時的に露出） |
| 秘密鍵 | 既存ウォレットの signer 層で管理。アプリケーションコードには生の鍵・シードフレーズを保持させない |
| オンチェーンデータ | ステルスアドレス（X25519 + Ephemeral Key）と Tor（mainnet 強制）で切断 |
| 断片ストレージ | KZG-VSS ハイブリッド暗号化 + 自己修復プロトコル（k=3, n=5 SSS + Reed-Solomon） |
| システム全体 | SBOMによる検証可能性 |

## Development Workflow

1. **仕様定義**: speckit でスペック作成
2. **テスト作成**: 受け入れ条件に基づくテストを先に書く
3. **実装**: テストをパスする最小限のコードを書く
4. **レビュー**: Constitution準拠を確認
5. **統合テスト**: マルチノード環境でのテスト実行

## Governance

- この Constitution は他の全ての慣行に優先する
- 原則 I〜III（NON-NEGOTIABLE）の**本旨**（匿名通信・鍵の非露出・クライアントサイド完結）を損なう変更は禁止。実装手段（使用する技術・プロトコル）の変更は影響分析と改訂履歴への記録を経て許容
- 修正には: ドキュメント更新、影響分析、マイグレーション計画が必要
- 全ての PR/レビューは Constitution 準拠を検証すること

## 改訂履歴

- **v1.1.0 (2026-04-20)**: 実装実態との整合
  - 原則I「Network Anonymity」: Arti 強制統合 → システム Tor + torsocks 方式（mainnet 強制は維持）
  - 原則II「Keyless UX」→「Minimal Key Exposure」に再定義。WebAuthn + Account Abstraction 強制要件を取り下げ、シードフレーズベースの AccountId 認証（polkadot.js / Nova wallet 互換）を容認
  - Technology Stack / Security Requirements を現行実装（SSS + Reed-Solomon / KZG-BLS12-381 / AES-256-GCM / Blake2b PoW / smoldot Light Client）に追随
  - Governance: NON-NEGOTIABLE の解釈を「本旨の不変」に明確化（実装手段の変更は許容）
- **v1.0.0 (2026-02-07)**: 初版

**Version**: 1.1.0 | **Ratified**: 2026-02-07 | **Last Amended**: 2026-04-20
