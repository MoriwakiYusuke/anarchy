# 経済モデル v1 レビュー(open issues / v2 設計のための入力)

> **作成日**: 2026-05-10
> **対象**: PR #54 + #55 で実装した TSTS v1
> **目的**: 経済学・社会学的観点からの構造的脆弱性 4 件を記録し、v2 設計の指針とする

---

## 結論サマリ

| 観点 | 成立? | 理由 |
|---|---|---|
| トークン経済(インフレ/デフレ) | ❌ | 5 年で mint の **60 倍** burn という極端なデフレ |
| 51% 攻撃耐性 | ❌ | tail emission 額が **絶対的に小さすぎる** |
| ユーザー獲得(ブートストラップ) | ❌ | Faucet cap が DAU 目標の **1/100** |
| Sybil 耐性(reactor) | △ | reactor lock min が low すぎ |
| Storage 経済性 | △ | stagnation シナリオで ROI 不成立 |
| Spam 耐性(post) | ✅ | EIP-1559 が機能 |
| 永続セキュリティ(構造) | ✅ | tail emission の**概念**は正しい |

---

## 1. 致命的: 極端なデフレの自己破壊サイクル

### 観測

シミュレーション S1 (100K DAU 成長) の結果:

```
Total minted:      2,904,054,427 MORAL
Total burned:    169,466,943,391 MORAL  ← 58 倍
Net Δ:          -166,562,888,963 MORAL
```

5 年で流通量が **1,660 億 MORAL 純減**。Genesis 配布が 10K MORAL × 数アカウント + プール seed 200K しかない中で、これだけ burn する MORAL がどこから来ているのかが論理的に破綻している(シミュレータの初期分配仮定がリアルでないか、実運用ではこのレベルの spam に到達不能)。

### 経済学的帰結(現実に起きた場合)

- **Gresham の法則の逆転**: ユーザーは MORAL 値上がり期待で hoarding → posting 用の流動性枯渇
- **後発ユーザーの参加コスト爆発**: 4 KB post = 462 MORAL (S1 想定)。1 MORAL = $1 なら 1 投稿 $462
- **Faucet 100 MORAL の実質価値喪失**: 1 投稿分にすら満たない

### 根本原因

post / DM の `base_fee × bytes` 全額 burn + 配分残差 30 % も burn で、**burn が二重化している**。これは仮想通貨の古典的失敗パターン(Luna terra 逆方向)。

### 推奨修正

- post 配分の burn 部分を **30 % → 10 %** に
- storage_share 50 → **60 %**, reaction_share 20 → **30 %** に振り直す
- または、burn 対象を base_fee の **overshoot 部分のみ**に限定

---

## 2. 致命的: 51% 攻撃コストが絶対的に低い

### 観測

```
TailEmission   = 0.5 MORAL/block
MinerShare     = 50%
→ 0.25 MORAL × 2880 block/day = 720 MORAL/day miner revenue (era 64+)
```

### PoW チェーンの経験則

`hashrate rental cost ≥ 1 day miner revenue` → **攻撃可能**

| MORAL 価格 | miner 1 日収入 | 攻撃成立性 |
|---:|---:|---|
| $1 | $720/日 | nicehash で容易にレンタル可能 → 常時攻撃可能 |
| $100 | $72,000/日 | 中規模攻撃者の射程内 |

比較: **Bitcoin tail emission は ~$50M/日** 規模

### 不変条件 I-1 の限界

「BlockReward > 0」と書いてあるが、**経済的セキュリティは絶対額の問題**であって、正値であるだけでは無意味。

### 推奨修正

選択肢(いずれか):
- TailEmission を **0.5 → 5 MORAL** 程度に上げる
- MinerSharePermill を era 64+ で **50 % → 80 %** に動的シフト
- post コストの一部を miner fee に直接還流(EIP-1559 priority tip 経路)

---

## 3. ブートストラップ詰み: Faucet 1000 枠 vs. 目標 100 K DAU

### 観測

```
RewardAmount = 100 MORAL
TotalCap     = 100,000 MORAL
→ 累計 1000 claim で枯渇
```

シミュレーション S1 は「1K → 100K DAU」を想定しているが、Faucet で MORAL を得られるのは **累計 1000 ユーザーのみ**。残り 99,000 人は:

| 経路 | 現実性 |
|---|---|
| 取引所で買う | 取引所流動性は採用後にしか生まれない(chicken-and-egg) |
| PoW でブロック報酬を採掘 | 一般ユーザーには非現実的 |
| 既存ユーザーから贈与 | これが唯一の現実解だが、**贈与インセンティブが設計されていない** |

### 社会学的比較

| プラットフォーム | 新規ユーザー ramp |
|---|---|
| Steemit | 登録時 1〜2 STEEM 自動付与 → 100 万 DAU 達成 (pre-2018) |
| Farcaster | ガス代を運営が肩代わり → 50 万 DAU |
| Anarchy v1 | Faucet 100 MORAL × 1000 claim **しかない** |

### 推奨修正

- Faucet TotalCap を **100K → 数百万 MORAL** に
- 既存ユーザーの post / reaction に「招待 quota」を生成する仕組み(referral economy)
- bootstrap 期間のみ post コストを 1/10 に減免

---

## 4. 中度: Reactor Sybil 耐性が機能していない

### 観測

シミュレータ S4 (1M Sybil reactor, 5y):

```
M1 Sybil takes 82.6% of reaction payouts = 481,083,012 MORAL over 5y
```

```
Reactor lock min = 0.1 MORAL → 1M Sybil の lock 総額 = 100K MORAL
Sybil reward = 481M MORAL → ROI 4810x
```

### 設計の非対称性

`√(bond) quadratic resistance` は **storage 側にだけ実装**されていて、reactor 側は単純な lock のみ。0.1 MORAL は実質的にゼロ。

### 推奨修正

- ReactorLockMin を **0.1 → 10〜100 MORAL** へ
- reactor reward にも **`√(reactor_lock / total_reactor_lock)`** を導入

---

## 5. 社会学的観点: 「有料 SNS」の構造的不利

| プラットフォーム | post コスト | 1B 達成 |
|---|---|---|
| Twitter/X | $0 | ✅ |
| Facebook | $0 | ✅ |
| Reddit | $0 | ✅ |
| Farcaster | $0(運営肩代わり)| ❌ (~50 万 DAU) |
| Lens | gas fee のみ (~$0.01) | ❌ |
| Steemit | $0(報酬のみ)| ❌ (peak 100 万 DAU) |
| **Anarchy** | **25 MORAL + 0.0008/byte** | ? |

### 理論

社会学の network effects 理論(Metcalfe's law / Reed's law)によると、ネットワーク価値は参加者数の **二乗以上**。摩擦は二乗以上の損失。

### Anarchy の正当化条件

ターゲットが「**匿名性プレミアムを払ってでも使う subset of users**」(dissidents, privacy advocates, censored journalists 等)であれば、有料モデルは正当化される。

この場合の現実的 DAU 上限:
- Tor users active ≈ 200 万
- ブラウジング目的を除くと Anarchy が刺さる subset は **1〜10 万**

### 推奨

docs に **目標 DAU の現実的レンジ(1K〜100K)** を明示し、シミュレータの S1 (100K DAU 成長) ではなく **S5 (5K DAU flat) を base case** に切り替える。

---

## 6. 良い点(壊さないでほしい)

- ✅ **EIP-1559 base_fee による spam 自己消費** は構造として正しい(S3 で機能確認済)
- ✅ **Storage の `√(bond)` quadratic Sybil resistance** は理論的に妥当
- ✅ **`pallet-economic-params` 経由の governance 可変** で事後調整が効く
- ✅ **Tail emission の概念導入**は方向性として正しい(絶対額が問題なだけ)
- ✅ **DM 受信報酬 (stealth pool)** は受信者インセンティブ設計として独創的

---

## 7. v2 への最小限の修正パッケージ

優先度順:

| Priority | 項目 | 旧 | 新 |
|---|---|---:|---:|
| **P0** | Faucet TotalCap | 100,000 MORAL | **数百万 MORAL** |
| **P0** | TailEmission | 0.5 MORAL | **5 MORAL** |
| **P1** | post / DM burn 比率 | 30 % | **10 %** |
| **P1** | post storage_share | 50 % | **60 %** |
| **P1** | post reaction_share | 20 % | **30 %** |
| **P1** | ReactorLockMin | 0.1 MORAL | **10 MORAL** |
| **P1** | Reactor reward に √(lock_share) 補正 | 無し | **追加** |
| **P2** | 目標 DAU レンジを docs に明示 | 無し | 1K〜100K(realistic case = 5K〜50K) |

### v2 成立条件

シミュレータを再実行して以下が満たされるべき:

1. **Net supply Δ が mint の −2 倍以内** に収まる(現状 −58 倍は破綻)
2. **Sybil reward / lock 比率が 10x 以下**(現状 4810x は破綻)
3. **Tail emission ≥ $50K-equivalent/day** で 51 % 攻撃を経済的に困難に

---

## 8. v1 の歴史的位置づけ

v1 は経済モデル**設計の第一稿**として、以下の貢献がある:
- TSTS (Triple-Sink Triple-Source) 構造の導入
- EIP-1559 base_fee による spam 自己消費の確立
- Quadratic Sybil resistance(storage 側)
- DM 受信者インセンティブ(stealth pool)
- Governance による事後調整可能性(pallet-economic-params)

v2 は v1 の **構造を保ちつつパラメータを調整**することで実装的には小さな差分で済む(`pallet-economic-params` の setter で動的に対応可能な項目もある)。

---

## 9. このレビューの扱い

- **mainnet ローンチ前**に v2 移行を完了させる
- 本 doc は v2 設計の **入力**として保存
- v2 実装は別 PR(`feature/economic-model-v2`)で進める
- v1 PR (#55) は **v1 として完結**させ、v2 への移行を別途トラック

---

## 10. 関連ドキュメント

- [`economic_model_proposal.md`](economic_model_proposal.md) — TSTS 設計提案 (v1 ベース)
- [`economic_model_implementation_plan.md`](economic_model_implementation_plan.md) — v1 実装計画
- [`economic_parameters.md`](economic_parameters.md) — v1 final 値の棚卸し
- [`economic/simulator.py`](economic/simulator.py) — シミュレータ
