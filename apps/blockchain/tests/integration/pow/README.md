# PoW Integration Tests (Phase B Staging)

Mainnet 投入前ゲートで運用者が手動実行する 5 シナリオ。 

## 前提

- `apps/blockchain` を release build 済み (`cargo build --release -p anarchy-node`)
- `--randomx-mode fast` (full 2GB dataset) で実行 → **最低 16GB RAM 推奨** (3 ノード × 2GB scratchpad + 動作余裕)
- `jq` 必須

## 実行

```bash
cd apps/blockchain
./tests/integration/pow/multi_miner.sh         # 3 ノード reorg / finality 一致
./tests/integration/pow/hashrate_jump.sh       # DAA hashrate 急増耐性
./tests/integration/pow/authority_rotation.sh  # GRANDPA top-K rotation
./tests/integration/pow/selfish_mining.sh      # selfish mining vs finality
./tests/integration/pow/coinbase_inject.sh     # 不正 PreRuntime digest reject
```

各スクリプト終了コード 0 で PASS、それ以外で FAIL。失敗時は各ノードの `/tmp/anarchy-pow-*/node.log` を保存して issue 報告。

## ハードウェア要件

| シナリオ | RAM 必要 | CPU | 所要時間 |
|---|---|---|---|
| multi_miner | ~7GB (3 × 2GB + 余裕) | 4-core+ | 30 分 |
| hashrate_jump | ~7GB | 4-core+ | 15 分 |
| authority_rotation | ~7GB | 4-core+ | 90 分 (rotation period 600 blocks @30s) |
| selfish_mining | ~5GB (2 ノード) | 4-core+ | 20 分 |
| coinbase_inject | ~3GB (1 ノード) | 2-core+ | 5 分 |

## CI スコープ外の理由

GitHub Actions 標準 runner は 7GB RAM。3 ノード × 2GB は厳しい。CI では `--randomx-mode light` で 1 ノード 3 分の smoke のみ実行する ([`.github/workflows/pow-smoke.yml`](../../../../.github/workflows/pow-smoke.yml))。本格 integration は staging で運用者が release 前ゲートとして実施。

## ヘルパスクリプト

各スクリプトは [`utils.sh`](../utils.sh) (既存) のログ関数を使用。
