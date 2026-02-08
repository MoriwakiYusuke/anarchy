# Implementation Plan: libp2p + Tor統合

**Branch**: `006-libp2p-tor` | **Date**: 2026-02-08 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/006-libp2p-tor/spec.md`

## Summary

Anarchyノード間の通信をTorネットワーク経由で実現し、ノード運営者のIPアドレスを秘匿する。段階的アプローチ：Phase 1でtorsocks外部プロキシによる送信匿名化、Phase 2でOnion Service設定による受信匿名化を実装。

**`--tor-mode=forced`の実装**:
- **① 出口ロック**: 環境変数`ANARCHY_RUNNING_UNDER_TORSOCKS`チェック、未設定なら即座にプロセス終了
- **② 入口ロック**: `listen_addresses`を`127.0.0.1:30333`に強制上書き、外部からの直接TCP接続を不可能にする

コード変更は最小限とし、既存技術の設定・ドキュメントを中心に進める。

## Technical Context

**Language/Version**: Rust 1.83+ (stable2503), Bash (セットアップスクリプト)  
**Primary Dependencies**: sc-network (Substrate libp2p実装), Tor 0.4.x (外部デーモン)  
**Storage**: N/A（ネットワーク層のみ）  
**Testing**: 手動テスト（複数ノード間通信）、統合テストスクリプト  
**Target Platform**: Linux (systemdベース)、macOS対応  
**Project Type**: Single (blockchain node)  
**Performance Goals**: Tor経由で10分以内にブロック同期開始、100 tx/hour中継  
**Constraints**: Torネットワークレイテンシ許容（数百ms〜数秒）、回線帯域制限対応  
**Scale/Scope**: 初期テストネット3-10ノード

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| 原則 | ステータス | 評価 |
|------|-----------|------|
| **I. Network Anonymity** | ✅ 直接支持 | 本機能の核心。Tor統合によりIPアドレス秘匿を実現 |
| **II. Keyless UX** | ⚪ 影響なし | ネットワーク層の変更でUX層に影響なし |
| **III. Client-Side Completion** | ⚪ 影響なし | 暗号化・断片化処理には影響しない |
| **IV. Zero-Trust Hydra** | ⚪ 影響なし | フロントエンドとノードは独立 |
| **V. Economic Autonomy** | ⚪ 影響なし | 報酬設計に変更なし |
| **VI. Test-First Development** | ✅ 準拠 | 統合テストスクリプトで検証 |

**Gate Result**: ✅ PASS（違反なし）

## Project Structure

### Documentation (this feature)

```text
specs/006-libp2p-tor/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output (minimal - network config entities)
├── quickstart.md        # Phase 1 output (Tor setup guide)
├── contracts/           # Phase 1 output
│   └── network-config.md  # CLI options and config file format
└── tasks.md             # Phase 2 output (/speckit.tasks command - NOT created by /speckit.plan)
```

### Source Code (repository root)

```text
apps/blockchain/
├── node/
│   └── src/
│       ├── cli.rs          # --tor-mode CLI option追加
│       └── command.rs      # Torモード処理
├── scripts/
│   ├── run-multi-node.sh   # 既存（Torモードオプション追加）
│   ├── tor-setup.sh        # NEW: Tor/torsocksセットアップ
│   └── onion-service.sh    # NEW: Onion Service設定生成
└── docs/
    └── tor-deployment.md   # NEW: Tor運用ガイド
```

**Structure Decision**: 既存のblockchainノード構造を維持し、スクリプト・ドキュメントを追加。コア変更は`cli.rs`のオプション追加のみ。

## Complexity Tracking

> 違反なし - このセクションは空欄

## Constitution Re-Check (Post-Design)

*Phase 1設計完了後の再評価*

| 原則 | ステータス | 設計への影響 |
|------|-----------|-------------|
| **I. Network Anonymity** | ✅ 強化 | torsocks + Onion Serviceで双方向匿名化を実現。`--tor-mode=forced`で非匿名接続を完全拒否可能 |
| **II. Keyless UX** | ⚪ 影響なし | 設計変更なし |
| **III. Client-Side Completion** | ⚪ 影響なし | 設計変更なし |
| **IV. Zero-Trust Hydra** | ⚪ 影響なし | 設計変更なし |
| **V. Economic Autonomy** | ⚪ 影響なし | 設計変更なし |
| **VI. Test-First Development** | ✅ 準拠 | quickstart.mdに検証手順を記載、統合テストスクリプトを計画 |

**Post-Design Gate Result**: ✅ PASS

---

## Generated Artifacts

| ファイル | 説明 | ステータス |
|----------|------|-----------|
| [research.md](research.md) | 技術調査結果 | ✅ Complete |
| [data-model.md](data-model.md) | データモデル定義 | ✅ Complete |
| [contracts/network-config.md](contracts/network-config.md) | CLI/設定API | ✅ Complete |
| [quickstart.md](quickstart.md) | セットアップガイド | ✅ Complete |
| tasks.md | 実装タスク | 🔜 `/speckit.tasks`で生成 |
