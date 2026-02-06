## 1. プロジェクト概要

**プロジェクト名:** Anarchy
**コンセプト:** 支配なき秩序（Order without Masters）.
**概要:** 中央集権的な管理者を介さず,数学的・経済的メカニズムによってユーザーの言論の自由を保護するL1ブロックチェーンベースの分散型SNSプロトコル.

## 2. コーディングおよびドキュメント規格（最優先）

* **記述言語:** コード内のコメントおよびドキュメントは,論理的かつ明確な日本語（上記ルール適用）または英語で行うこと.

## 3. セキュリティ・アノニミティ原則（妥協不可）

1. **ネットワーク秘匿:** libp2pのトランスポート層にTor/I2Pを強制統合し,IPアドレス等のメタデータを物理的に遮断すること.
2. **秘密鍵の排除:** ユーザーに秘密鍵（シードフレーズ）を扱わせないこと. WebAuthn（パスキー）とアカウント抽象化（AA）を組み合わせ,Secure Enclave等での署名を前提とする.
3. **クライアントサイド完結:** 暗号化,断片化（SSS）,メタデータ削除は必ずクライアント側で実行してから送信すること.
4. **フォアグラウンド処理:** 反応マイニング（PoW）は,マルウェア判定を避けるため,原則としてフォアグラウンドかつユーザーの可視範囲で実行し,Page Visibility API等で制御すること.

## 4. 技術スタック

* **L1 Core:** Rust / Substrate (Polkadot SDK).
* **P2P Networking:** libp2p (Tor / I2P Native Integration).
* **Authentication:** WebAuthn (Passkeys) / Account Abstraction (ERC-4337).
* **Cryptography:** * Zero-Knowledge Proofs (Circom / Noir).
* Shamir's Secret Sharing (SSS).
* Stealth Addresses (ECDH).


* **Front-end:** Next.js (PWA) / TypeScript / WebAssembly (Wasm).

## 5. 実装ロードマップ（進化型プロトタイピング）

### Phase 1: セキュア・ファンデーション（現在）

* libp2pトランスポートへのTor統合による通信の完全匿名化.
* Substrateパレットでのパスキー署名検証の実装.
* 最小限の $moral トークンロジック（発行・焼却）の実装.

### Phase 2: プライバシー・レイヤー

* SSSによるデータの断片化・分散ストレージ報酬の実装.
* ステルスアドレスによる取引履歴の匿名化.

### Phase 3: 自律エコシステム

* 反応マイニング（PoW）と動的難易度調整の統合.
* ZKPによる匿名人間証明の実装.

## 6. ディレクトリ構造（モノレポ）

```text
anarchy/
├── apps/
│   ├── blockchain/      # Substrate L1 Core (Rust)
│   └── frontend/        # Next.js PWA (TypeScript / Wasm)
├── packages/
│   ├── circuits/        # ZKP Circuits (Circom / Noir)
│   ├── sdk/             # Shared Cryptography SDK
│   └── wasm-engine/     # Rust-Wasm Implementation (SSS, PoW)
├── agent.md             # This file
└── pnpm-workspace.yaml  # Workspace configuration

```

---