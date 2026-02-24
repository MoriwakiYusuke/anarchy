# 自己修復プロトコル設計仕様

**Status**: Draft  
**Created**: 2026-02-24  
**Feature Branch**: `013-slashing-repair`

## 概要

ストレージノードがオフラインになった際に、断片を自動的に再配布し、k-of-n閾値（3-of-5）を維持するプロトコル。報酬プールからの積み立てをスラッシング原資とし、再分散協力者へのインセンティブを提供する。

### 既存GCとの関係

本プロトコルは**既存のスコアベースGC**（`gc.rs`、`ForgettingCandidates`）と**並列で動作**する。

| GCの種類 | 対象 | 目的 | 実装済み |
|----------|------|------|----------|
| **スコアベースGC** | 低スコアコンテンツ | 不人気コンテンツの削除 | ✅ 既存 |
| **Stale Holder GC** | 超過ホルダー | 復帰ノードによる重複解消 | 🆕 新規 |

両方とも**オフチェーン計算方式**（ストレージノードがRPCでオンチェーン情報取得→判断→実行）を採用し、一貫性を維持。

## 設計パラメータ

| 項目 | 値 | 根拠 |
|------|------|------|
| 検出閾値 | チャレンジ未応答3回 | 約300分（100分×3）で検出 |
| 再分散トリガー | ≤4個（k+1） | 予防的発動、k=3到達前に修復 |
| ホルダー上限 | n=5 | 超過時にGC発動 |
| GC対象 | 古いindex優先 | スラッシュ済みノードのシェアを優先削除 |
| 引き出し下限 | 500 MORAL | 引き出し制限（スラッシング原資確保） |
| 下限未満スラッシュ | 全額没収 | デポジット不要、残高不足でも即時ペナルティ |
| スラッシュ率 | 50%/違反 | 2回で実質退場（厳格） |
| 再分散報酬 | スラッシュ金額の100% | 複数ノードに分配 |

## 報酬・スラッシングフロー

```
投稿費用90% → RewardPool → 保持証明成功 → AccruedRewards（個人報酬プール）
                                           ↓
                              claim_rewards（最低額: 500 MORAL）
                                           ↓ 違反発生（未応答3回）
                              50%スラッシュ → 再分散者へ分配
```

### ポイント

- **積み立て方式**: 保持証明成功時に即時支払いではなく、`AccruedRewards` に積み立て
- **引き出し下限**: 500 MORAL 以上でないと引き出し不可（スラッシング原資確保）
- **スラッシュ原資**: 積み立て済みの `AccruedRewards` から没収
- **下限未満のスラッシュ**: AccruedRewards が 500 MORAL 未満でも全額没収を実行（デポジット不要）
- **2回で退場**: 50%×2回 = 実質全損（残高不足時は1回で即退場）

## 状態遷移

```
FragmentState:
  Active ──[holder ≤ 4]──▶ AtRisk ──[repair started]──▶ Repairing ──[confirm]──▶ Active
                              │                            │
                              └──────[holder ≤ 2]──────────▶ Lost (復元不可)
```

| 状態 | 説明 |
|------|------|
| `Active` | 正常（5個のホルダーが保持中） |
| `AtRisk` | 危険（4個以下、再分散が必要） |
| `Repairing` | 修復中（再分散プロセス実行中） |
| `Lost` | 喪失（2個以下、復元不可能） |

## シェア再生成方式

**新シェア生成**を採用（オフラインノードのシェア引継ぎではない）。

### 理由

| 方式 | メリット | デメリット |
|------|----------|------------|
| **新シェア生成** | オフラインノードのシェアが「無効化」。攻撃者が後から復活しても二重取り不可 | 計算コストあり |
| シェア引継ぎ | 単純なコピー | 同じシェアが複数ノードに存在し、二重取りリスク |

### アルゴリズム

```
1. 既存ホルダーからkつ（3個）のシェアを収集
2. Lagrange補間で多項式 f(x) を復元
3. 未使用のindex（例: 6, 7, ...）で新シェアを評価: f(new_index)
4. 新シェアに対するKZG proofを生成
5. 新規ノードにPush配送
```

## ホルダー超過時のGC（復帰ノード対応）

### 問題

オフラインだったノードが復帰した場合、以下の状況が発生する：

```
1. ノードAがオフライン → スラッシュ → 再分散でノードFが新シェア(index=6)を取得
2. ノードAが復帰 → 古いシェア(index=1)をまだ保持
3. 結果: 断片Xに6個のホルダーが存在（n=5を超過）
```

### 解決策: Stale Holder GC

**優先度ルール**（低い方から削除）:

| 優先度 | 条件 | 理由 |
|--------|------|------|
| 1（最低） | スラッシュ済みノード | 信頼性が低い |
| 2 | 古いindex（1-5） | 新index（6+）は修復で生成された「有効」なシェア |
| 3 | 最終証明成功が古い | 長期間証明していない |

### 状態遷移（GC込み）

```
FragmentState:
  Active ──[holder ≤ 4]──▶ AtRisk ──[repair]──▶ Active
                                                  │
                                   [holder > n]───┘
                                                  │
                                        ▼ (GC発動)
                                  evict_stale_holder
                                        │
                                        ▼
                                  holder = n (正常化)
```

### GCトリガー（オフチェーン計算方式）

現状のスコアベースGCと同様に、ストレージノードがRPCでオンチェーン情報を取得し、判断・実行する方式を採用。

```
フロー:
1. Storage Node: 定期的に `storage_getEvictionCandidates` RPCを呼び出し
2. RPC: オンチェーンの `FragmentHolders` + `ProofRecords` から候補を計算
3. Storage Node: 自分が削除候補かを確認 → 自発的に `revoke_holding`
4. Storage Node: 他ノードが候補の場合 → `evict_stale_holder(fragment_id, target)` を提出
5. オンチェーン: 指定されたノードが本当に優先度最低か検証 → 削除実行
```

**メリット**:
- `on_finalize`への処理追加不要（計算コスト無し）
- 既存のスコアベースGCと同じパターンで一貫性あり
- ストレージノードの自律的な判断で分散的

### 復帰ノードの扱い

- **スラッシュ済み**: `AccruedRewards` が減額されている状態で復帰
- **報酬なし**: GCで削除されるまでの間、保持証明しても報酬は付与されない
- **自発的退出**: `revoke_holding` で自主的にシェアを放棄し、ストレージ容量を解放

## 実装スコープ

### pallet-storage 拡張

```rust
// 新規ストレージ
#[pallet::storage]
pub type FragmentState<T> = StorageMap<_, Blake2_128Concat, FragmentId, State>;

#[pallet::storage]
pub type AccruedRewards<T> = StorageMap<_, Blake2_128Concat, T::AccountId, Balance>;

#[pallet::storage]
pub type RepairRewardPool<T> = StorageMap<_, Blake2_128Concat, FragmentId, Balance>;

// 既存のProofRecordを拡張（新規ストレージを減らす）
// Note: SlashedNodes, ShareIndex は不要。ProofRecordにフィールド追加で対応。
pub struct ProofRecord<BlockNumber> {
    pub last_proof_block: BlockNumber,
    pub failure_count: u32,
    pub slashed: bool,           // 追加: スラッシュ済みフラグ
    pub share_index: u8,         // 追加: 保持しているシェアのindex
}

// 新規イベント
#[pallet::event]
pub enum Event<T> {
    FragmentAtRisk { fragment_id: FragmentId, holder_count: u32 },
    RepairRequested { fragment_id: FragmentId, required_count: u32 },
    RepairCompleted { fragment_id: FragmentId, new_holder: T::AccountId },
    NodeSlashed { operator: T::AccountId, amount: Balance, reason: SlashReason },
    StaleHolderDetected { fragment_id: FragmentId, excess_count: u32 },
    HolderEvicted { fragment_id: FragmentId, evicted: T::AccountId, reason: EvictReason },
}

// 新規エクストリンシック
#[pallet::call]
impl<T: Config> Pallet<T> {
    /// 報酬引き出し（下限: 500 MORAL）
    pub fn claim_rewards(origin: OriginFor<T>) -> DispatchResult;
    
    /// 再分散完了報告（報酬分配）
    pub fn confirm_repair(
        origin: OriginFor<T>,
        fragment_id: FragmentId,
        new_share_index: u32,
        kzg_proof: KzgProof,
    ) -> DispatchResult;
    
    /// 不要ホルダーの削除（GC）
    /// - 誰でも呼び出し可能（ガス代負担でインセンティブ）
    /// - 呼び出し者が削除対象を指定（オフチェーンで候補計算済み）
    /// - オンチェーンで「本当に優先度最低か」を検証してから削除
    pub fn evict_stale_holder(
        origin: OriginFor<T>,
        fragment_id: FragmentId,
        target: T::AccountId,  // 削除対象を指定
    ) -> DispatchResult;
}

// Runtime API 追加（既存のStorageRuntimeApiを拡張）
sp_api::decl_runtime_apis! {
    pub trait StorageRuntimeApi {
        // ... 既存のAPI ...
        
        /// Get eviction candidates for a fragment (holders sorted by priority)
        /// Returns: Vec<(account_id, priority_score, share_index, is_slashed)>
        fn get_eviction_candidates(
            fragment_id: FragmentId,
        ) -> Vec<EvictionCandidate>;
        
        /// Get fragments with excess holders (count > n)
        fn get_fragments_with_excess_holders() -> Vec<(FragmentId, u32)>;
    }
}
```

### storage-node 拡張

```rust
// 新規モジュール
mod health_monitor;     // FragmentState監視 + AtRisk検出
mod share_regenerator;  // k個収集 → Lagrange → 新シェア生成
mod repair_executor;    // ノード選定 → Push配送
mod repair_reporter;    // confirm_repair提出
mod stale_holder_gc;    // 復帰ノード検出 + 自発的退出 + シェア削除
```

#### HealthMonitor

- 定期的にチェーン上の `FragmentState` をポーリング
- `AtRisk` 状態のフラグメントを検出
- 自身が保持しているフラグメントの場合、修復プロセスを開始

#### ShareRegenerator

- 他のホルダーノードからk個のシェアを収集（P2P）
- Lagrange補間で多項式を復元
- 新しいindexでシェアを評価
- KZG proofを生成

#### RepairExecutor

- オンラインで空き容量のある新規ノードを選定
- 生成したシェアをPush配送
- 複数ノードが同時に修復した場合、報酬は分配

#### RepairReporter

- `confirm_repair` エクストリンシックを提出
- 報酬を受け取り

#### StaleHolderGC

既存のスコアベースGC（`gc.rs`）と同様のパターンで実装:

```rust
// storage-node/src/stale_holder_gc.rs

pub struct StaleHolderGC {
    // 削除候補キャッシュ（RPC結果）
    candidates: HashMap<FragmentId, Vec<EvictionCandidate>>,
    // チェック間隔（60秒）
    check_interval: Duration,
}

impl StaleHolderGC {
    /// 定期チェック処理
    pub async fn check(&mut self, chain: &ChainClient) {
        // 1. ホルダー超過断片を取得
        let excess = chain.get_fragments_with_excess_holders().await?;
        
        for (fragment_id, holder_count) in excess {
            // 2. 削除候補を取得（優先度順）
            let candidates = chain.get_eviction_candidates(fragment_id).await?;
            
            // 3. 自分が候補か確認
            if candidates[0].account_id == self.my_account {
                // 自発的に退出（ガス代節約）
                chain.revoke_holding(fragment_id).await?;
            } else {
                // 他ノードを削除（インセンティブ: 将来のガス代補填）
                chain.evict_stale_holder(fragment_id, &candidates[0].account_id).await?;
            }
        }
    }
}
```

**ポイント**:
- 既存の`gc.rs`と並列で動作（別の目的）
- 60秒間隔でRPC呼び出し（既存GCと同じ）
- 自分が削除候補なら`revoke_holding`（ガス代安い）
- 他ノードなら`evict_stale_holder`（ガス代負担、将来的に補填検討）

### wasm-engine 拡張

```rust
// packages/wasm-engine/src/repair.rs (新規)

/// 既存シェアから新しいインデックスのシェアを再生成
pub fn regenerate_share(
    shares: Vec<VssShare>,     // k個以上のシェア
    new_index: u32,            // 新しいシェアのインデックス
    commitment: &KzgCommitment, // 検証用コミットメント
) -> Result<(VssShare, KzgProof), Error>
```

## 実装順序（統合テスト駆動）

```
1. E2E統合テストシナリオ作成
   └─ tests/integration/repair_protocol_test.sh
   
2. pallet-storage拡張
   ├─ FragmentState + AccruedRewards
   ├─ claim_rewards + slash_node
   └─ confirm_repair + RepairRequested

3. wasm-engine拡張
   └─ repair.rs (regenerate_share)

4. storage-node拡張
   ├─ HealthMonitor (AtRisk検出)
   ├─ ShareRegenerator (新シェア生成)
   ├─ RepairExecutor (Push配送)
   └─ RepairReporter (confirm_repair)

5. フロントエンド（オプション）
   └─ 断片健全性ダッシュボード
```

## テストシナリオ

### T-001: 正常な保持報酬積み立て

```
Given: ノードAが断片を保持し、保持証明に成功
When: 報酬計算が実行される
Then: AccruedRewards[A] に報酬が積み立てられる
```

### T-002: 引き出し下限チェック

```
Given: AccruedRewards[A] = 400 MORAL（下限未満）
When: Aがclaim_rewardsを呼び出す
Then: エラー「InsufficientAccruedRewards」が返る
```

### T-003: 正常な引き出し

```
Given: AccruedRewards[A] = 600 MORAL
When: Aがclaim_rewardsを呼び出す
Then: 600 MORALがAのウォレットに転送される
And: AccruedRewards[A] = 0
```

### T-004: スラッシング発動

```
Given: ノードAがチャレンジに3回連続未応答
And: AccruedRewards[A] = 500 MORAL
When: slash_nodeが実行される
Then: 250 MORAL（50%）がRepairRewardPoolに移動
And: AccruedRewards[A] = 250 MORAL
And: NodeSlashedイベントが発行される
```

### T-005: AtRisk状態への遷移

```
Given: 断片Xのホルダーが5個
When: 2個のノードがオフラインになる（未応答3回）
Then: FragmentState[X] = AtRisk
And: FragmentAtRiskイベントが発行される
```

### T-006: 再分散完了

```
Given: 断片XがAtRisk状態（ホルダー3個）
When: ノードBが新シェア（index=6）を生成し、ノードCにPush
And: confirm_repair(X, 6, proof)が提出される
Then: FragmentState[X] = Active
And: ホルダーが4個に回復
And: RepairRewardPool[X]からBに報酬
```

### T-007: 複数ノード同時修復

```
Given: 断片XがAtRisk状態
When: ノードB, Cが同時にconfirm_repairを提出
Then: 両方の報告が受理される（異なるindex）
And: 報酬はB, Cに分配される
```

### T-008: Lost状態への遷移

```
Given: 断片Xのホルダーが3個
When: 2個のノードがオフラインになる
Then: FragmentState[X] = Lost
And: 復元不可能としてマークされる
```

### T-009: 復帰ノードによるホルダー超過

```
Given: 断片Xのホルダーが5個（index 1-5）
And: ノードA（index=1）がスラッシュされ、ノードF（index=6）が新シェアを取得
When: ノードAが復帰する
Then: holder_count = 6（超過）
And: StaleHolderDetectedイベントが発行される
```

### T-010: evict_stale_holderによるGC

```
Given: 断片Xのholder_count = 6（超過）
And: ノードA（index=1）がスラッシュ済み
When: evict_stale_holder(X)が呼び出される
Then: ノードAがホルダーから削除される
And: holder_count = 5（正常化）
And: HolderEvictedイベントが発行される
```

### T-011: 復帰ノードの自発的退出

```
Given: ノードAがスラッシュ済みで復帰
And: 断片Xのholder_count = 6
When: ノードAがrevoke_holding(X)を呼び出す
Then: ノードAがホルダーから削除される
And: ノードAのストレージ容量が解放される
```

### T-012: 優先度に基づく削除順序

```
Given: 断片Xのholder_count = 7
And: ノードA（index=1, スラッシュ済み）
And: ノードB（index=2, 正常）
And: ノードF（index=6, 正常）
When: evict_stale_holder(X)が2回呼び出される
Then: 1回目: ノードA（スラッシュ済み）が削除
And: 2回目: ノードB（古いindex）が削除
And: holder_count = 5（正常化）
```

## セキュリティ考慮事項

### Sybilアタック対策

- 再分散先のノード選定はランダム化
- 同一オペレーターの複数ノードへの集中を検出・制限
- PoWによるノード登録制限

### 共謀耐性

- k個のシェアを持つノードが共謀しても、新シェア生成には既存ホルダーの協力が必要
- KZG検証により不正なシェアを検出

### DoS耐性

- confirm_repairのレート制限
- 虚偽のAtRisk報告への対策（オンチェーン検証）

## 既知の問題と対応策

### 1. 検出〜修復の時間差問題

**問題**: 複数ノードが同時にオフラインになった場合、k=3閾値に到達する前に修復が完了しない可能性

```
チャレンジ未応答3回 = 100分 × 3 = 300分 で検出
+ 再分散プロセス実行 = 推定100〜200分
= 合計 400〜500分で1ノード分の修復
```

**対応策**:
- 検出閾値を「2回未応答」に短縮（設計変更時に検討）
- 複数ノード同時離脱の確率は低いため、現状パラメータで初期運用

### 2. GCトリガーのインセンティブ不足

**問題**: `evict_stale_holder` 呼び出し者はガス代を負担するが、直接報酬がない

**対応策**:
| 方式 | メリット | デメリット |
|------|----------|------------|
| **A) 自発的退出優先** | スラッシュ済みノードは自ら退出（ガス代節約） | 悪意ノードは無視する可能性 |
| **B) 小額報酬** | GC実行者に削除対象のAccruedRewardsの5%を付与 | 実装複雑化 |
| **C) 優先GC権** | GC実行者は次回チャレンジで優先権を得る | 間接的で弱い |

**採用**: 初期実装ではA方式。B方式は将来的に検討。

### 3. 新シェア検証の信頼性

**問題**: `confirm_repair` でKZG proofを検証するが、「このシェアは正当に再生成されたか」の検証が複雑

```
攻撃シナリオ:
1. 悪意あるノードが偽のシェアを生成
2. オンチェーン検証をバイパス？
```

**対応策**:
- KZG commitment との整合性検証は必須（`e(C, [1]) = e(π, [τ - index])` 形式）
- 追加の検証レイヤー: k個のホルダーからの署名付き承認（Phase 2で検討）
- 現実的には、正当なシェアでないと保持証明で失敗するため、長期的リスクは低い

### 4. P2Pシェア収集プロトコルの不在

**問題**: storage-node がk個のシェアを他ノードから収集するプロトコルがない

**現状**: `FragmentRequest::Get/Put` のみ実装

**対応策**:
```rust
// 新規プロトコル追加
pub enum RepairRequest {
    /// シェア収集要求（修復用）
    CollectShare {
        content_hash: [u8; 32],
        requester_proof: RequesterProof,  // 正当な修復者であることの証明
    },
    /// シェア提供応答
    ShareResponse {
        share: VssShare,
        signature: Signature,  // 提供者の署名
    },
}
```

**工数**: 2〜3週間の新規開発が必要

### 5. Sybil攻撃の限定的対策

**問題**: 異なるアカウントで複数ノードを運用するSybilは検出不可能

**対応策**:
| 方式 | 実装難易度 | 効果 |
|------|-----------|------|
| **PoW制限** | 実装済み | 経済的コストを上げるが、完全ではない |
| **地理的分散要求** | 高 | IPベースは回避可能 |
| **ステーキング** | 中 | デポジット方式と同じ問題 |
| **レピュテーション** | 低 | 長期運用実績で信頼度を上げる |

**採用**: PoW + 長期的にレピュテーションシステム導入を検討

### 6. 残高ゼロノードのゾンビ化

**問題**: AccruedRewards = 0 でスラッシュ済みのノードが、ペナルティなしでシステムに居座る可能性

```
シナリオ:
1. ノードAが登録直後にオフライン → AccruedRewards = 0
2. スラッシュ発動 → 0 × 50% = 0（実質ペナルティなし）
3. ノードAは失うものがない状態で悪意ある行動可能
```

**対応策**:
- **登録時最低運用期間**: 一定期間（例: 7日）のチャレンジ応答義務
- **スラッシュフラグ**: AccruedRewards = 0 でも `slashed = true` をマーク → GC優先度最低に
- **自動登録解除**: スラッシュ2回で強制的にノード登録解除

**採用**: スラッシュフラグ方式を実装

### 7. 修復競合（Race Condition）

**問題**: 複数ノードが同時に同じ断片の修復を試みた場合

```
1. ノードB, Cが同時にAtRisk検出
2. 両方がシェア収集 → 両方が新シェア生成（index=6, 7）
3. 両方がconfirm_repair提出
```

**対応策**:
- **許容**: 両方のシェアを受け入れ（n=5→n=7）、GCで正常化
- **ロック**: AtRisk→Repairing 遷移時に「修復中」フラグで排他制御
- **遅延ランダム化**: 各ノードがランダム遅延後に修復開始

**採用**: 許容方式（シンプル、GCで自然に解消）

### 8. regenerate_share の実装課題

**問題**: 現在の `vss_recover` はデータ復元が目的であり、多項式係数を直接返す設計になっていない

**対応策**:
```rust
// vss.rs への追加
pub fn regenerate_share(
    shares: &[VssShare],
    threshold: u32,
    new_index: u8,
) -> Result<VssShare, KzgError> {
    // 1. Lagrange補間で多項式を復元（既存ロジック流用）
    let polynomial = lagrange_interpolate(&points)?;
    
    // 2. 新しいindexで評価
    let new_x = Fr::from(new_index as u64);
    let new_y = polynomial.evaluate(&new_x);
    
    // 3. 新シェアを返す
    Ok(VssShare {
        index: new_index,
        value: fr_to_bytes(new_y),
    })
}
```

**工数**: 約2時間（既存コードの拡張）

## 今後の検討事項

- [x] ~~オフラインノードの復帰時の取り扱い~~ → Stale Holder GCで対応
- [ ] スラッシュ率の動的調整（ネットワーク状態に応じて）
- [ ] 複数断片の同時修復効率化（バッチ処理）
- [ ] 修復優先度アルゴリズム（スコアが高い投稿を優先）
- [ ] evict_stale_holder呼び出し者へのガス代補填インセンティブ
