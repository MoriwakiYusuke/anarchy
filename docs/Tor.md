libp2p + Tor統合の実現可能性を評価するため、現在のプロジェクト構造とSubstrateのネットワーク層を確認します。   

正規表現 `libp2p|sc-network|NetworkConfiguration` (`**/*.rs`) を検索し、結果は1 件でした

[](file:///home/moriwaki-y/self/anarchy/apps/blockchain/Cargo.toml) の読み取り

[](file:///home/moriwaki-y/self/anarchy/apps/blockchain/node/Cargo.toml) の読み取り

[](file:///home/moriwaki-y/self/anarchy/apps/blockchain/node/src/service.rs#1-1) の 1 から 200 行を読み取り

arti-clientとlibp2p-torの現状を確認します。 

Research libp2p Tor integration

正規表現 `tor|libp2p|transport|network` (`**/*.md`) を検索し、結果は 20 件でした

正規表現 `sc-network|NetworkConfiguration|Transport` (`**/*.rs`) を検索し、結果は1 件でした

[](file:///home/moriwaki-y/self/anarchy/docs/TODO.md#1-1) の 1 から 80 行を読み取り

[](file:///home/moriwaki-y/self/anarchy/apps/blockchain/node/src/service.rs#150-150) の 150 から 200 行を読み取り

[](file:///home/moriwaki-y/self/anarchy/apps/blockchain/node/Cargo.toml) の読み取り

[](file:///home/moriwaki-y/self/anarchy/apps/blockchain/Cargo.toml#1-1) の 1 から 80 行を読み取り

## 評価結果

**結論: できるが、段階的アプローチを推奨**

### 実現可能性サマリー

| 項目 | 可否 | 難易度 |
|------|------|--------|
| libp2pネットワーク層 | ✅ | 低（Substrateが既に使用中） |
| Kademlia + GossipSub | ✅ | 低（sc-network標準機能） |
| arti-client統合 | ⚠️ | **高**（sc-networkフォーク必要） |
| Tor強制モード切替 | ⚠️ | 中〜高 |

### 主要な課題

1. **sc-networkのハードコード**: Substrateのネットワーク層はTCP+Noise+Yamuxが固定されており、カスタムトランスポート注入は**フォーク必須**

2. **arti-clientの不安定性**: 現在0.25.x（1.0未達）でAPIの破壊的変更が頻繁。セキュリティ監査も1.0以降

3. **libp2p-tor非存在**: Rust版libp2pには公式Torトランスポートが**存在しない**（Go版にはある）

### 推奨段階的アプローチ

| Phase | 内容 | 工数 | メリット |
|-------|------|------|----------|
| **1（即時）** | `torsocks ./anarchy-node`で外部プロキシ | 1日 | コード変更不要、即検証可能 |
| **2（短期）** | リバースプロキシ + Onion Service | 3-5日 | 受信もTor化、コード変更なし |
| **3（中長期）** | sc-networkフォーク + arti統合 | 2-4週 | 完全なアプリ内Tor |

### 今やるべきか？

- **Phase 1-2**: すぐ着手可能。プロトタイプとして十分
- **Phase 3**: arti 1.0安定版（2026年予定）後に再評価推奨

---

**「spec作成して計画立てる？」** または **「まずPhase 1のtorsocks検証をやってみる？」** どちらが良いですか？