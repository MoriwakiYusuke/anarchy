# PoW Migration — Threat Model

> **Status**: Phase B 実装ベース (2026-05-NN)
> **Spec**: [`docs/superpowers/specs/2026-05-06-pow-migration-design.md`](../superpowers/specs/2026-05-06-pow-migration-design.md) §11
> **Implementation**: [`apps/blockchain/runtime/src/lib.rs`](../../apps/blockchain/runtime/src/lib.rs), [`apps/blockchain/node/src/pow/`](../../apps/blockchain/node/src/pow/), [`pallets/grandpa_authority_election/`](../../apps/blockchain/pallets/grandpa_authority_election/)

Anarchy の PoW 構成 (RandomX + Permissionless GRANDPA + LWMA-3) に対する脅威分析。

## 1. 51% 攻撃 (majority hashrate)

**攻撃**: 攻撃者が過半数 hashrate を確保し、競合チェーンを生成して reorg を引き起こす。

**コスト**: ターゲットの hashrate を超える RandomX 計算リソース。RandomX は ASIC 抵抗性が
高い (CPU 優位) ため、汎用クラウド CPU を大量にレンタルする方式が現実的。
試算: 100 vCPU を 24h 借りると数百ドル規模 (cloud spot price)。SNS インフラへの
攻撃インセンティブが低いことが主な抑止力。

**緩和**:
- LWMA-3 が hashrate 急増を 60 ブロック以内に検出 → difficulty 急騰で攻撃継続コストを
  急激に上げる
- GRANDPA finality が打ったブロックは fork-choice rule で覆らない (Substrate 標準)
- UX 側で post 確定の confirmation 深度を最低 12 ブロック (= 6 分) 推奨

**残存リスク**: finality 直前の 5 〜 10 ブロックは reorg 可能。これは Bitcoin / Monero と
同等の trade-off で、SNS 用途では許容。

## 2. Selfish mining

**攻撃**: 攻撃者が新ブロックを公開せずにプライベートチェーンを伸ばし、公開チェーンが
追いついた瞬間にまとめて公開して reorg を起こす (Eyal & Sirer 2014)。

**緩和**:
- GRANDPA finality によって深い reorg が無効化 → selfish chain が finalized ブロックを
  含む reorg を試みても、fork-choice で蹴られる
- 影響範囲は finality 直前の 5 〜 10 ブロックに限定 (1.の残存リスクと同じ)

**残存リスク**: 短期的なブロック報酬の不公平 (selfish miner の取り分増)。経済的影響は
限定的だが、コミュニティの信頼を損なう可能性あり。

## 3. Time warp 攻撃 (timestamp manipulation)

**攻撃**: ブロック timestamp を操作して LWMA-3 の見積もりを誤らせ、difficulty を不当に
下げてマイニングを容易にする (Bitcoin の歴史的な攻撃ベクター)。

**緩和**:
- `pallet_timestamp::MinimumPeriod = SLOT_DURATION / 2` で連続ブロック間に最小間隔強制
- LWMA-3 内で `solve_time` を `clamp(1, 6 * target)` (= 最大 180s) しているため、極端な
  timestamp でも難易度は最大 6x 緩く・最小 1ms 厳しくしか変わらない
- pallet_timestamp の inherent check (各ブロック timestamp ≥ parent timestamp) も補強

**残存リスク**: 軽微。攻撃可能な変動幅は ±50% 程度に限定される。

## 4. GRANDPA authority sybil (top-K 占拠)

**攻撃**: 1 攻撃者が複数の mining node + AccountId を運用し、`pallet_grandpa_authority_election`
の top-K (=10) を占拠して finality を独占する。

**緩和**:
- top-K に入るには直近 100 ブロックの過半数を採掘する必要がある = 51% 相当のコストが要求される
- NPoS と異なりステーキング不要なので「資金で買う」攻撃は無効。攻撃には実 hashrate 投入が必須

**残存リスク**: 51% 攻撃の派生として finality 拒否 (chain halt) は可能。ただし PoW chain で
ブロック生成が続く限り、authority set が次の rotation で正常 miner に置き換わる (5h 周期)。

## 5. RandomX seed rotation 時の DoS

**攻撃**: RandomX dataset 切替 (seed key 変更) のタイミングでマイナーが一時的に
hash 計算停止 → ブロック生成が空白期に入る。

**現状**: Phase B 実装は `seed_key = genesis_hash` 固定 (epoch rotation 未実装)。
seed が変わらないので本攻撃は該当しない。

**Phase C で epoch rotation 導入時の緩和案**:
- epoch 境界の数ブロック前から並行 prebuild (バックグラウンドで次 dataset 構築開始)
- prebuild 中も旧 dataset で mining 継続

## 6. Long-range attack

**攻撃**: 攻撃者が創世 (genesis) から並行する別チェーンを構築し、現在の main chain と
入れ替えようとする。

**緩和**:
- GRANDPA finality が打ったブロックは fork-choice rule で創世まで遡って優先される
  (Substrate 標準 `LongestChain` + finality grandfather rule)
- 新規ノード起動時の sync では `--checkpoint` で warp sync を使うことが推奨

**残存リスク**: 完全に新規のノード (warp sync 無し、genesis から full sync) には理論上
攻撃可能だが、Anarchy では smoldot light client + chain spec hard-coded checkpoint で
回避できる。

## 7. Equivocation (GRANDPA 二重投票)

**攻撃**: GRANDPA authority が同じ height で異なる vote を発行する (悪意 or バグ)。

**緩和**:
- `pallet_grandpa::report_equivocation` で報告可能 (Substrate 標準)
- Anarchy では equivocation を起こした authority を `pallet_grandpa_authority_election` から
  自動 unregister する hook を Phase C で実装予定 (現状は手動)

**残存リスク**: equivocation 報告 → unregister の自動化が無いため、悪意 authority が
即座に排除されない可能性。実害は finality stall (ブロック生成は継続)。

## 8. Coinbase 注入 (PreRuntime digest tampering)

**攻撃**: PreRuntime digest に偽の AccountId を注入してブロック報酬を奪う。

**緩和**:
- `PowAuthor::find_author` (PreRuntime decoder) は SCALE デコード失敗時に `None` を返す
  → 不正バイトは無視 → block_reward が `BlockRewardSkipped { NoAuthor }` イベントを
  発行して mint しない
- Engine ID は `sp_consensus_pow::POW_ENGINE_ID = b"pow_"` を強制 (chain ローカルな
  別 ID は受け付けない)
- pallet_block_reward の unit test (pallets/block_reward/src/tests.rs:`current_reward_*`)
  + integration test (`coinbase_inject.sh`) でカバー

**残存リスク**: 軽微。PreRuntime digest は公開なので情報漏洩リスクのみ (実害なし)。

## まとめ

| 脅威 | 影響 | 攻撃コスト | 緩和状況 |
|---|---|---|---|
| 51% | reorg ≤ 10 blocks | 数百ドル / 24h | LWMA-3 + GRANDPA finality |
| Selfish | 報酬偏重 | 100% hashrate | finality で深い reorg 阻止 |
| Time warp | difficulty ±50% | 軽 | clamp + pallet_timestamp |
| GRANDPA sybil | finality halt | 51% 相当 | top-K に hashrate 必須 |
| RandomX seed DoS | 一時 stall | 軽 (Phase B 該当なし) | Phase C で prebuild |
| Long-range | warp sync 攻撃 | 高 | finality grandfather + checkpoint |
| Equivocation | finality stall | 極軽 (バグ系) | 手動 unregister (Phase C で自動化) |
| Coinbase 注入 | 報酬奪取試行 | ゼロ | PowAuthor + Engine ID 強制 |

Phase B 出荷時点では Anarchy 原則 (匿名・分散・誰でも参加) を維持しつつ、
mainnet 投入に十分な耐攻撃性を持つと判断。
