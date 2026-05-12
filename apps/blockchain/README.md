# anarchy-blockchain

Anarchy の L1 ブロックチェーンノード (Substrate / Polkadot SDK stable2503)。

## 構成

```
apps/blockchain/
├── node/               # ノード実行ファイル (consensus, RPC, networking)
├── runtime/            # WASM ランタイム (FRAME 構成)
├── pallets/            # カスタム pallet
│   ├── base_fee/       #   動的基準料金
│   ├── block_reward/   #   ブロック報酬分配
│   ├── difficulty/     #   PoW 難易度調整
│   ├── economic_params/#   ガバナンス可変パラメータ
│   ├── faucet/         #   PoW Faucet
│   ├── grandpa_authority_election/  # 動的 GRANDPA 権限選出
│   ├── messaging/      #   DM (ChaCha20-Poly1305 + ステルス)
│   ├── nickname/       #   表示名管理
│   ├── popularity/     #   投稿人気度スコア
│   ├── post/           #   投稿作成 (Merkle root 記録)
│   ├── reaction/       #   Like / Bad (反応マイニング)
│   ├── stealth/        #   ステルスアドレス (EIP-5564 互換)
│   ├── storage/        #   分散ストレージ協調
│   └── storage_stake/  #   ストレージノード ステーク
├── primitives/         # 共有型・トレイト
├── scripts/            # multi-node 起動 / Tor 関連スクリプト
├── tests/integration/  # shell ベース E2E テスト
└── docs/               # Tor デプロイガイド
```

## ビルド & 起動

```bash
cargo build --release

# 単一 dev ノード
cargo run -- --dev --mine --coinbase 5Grwv... --randomx-mode light

# 3 ノード testnet (Alice/Bob = Validator, Charlie = Full)
./scripts/run-multi-node.sh start
```

詳細起動手順: [../../docs/development/getting-started.md](../../docs/development/getting-started.md)

## 主要技術

- **Consensus**: 動的 PoW (RandomX) + GRANDPA finality
- **Networking**: libp2p (sc-network) + Tor (torsocks / Onion Service)
- **Token**: `pallet-balances` ネイティブ ($MORAL, 12 decimals)
- **Auth**: シードフレーズ → sr25519 AccountId

## Tor 強制モード

mainnet では `--tor-mode=forced` が自動有効化されます。詳細は [docs/tor-deployment.md](docs/tor-deployment.md)。

## 統合テスト

shell ベースの E2E:

```bash
cd tests/integration && ./run_all_tests.sh
```

個別: `test_block_sync.sh`, `test_consensus.sh`, `test_invalid_data.sh`, `test_node_recovery.sh`, `test_scalability.sh`

## 関連ドキュメント

- アーキテクチャ: [docs/architecture/blockchain.md](../../docs/architecture/blockchain.md)
- 経済モデル: [docs/economic/](../../docs/economic/)
- セキュリティ: [docs/security/](../../docs/security/)
