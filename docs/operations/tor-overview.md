# Tor 統合の実現性評価

> **背景**: libp2p + Tor を Anarchy ノード間通信に統合する選択肢を初期評価したメモ。
> Phase 1 (torsocks + Onion Service) は実装済み。Phase 3 (sc-network フォーク + arti) は将来検討。

## 評価結果

**結論: できるが、段階的アプローチを推奨**

### 実現可能性サマリー

| 項目 | 可否 | 難易度 |
|------|------|--------|
| libp2p ネットワーク層 | ✅ | 低 (Substrate が既に使用中) |
| Kademlia + GossipSub | ✅ | 低 (sc-network 標準機能) |
| arti-client 統合 | ⚠️ | **高** (sc-network フォーク必要) |
| Tor 強制モード切替 | ⚠️ | 中〜高 |

### 主要な課題

1. **sc-network のハードコード**: Substrate のネットワーク層は TCP+Noise+Yamux が固定されており、カスタムトランスポート注入は **フォーク必須**
2. **arti-client の不安定性**: 現在 0.25.x (1.0 未達) で API の破壊的変更が頻繁。セキュリティ監査も 1.0 以降
3. **libp2p-tor 非存在**: Rust 版 libp2p には公式 Tor トランスポートが **存在しない** (Go 版にはある)

### 推奨段階的アプローチ

| Phase | 内容 | 工数 | メリット | 状態 |
|-------|------|------|----------|------|
| **1 (即時)** | `torsocks ./anarchy-node` で外部プロキシ | 1 日 | コード変更不要、即検証可能 | ✅ 実装済み |
| **2 (短期)** | リバースプロキシ + Onion Service | 3-5 日 | 受信も Tor 化、コード変更なし | ✅ 実装済み |
| **3 (中長期)** | sc-network フォーク + arti 統合 | 2-4 週 | 完全なアプリ内 Tor | ⏳ 保留 (arti 1.0 待ち) |

### 今後の方針

- **Phase 1-2** は本番運用で十分匿名性を担保 (mainnet で `--tor-mode=forced` 強制)
- **Phase 3** は arti 1.0 安定版 (2026 年予定) リリース後に再評価

## 関連ドキュメント

- 接続パターン: [tor-connection-patterns.md](tor-connection-patterns.md)
- デプロイ手順: [../../apps/blockchain/docs/tor-deployment.md](../../apps/blockchain/docs/tor-deployment.md)
- 脅威モデル: [../security/pow-threat-model.md](../security/pow-threat-model.md)
