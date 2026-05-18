# Anarchy Documentation

Anarchy は L1 ブロックチェーンを基盤とした匿名分散型 SNS プロトコルです。本ディレクトリは設計・運用・開発に関するドキュメントの索引です。

> プロジェクト概要は [リポジトリルートの README](../README.md) を参照してください。

---

## 🌐 ビジョン (なぜ作るか)

| ドキュメント | 内容 |
|---|---|
| [vision/overview.md](vision/overview.md) | プロジェクト「Anarchy」とは何か (非技術者向け要旨) |
| [vision/concepts.md](vision/concepts.md) | 構想・検討中の機能 (経済設計・将来トピック) |
| [vision/matter.md](vision/matter.md) | 核心的な技術課題と解決策の全体像 |
| [vision/critique.md](vision/critique.md) | 新規性に対する自己批判的評価 |

## 🏛 アーキテクチャ (どう動くか)

| ドキュメント | 内容 |
|---|---|
| [architecture/overview.md](architecture/overview.md) | 5 層プロトコルスタック (ネットワーク〜UI) |
| [architecture/blockchain.md](architecture/blockchain.md) | Substrate L1 ノード技術仕様 |
| [architecture/storage.md](architecture/storage.md) | 分散ストレージ + 報酬モデル |
| [architecture/storage-strategy.md](architecture/storage-strategy.md) | 「地図と宝の分離」設計 |
| [architecture/posr.md](architecture/posr.md) | Proof of Storage Retrieval (KZG + SSS) |
| [architecture/self-recovery.md](architecture/self-recovery.md) | スラッシング・自己修復プロトコル |
| [architecture/frontend.md](architecture/frontend.md) | Next.js + PAPI + Wasm エンジン |

## 💰 経済モデル

| ドキュメント | 内容 |
|---|---|
| [economic/parameters.md](economic/parameters.md) | 全パラメータ棚卸し (実装値ベース) |
| [economic/proposal.md](economic/proposal.md) | TSTS 経済モデル設計提案 |
| [economic/implementation-plan.md](economic/implementation-plan.md) | M0 → M1 移行実装計画 |
| [economic/review-v1.md](economic/review-v1.md) | v1 構造的脆弱性レビュー |
| [economic/simulator.py](economic/simulator.py) | パラメータシミュレータ |

## ⚙️ 運用

| ドキュメント | 内容 |
|---|---|
| [operations/tor-overview.md](operations/tor-overview.md) | libp2p + Tor 統合の実現性評価 |
| [operations/tor-connection-patterns.md](operations/tor-connection-patterns.md) | 「内部直結 / 外部 Tor」接続パターン |
| [operations/pow-mainnet-runbook.md](operations/pow-mainnet-runbook.md) | mainnet ローンチ手順書 |
| [operations/pow-mining-setup.md](operations/pow-mining-setup.md) | PoW マイナー構築ガイド |
| [operations/dm-storage-growth.md](operations/dm-storage-growth.md) | DM ストレージ成長運用ガイド |

Tor 強制モードの起動方法は [apps/blockchain/docs/tor-deployment.md](../apps/blockchain/docs/tor-deployment.md) を参照。

## 🔒 セキュリティ

| ドキュメント | 内容 |
|---|---|
| [security/dm-key-exposure.md](security/dm-key-exposure.md) | DM 鍵露出のスレッドモデル |
| [security/pow-threat-model.md](security/pow-threat-model.md) | PoW 移行に関する脅威分析 |

## 🛠 開発

| ドキュメント | 内容 |
|---|---|
| [development/getting-started.md](development/getting-started.md) | 環境構築・起動手順 (フル) |
| [development/commands.md](development/commands.md) | ビルド / テスト / 開発コマンド一覧 |
| [development/status.md](development/status.md) | 開発状況レポート (非専門家向け) |
| [development/todo.md](development/todo.md) | 実装 TODO チェックリスト |

## 🦸 Superpowers (進行中の plan / spec)

| 場所 | 内容 |
|---|---|
| [superpowers/specs/](superpowers/specs/) | 進行中の機能設計 (Superpowers skill 経由) |
| [superpowers/plans/](superpowers/plans/) | 実装計画 |

> Superpowers plugin の規約上、このディレクトリは `docs/superpowers/` 固定です。

## 🗂 アーカイブ (履歴・過去設計)

| 場所 | 内容 |
|---|---|
| [archive/specs/](archive/specs/) | 001-019 機能仕様 (Spec-Kit 時代の設計資料) |
| [archive/code-review-2026-02.md](archive/code-review-2026-02.md) | 2026-02 時点の包括コードレビュー |
| [archive/frontend-extend-phase2.6.md](archive/frontend-extend-phase2.6.md) | Phase 2.6 フロントエンド拡充実装メモ |

> アーカイブは過去の設計判断の文脈として保持されています。現状の実装と一致しない場合があります。
