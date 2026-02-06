# 統合テスト

ブロックチェーンノード間の通信とコンセンサスの正当性を検証するテストスイート。

## テスト項目

| テスト | 説明 | 検証内容 |
|--------|------|----------|
| `test_block_sync.sh` | ブロック同期 | 新規ノードが既存チェーンに追いつけるか |
| `test_consensus.sh` | コンセンサス | バリデータ間でブロックが正しくファイナライズされるか |
| `test_invalid_data.sh` | 不正データ拒否 | 不正なトランザクションが拒否されるか |
| `test_node_recovery.sh` | ノード復旧 | 停止したノードが復帰後に同期できるか |
| `test_scalability.sh` | スケーラビリティ | 多数ノードでの協調動作 |

## 実行方法

```bash
# 全テスト実行
./run_all_tests.sh

# 個別テスト実行
./test_block_sync.sh
./test_consensus.sh
```

## 必要条件

- Node.js 18+
- jq (JSONパーサー)
- ビルド済みノードバイナリ (`cargo build --release`)
