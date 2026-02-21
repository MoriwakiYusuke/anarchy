# 統合テスト

ブロックチェーンノード間の通信とコンセンサスの正当性を検証するテストスイート。

## テスト項目

### Phase 1: ブロックチェーン基本テスト（自己起動）

| テスト | 説明 | 検証内容 |
|--------|------|----------|
| `test_block_sync.sh` | ブロック同期 | 新規ノードが既存チェーンに追いつけるか |
| `test_consensus.sh` | コンセンサス | バリデータ間でブロックが正しくファイナライズされるか |
| `test_invalid_data.sh` | 不正データ拒否 | 不正なトランザクションが拒否されるか |
| `test_node_recovery.sh` | ノード復旧 | 停止したノードが復帰後に同期できるか |
| `test_scalability.sh` | スケーラビリティ | 多数ノード（デフォルト10）での協調動作 |

### Phase 2: 静的検証テスト

| テスト | 説明 | 検証内容 |
|--------|------|----------|
| `test_fee_distribution.sh` | 手数料分配 | 投稿手数料の90%報酬プール/10%バーン分配ロジック |

### Phase 3: 外部ノードテスト（testnet必要）

| テスト | 説明 | 検証内容 |
|--------|------|----------|
| `test_multi_node.sh` | マルチノードストレージ | 3ノード以上での断片分散 |
| `test_p2p_gossip.sh` | P2Pエンドポイント伝播 | ストレージノード登録と伝播 |
| `test_failover.sh` | フェイルオーバー | ノード障害時の冗長性確認 |

### スタブテスト（要実装）

| テスト | 仕様 | 状態 |
|--------|------|------|
| `test_kzg_vss_e2e.sh` | T018: KZG-VSS投稿フロー | TODO |
| `test_kzg_proof_e2e.sh` | T033: KZG証明検証 | TODO |
| `test_forgetting_flow.sh` | T053/T054: スコアベース忘却 | TODO |
| `test_score_default.sh` | T062: デフォルトスコア報酬 | TODO |
| `test_proof_success_rate.sh` | SC-004: 証明成功率99%以上 | TODO |
| `test_gc_timing.sh` | SC-005: GCタイミング精度 | TODO |

## 実行方法

```bash
# 全テスト実行（基本 + 静的検証）
./run_all_tests.sh

# クイックモード（scalabilityスキップ）
./run_all_tests.sh --quick

# フルモード（外部ノードテスト含む、testnet起動必要）
pnpm testnet:start    # 別ターミナルで
./run_all_tests.sh --full

# オプション
#   --quick           スケーラビリティテストをスキップ
#   --full            外部ノードテストを含む（testnet起動必要）
#   --scalability N   スケーラビリティテストのノード数（デフォルト: 10）

# 個別テスト実行
./test_block_sync.sh
./test_consensus.sh
```

## 必要条件

- jq (JSONパーサー)
- ビルド済みノードバイナリ (`cargo build --release`)
- `--full` モード: testnet起動済み (`pnpm testnet:start`)
