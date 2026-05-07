# Anarchy Grafana Dashboards

TSTS 経済モデルの監視用 Grafana ダッシュボード集。

## anarchy-economy.json

TSTS 経済モデル v1 の主要 KPI を可視化する。設計提案 [docs/economic_model_proposal.md](../../docs/economic_model_proposal.md) §5 の不変条件 (I-1〜I-5) を実運用で検証するためのもの。

### パネル

| # | パネル | データソース | 観測対象 |
|---|---|---|---|
| 1 | σ_storage | `anarchy_storage_reward_pool_balance` | I-2 ストレージプール下限 |
| 2 | σ_reaction | `anarchy_reaction_reward_pool` | I-3 反応プール正値性 |
| 3 | σ_stealth | `anarchy_stealth_reward_pool` | DM 還流 20% の累積 |
| 4 | TotalIssuance | `anarchy_total_issuance` | tail emission 効果 (I-1) |
| 5 | base_fee | `anarchy_base_fee` | I-5 spam 自己消費 (混雑シグナル) |
| 6 | bytes used vs target | `anarchy_gas_used_this_block` + `50000` | EIP-1559 utilization |
| 7 | miner revenue/sec | `rate(anarchy_block_reward_miner_total[5m])` | tail emission 後の永続収入 |
| 8 | TotalActiveBond | `anarchy_total_active_bond` | I-4 Sybil 経済コスト |
| 9 | slashing rate | `rate(anarchy_node_slashed_total[5m])` | 障害検知 |
| 10 | faucet minted vs cap | `anarchy_faucet_total_minted` + `100000000000000000` | TSTS P7 sunset 進行 |
| 11 | reactor locks count | `anarchy_reactor_locks_count` | Sybil 攻撃検知 |

### 推奨アラート (本ダッシュボードでは未定義、別途 Alertmanager で設定)

```yaml
- alert: AnarchyReactionPoolNearEmpty
  expr: anarchy_reaction_reward_pool < 1000000000000000   # < 1000 MORAL
  for: 5m
  labels: { severity: warning }
  annotations:
    summary: "Reaction pool σ_reaction below 1000 MORAL — I-3 violation imminent"

- alert: AnarchyBaseFeeAtCap
  expr: anarchy_base_fee >= 50000000000   # >= 0.05 MORAL/byte (50% of cap)
  for: 10m
  labels: { severity: critical }
  annotations:
    summary: "Base fee climbed near cap (sustained spam attack suspected)"

- alert: AnarchyTotalIssuanceFalling
  expr: deriv(anarchy_total_issuance[1h]) < 0
  for: 1h
  labels: { severity: warning }
  annotations:
    summary: "Total issuance shrinking for 1h — burn dominating tail emission"

- alert: AnarchyBondLockedZero
  expr: anarchy_total_active_bond == 0
  for: 30m
  labels: { severity: critical }
  annotations:
    summary: "No storage nodes bonded — F1 skin-in-the-game effectively off"
```

### Substrate Prometheus メトリクス取得

`anarchy_*` プレフィックスのメトリクスは現状 **未実装** (本 PR で追加した dashboard JSON のみ)。
`pallet-prometheus-exporter` 風の独自 exporter で `pallet_storage::RewardPoolBalance::<Runtime>::get()` 等を読み取って Prometheus に export する後続作業が必要。

短期的には:
1. Substrate ノードの組込 Prometheus (`--prometheus-port 9615`) で一般メトリクスを Grafana に取り込む
2. 補助 `scripts/anarchy-metrics-exporter.ts` を別途用意してチェーン RPC を polling し、独自メトリクス系列を追加で吐く

### インポート手順

1. Grafana > Dashboards > New > Import
2. `infra/grafana/anarchy-economy.json` をアップロード
3. Prometheus データソースを選択 (テンプレート変数 `DS_PROMETHEUS` で割り当てる)
4. Save

## ロードマップ

- [ ] `anarchy_*` メトリクスを emit する exporter (Rust / TypeScript いずれか)
- [ ] Storage node 単体の health dashboard (`anarchy-storage-node.json`)
- [ ] Frontend health (PAPI WebSocket latency, page load) dashboard
