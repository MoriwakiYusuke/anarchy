# Anarchy 開発状況レポート

> **最終更新**: 2026-05-12
> **対象読者**: ソフトウェア開発経験者でブロックチェーン初学者

## 🎯 プロジェクト概要

**Anarchy** は L1 ブロックチェーンを基盤とした匿名分散型 SNS プロトコルです。

従来の SNS との違い:

- **中央サーバーなし**: データはチェーンノード + 分散ストレージノードに保存
- **検閲耐性**: 単一の管理者がアカウントやコンテンツを削除できない
- **匿名性**: ユーザーの実世界の身元と投稿が紐付かない (libp2p + Tor 強制)

詳細なビジョン: [../vision/overview.md](../vision/overview.md) ・ アーキテクチャ: [../architecture/overview.md](../architecture/overview.md)

---

## 🧱 ブロックチェーン基礎

Anarchy は [Polkadot SDK](https://github.com/paritytech/polkadot-sdk) (`stable2503`) の **Substrate** で構築されています。

- **言語**: Rust (ノード・ランタイム) / TypeScript (フロント) / Rust → Wasm (暗号エンジン)
- **モジュラー設計**: 機能を「pallet」という単位で追加
- **コンセンサス**: 動的 PoW (RandomX) + GRANDPA finality

### 現在の pallet 一覧 (14 個)

| Pallet | 役割 |
|---|---|
| `base_fee` | EIP-1559 ベースの動的基準料金 |
| `block_reward` | 3-way fan-out + tail emission |
| `difficulty` | PoW 難易度自動調整 |
| `economic_params` | ガバナンス可変パラメータ (council 1/2) |
| `faucet` | PoW Faucet (1 回 / アカウント) |
| `grandpa_authority_election` | 動的 GRANDPA 権限選出 |
| `messaging` | DM (ChaCha20-Poly1305 + 固定長 padding + stealth) |
| `nickname` | 表示名管理 |
| `popularity` | 投稿人気度スコア + 削除候補化 |
| `post` | 投稿作成 (Merkle root 記録) |
| `reaction` | Like / Bad 反応マイニング (foreground PoW) |
| `stealth` | ステルスアドレス (EIP-5564 互換) |
| `storage` | 分散ストレージ協調 (KZG-VSS 報酬, slashing, self-repair) |
| `storage_stake` | ストレージノードの skin-in-the-game bond |

---

## ✅ 実装済みの主要機能

| 領域 | 機能 | 状態 |
|---|---|---|
| **Consensus** | 動的 PoW (RandomX) + GRANDPA finality | ✅ |
| **Token** | `pallet-balances` ネイティブ ($MORAL, 12 decimals) | ✅ |
| **Auth** | シードフレーズ → AccountId (sr25519), session-only key | ✅ |
| **Network** | libp2p + Tor (torsocks / Onion Service / mainnet 強制) | ✅ |
| **Storage** | KV (redb) ベース fragment store, KZG-VSS proof, slashing, self-repair | ✅ |
| **Post** | 動的バイト課金 (`PostBaseCost + content_bytes × PostByteCost`) | ✅ |
| **Reactions** | Like / Bad + foreground PoW マイニング | ✅ |
| **DM** | ステルスアドレス + 固定長 padding + ChaCha20-Poly1305 | ✅ |
| **Frontend** | Next.js 14 + PAPI WebSocket + Wasm Worker pool | ✅ |
| **Economic model (TSTS v1)** | 3 sink + 3 source 経済モデル全実装 | ✅ |
| **E2E** | Playwright 14 spec all green (WSL2) | ✅ |
| **Observability** | Prometheus exporter + Grafana ダッシュボード | ✅ |

> **過去の設計変更**:
> - WebAuthn 認証 → 廃止 (シードフレーズベース AccountId に統一) — Phase 1 段階
> - smoldot light client → 廃止 (PoW + chain-node RPC 拡張依存のため通常 WebSocket に戻す) — Phase B 段階

---

## 🚀 起動方法

```bash
pnpm stack:start   # testnet + storage + frontend を依存順で起動
```

詳細: [getting-started.md](getting-started.md)

---

## 🛠️ 技術スタック

### バックエンド

| 領域 | 技術 |
|---|---|
| 言語 | Rust (stable) |
| フレームワーク | Polkadot SDK `stable2503` |
| ストレージ (チェーン) | RocksDB |
| ストレージ (フラグメント) | redb (off-chain storage node) |
| P2P | libp2p + Tor (torsocks) |
| 暗号 | `ark-bls12-381` (KZG), `rs_merkle`, Blake2b |
| HTTP | axum (storage node) |

### フロントエンド

| 領域 | 技術 |
|---|---|
| フレームワーク | Next.js 14 (App Router) |
| UI | React 18, lucide-react |
| State | Zustand |
| Chain access | polkadot-api (PAPI) 1.x — `getWsProvider` |
| Crypto | anarchy-wasm-engine (Rust → wasm-bindgen) |
| i18n | en / ja / zh |
| Tests | Jest + Playwright |

**なぜ PAPI？**: Polkadot SDK `stable2503` はメタデータ v16 を使用するため、legacy `@polkadot/api` (v15 まで) では動かない。

---

## 📋 開発ロードマップ

### ✅ 完了済み

- Phase 1: セキュア・ファンデーション (Substrate 基盤, pallet 群, frontend MVP)
- Phase 2: プライバシー・レイヤー (SSS / KZG-VSS, ステルスアドレス, 分散ストレージ)
- Phase 3: 自律エコシステム (反応マイニング, DM E2EE, PoW Faucet)
- Phase 4: Slashing & Self-Repair (`pallet-storage` 自己修復プロトコル)
- Phase A/B: PoW migration (smoldot 撤退 + 動的 PoW + dynamic GRANDPA)
- TSTS 経済モデル v1 (3-sink / 3-source 全実装)
- Storage Node Phase 2 (redb backend + LRU + verify_on_read + crash test)

### ⏳ 進行中 / 次の課題

- TSTS 経済モデル v2 (v1 レビューで指摘された 4 件の構造的脆弱性対応)
- mainnet 投入準備 (chainspec, bootnodes, monitoring 強化)
- ハイドラ戦略の文書化 (複数フロントエンド運用ガイド)

最新の TODO: [todo.md](todo.md) ・ 直近の設計ドキュメント: [../superpowers/specs/](../superpowers/specs/)

---

## 📚 参考リンク

- [Substrate Documentation](https://docs.substrate.io/)
- [Polkadot SDK](https://github.com/paritytech/polkadot-sdk)
- [PAPI (polkadot-api)](https://papi.how/)
- [Next.js Documentation](https://nextjs.org/docs)
- 内部設計: [../architecture/](../architecture/)
- 経済モデル: [../economic/](../economic/)
- セキュリティ: [../security/](../security/)

---

## 💬 用語集

| 用語 | 説明 |
|---|---|
| ノード | ブロックチェーンネットワークに参加するコンピュータ |
| バリデーター | ブロックを生成・検証するノード (Anarchy では PoW マイナー + GRANDPA 認可者) |
| フルノード | 全データを保持するがブロック生成しないノード |
| ストレージノード | チェーンノードとは別に動く、断片保存専用のデーモン |
| エクストリンシック | Substrate でのトランザクションの呼称 |
| ランタイム | ブロックチェーンのビジネスロジック (Wasm にコンパイルされチェーン状態に保存) |
| pallet | ランタイムを構成するモジュール |
| Genesis | ブロックチェーンの初期状態 |
| RPC | Remote Procedure Call (ノードとの通信 API) |
| WebSocket | PAPI / フロントが使う双方向通信プロトコル |
| KZG コミットメント | 多項式コミットメント方式 (BLS12-381 上) |
| SSS | Shamir's Secret Sharing (秘密分散) |
| ステルスアドレス | 受信者を匿名化する一回限りアドレス (EIP-5564 互換) |
| Foreground PoW | Page Visibility API でブラウザ前面のみマイニング |
