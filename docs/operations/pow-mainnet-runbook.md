# PoW Mainnet 投入ランブック

> **Status**: Phase B 出荷後の mainnet ローンチ手順
> **Policy**: CLAUDE.md "Compatibility Policy" に従い **chain reset 方式** (migration code は書かない)
> **Spec**: [`docs/superpowers/specs/2026-05-06-pow-migration-design.md`](../superpowers/specs/2026-05-06-pow-migration-design.md) §12

## 投入前ゲート (Release Checklist)

mainnet 起動前に以下を全て pass させること。

### コード品質
- [ ] Phase A PR (#52) と Phase B PR が main にマージ済み
- [ ] `cargo test --workspace` 通過
- [ ] `cargo clippy -p pallet-difficulty -p pallet-block-reward -p pallet-grandpa-authority-election -p anarchy-runtime -p anarchy-node -- -D warnings` clean
- [ ] CI [`pow-smoke.yml`](../../.github/workflows/pow-smoke.yml) green (1 ノード light mode 3min)

### Staging integration ([`tests/integration/pow/`](../../apps/blockchain/tests/integration/pow/))
最低 16GB RAM のマシンで 5 シナリオを全て pass:
- [ ] `multi_miner.sh` (3 ノード fast mode 30 分稼働、best ≥ 50, finality ±3 以内)
- [ ] `hashrate_jump.sh` (DAA が target 30s ± 50% 内に再収束)
- [ ] `authority_rotation.sh` (AuthoritySetRotated event がトリガ)
- [ ] `selfish_mining.sh` (reorg 発生するが finalized は守られる)
- [ ] `coinbase_inject.sh` (BlockRewardMinted event 発行)

### チューニング
- [ ] `scripts/bench-randomx.sh` でリファレンス HW (8-core CPU 推奨) の hashrate を実測
- [ ] 推奨 initial_difficulty 値を [`apps/blockchain/node/src/chain_spec.rs`](../../apps/blockchain/node/src/chain_spec.rs) `production_config()` に焼き込み
- [ ] `MinDifficulty` (runtime) を本番想定値 (例 10_000) に戻す ※Phase B では dev/smoke 用に 100 設定中

### セキュリティ・運用
- [ ] [`docs/security/pow-threat-model.md`](../security/pow-threat-model.md) レビュー完了
- [ ] [`docs/operations/pow-mining-setup.md`](pow-mining-setup.md) のオペレータ向けチェック
- [ ] genesis bootstrap miner の GRANDPA key を**オフラインマシンで生成** (subkey)、chain spec に焼き込み
- [ ] `--tor-mode forced` で起動できることを staging で確認

## 投入手順

### 0. 準備
1. Phase A + Phase B 両 PR が `main` にマージされていること
2. リリースタグを切る: `git tag v1.0.0-pow && git push origin v1.0.0-pow`

### 1. Production build
```bash
cd apps/blockchain
cargo build --release -p anarchy-node

# 出力 binary を確認
./target/release/anarchy-node --version
# anarchy-node 1.0.0-...
```

### 2. Production chain spec 生成
```bash
./target/release/anarchy-node build-spec --chain production --raw \
    > production-spec.json
```

`production-spec.json` を編集して以下を確認:
- `genesisConfig.grandpa.authorities`: bootstrap miner の GRANDPA key 1 〜 3 名
- `genesisConfig.difficulty.initialDifficulty`: bench 実測値
- `genesisConfig.balances.balances`: 初期残高 (例: faucet pool / treasury 用に少量配布)
- `genesisConfig.sudo.key`: **None** (mainnet では sudo 撤廃推奨) または signing 用 root key

### 3. Genesis 配布
- `production-spec.json` の SHA256 を公式 announce
- GitHub Release / IPFS / Tor hidden service 経由で配布
- 各マイナーは chain spec を取得して `--chain ./production-spec.json` で起動

### 4. Bootstrap miners 起動
最初の 100 ブロック (≒ 50 分) は genesis grandpa authority 1 名で finality を回す。
3 ノード以上の bootstrap miner を同時起動 (best practice):

```bash
./target/release/anarchy-node \
    --chain ./production-spec.json \
    --mine --coinbase 5G... \
    --randomx-mode fast \
    --base-path /var/lib/anarchy \
    --tor-mode forced \
    --validator \
    --bootnodes <他 bootstrap node の peer ID 経由>
```

### 5. Public mining 解放
- block #100 〜 #600 の間で `pallet_grandpa_authority_election::RecentAuthors` window が
  満たされ、最初の rotation がトリガする (block #600)
- AuthoritySetRotated event が発行されたら announce 公開して community mining を呼びかけ
- ハッシュパワーが分散したことを確認 (ノードログで author の多様性を grep)

### 6. ローンチ後の監視
最初の 24 時間は以下をリアルタイム観察:

| 項目 | 期待値 | 異常時対応 |
|---|---|---|
| Block time 平均 | 25-35s | LWMA-3 追従中なら正常。逸脱継続なら hashrate 異常を疑う |
| Finalized lag | ≤ 10 ブロック | GRANDPA stall の可能性。authority set を確認 |
| Authority rotation | 5h ごとに 1 回 | RecentAuthors 集計が正常か state_call で確認 |
| Mining ノード数 | ≥ 3 | bootstrap miners 以外が来ないなら hashrate アピール強化 |

### 7. インシデント対応

#### Block 生成停止 (10 分以上)
1. `tail -f` で全 bootstrap node のログを確認
2. RandomX seed 変更時の dataset 再構築待ちでないか確認
3. 全マイナーのハッシュパワー喪失なら chain halt → community に再起動アナウンス

#### Finality stall (1 時間以上 finalized 進まず)
1. authority set の 1/3 以上がオフラインの可能性 → state_getStorage で
   `pallet_grandpa::Authorities` を確認
2. authority rotation を待つ (最大 5h)、または sudo (残してれば) で強制 set 入れ替え
3. 完全な復旧不可 → chain reset (新 chain spec、過去データ放棄、CLAUDE.md ポリシー通り)

#### 51% 攻撃疑い
1. 短時間に大量の reorg が起きていないかログ確認 (`grep -c "Reorg" /var/log/anarchy.log`)
2. hashrate を Prometheus メトリクスで確認 (将来 Task)
3. コミュニティに正常チェーン情報を即座 broadcast、追加 hashrate 提供を呼びかけ
4. 限界事例では chain halt + reset

## migration code を書かない理由 (CLAUDE.md ポリシー)

- 開発初期段階のため、過去の chain state は破棄して問題なし
- `pallet_aura` → PoW の差分はストレージレイアウトを変えるため、migration code を書くより
  新 genesis の方が安全 (migration バグでチェーン破損リスク回避)
- フロントエンド (smoldot) も chain spec hash の変更を検知して再 sync する

## ロールバック計画

mainnet ローンチ後、致命的バグ (例: block_reward の overflow) が発生した場合:

1. 全マイナーに「停止 + 旧 binary に戻して再起動」アナウンス (Tor hidden service / Slack 等)
2. `git revert` で問題コミットを戻し、hotfix branch を切る
3. 新 chain spec で再ローンチ (chain reset、過去 state 放棄)
4. ローンチから 7 日以内なら post-mortem を `docs/incidents/` に記録

## 参考

- [pow-mining-setup.md](pow-mining-setup.md) — オペレータ向け詳細手順
- [pow-threat-model.md](../security/pow-threat-model.md) — セキュリティ前提
- [Phase B implementation plan](../superpowers/plans/2026-05-06-pow-migration-phase-b.md)
