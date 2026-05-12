# Research: 自己修復プロトコル

**Feature**: 013-slashing-repair  
**Created**: 2026-02-24

## Research Questions

1. 既存のProofRecordとPendingRewardsをどう拡張するか？
2. regenerate_share関数の実装方法（Lagrange補間の活用）
3. P2Pシェア収集プロトコルの設計
4. スラッシングメカニズムのベストプラクティス
5. 断片状態管理（FragmentState）の設計

---

## 1. ProofRecordとPendingRewardsの拡張

### 現状分析

**既存のProofRecord構造体** (`apps/blockchain/pallets/storage/src/lib.rs:243`):
```rust
pub struct ProofRecord<BlockNumber: Default> {
    pub last_proved_at: BlockNumber,
    pub success_count: u32,
    pub failure_count: u32,
}
```

**既存のPendingRewards** (`apps/blockchain/pallets/storage/src/lib.rs:506`):
- `StorageMap<AccountId, u128>` - ノードごとの保留報酬

### 決定事項

**ProofRecordの拡張**:
```rust
pub struct ProofRecord<BlockNumber: Default> {
    pub last_proved_at: BlockNumber,
    pub success_count: u32,
    pub failure_count: u32,
    pub slashed: bool,           // 追加: スラッシュ済みフラグ
    pub share_index: u8,         // 追加: 保持しているシェアのindex
}
```

**PendingRewardsの再利用**:
- 仕様書の「AccruedRewards」は既存の`PendingRewards`に対応
- 追加変更不要、既存のclaim_rewards関数に引き出し下限チェックを追加

**根拠**:
- 既存構造体の拡張で新規ストレージを最小化
- マイグレーションが必要だが、新規ストレージ追加より複雑度が低い

**代替案（却下）**:
- 完全に新しいストレージ（SlashedNodes, ShareIndexes）を追加 → ストレージ肥大化、クエリ複雑化

---

## 2. regenerate_share関数の実装

### 現状分析

**既存のVSS実装** (`packages/wasm-engine/src/kzg/vss.rs`):
- `vss_split`: データをk-of-nシェアに分割
- `vss_recover`: k個のシェアからデータを復元
- `lagrange_interpolate`: k点から多項式を補間（内部関数）

### 決定事項

**新関数: regenerate_share**
```rust
/// 既存シェアから新しいインデックスのシェアを再生成
pub fn regenerate_share(
    shares: &[VssShare],     // k個以上のシェア
    threshold: u8,           // 閾値k
    new_index: u8,           // 新しいシェアのインデックス（6以上を推奨）
    commitment: &KzgCommitment, // 検証用コミットメント
) -> Result<(VssShare, KzgProof), KzgError> {
    // 1. shares.len() >= threshold を検証
    // 2. new_indexが既存シェアのindexと重複していないことを確認
    // 3. lagrange_interpolateで多項式を復元
    // 4. polynomial.evaluate(Fr::from(new_index))で新シェア値を計算
    // 5. create_evaluation_proofで新シェアのKZG proofを生成
    // 6. (VssShare, KzgProof)を返す
}
```

**実装ステップ**:
1. `lagrange_interpolate`を`pub(crate)`に変更（既に内部で使用）
2. 新関数でpolynomial.evaluate()を呼び出し
3. 既存の`create_evaluation_proof`を再利用

**根拠**:
- 既存のLagrange補間コードを最大限再利用
- KZG proofの生成も既存コードを活用
- 新コードは約50行程度で実装可能

**代替案（却下）**:
- 完全に新しい補間実装 → 重複コード、バグリスク増大

---

## 3. P2Pシェア収集プロトコル

### 現状分析

**既存のP2Pプロトコル** (`apps/storage-node/src/network/`):
- `FragmentRequest::Get/Put` - 断片の取得・配布
- libp2p request-response protocol

### 決定事項

**新プロトコルの追加**:
```rust
pub enum RepairRequest {
    /// 修復用シェア収集要求
    CollectShare {
        content_hash: [u8; 32],
        requester: AccountId,
        /// 正当な修復者であることの署名
        signature: Vec<u8>,
    },
}

pub enum RepairResponse {
    /// シェア提供応答
    ShareProvided {
        share: VssShare,
        /// 提供者の署名（改ざん防止）
        signature: Vec<u8>,
    },
    /// 拒否（シェアを持っていない、または検証失敗）
    Rejected { reason: String },
}
```

**プロトコルフロー**:
1. HealthMonitorがAtRisk断片を検出
2. RepairExecutorがk個のホルダーノードを選定（FragmentHoldersから取得）
3. 各ホルダーに`CollectShare`リクエストを送信
4. k個の応答を収集（タイムアウト: 30秒）
5. `regenerate_share`で新シェア生成
6. 新ノードに`FragmentRequest::Put`で配送
7. `confirm_repair`をチェーンに提出

**根拠**:
- 既存のrequest-responseパターンを踏襲
- 署名による認証で悪意あるリクエストを防止

---

## 4. スラッシングメカニズム

### 業界ベストプラクティス

| プロジェクト | スラッシュ率 | 検出方法 | 回復 |
|-------------|-------------|---------|------|
| Ethereum 2.0 | 1/32 〜 全額 | 二重署名、サボタージュ | 出金待機期間 |
| Polkadot | 可変（重大度依存） | オフライン、不正署名 | unbonding期間後 |
| Filecoin | 担保没収 | WindowPoSt失敗 | 即時罰則 |

### 決定事項

**Anarchyのスラッシング設計**:
- **スラッシュ率**: 50%/違反（2回で実質全損）
- **検出**: チャレンジ3回連続未応答
- **回復**: 即時復帰可（参加制限なし）
- **原資**: `PendingRewards`から没収（デポジット不要）

**スラッシュ実行ロジック**:
```rust
fn slash_node(who: &T::AccountId, fragment_id: FragmentId) -> DispatchResult {
    let pending = PendingRewards::<T>::get(who);
    let slash_amount = pending / 2; // 50%
    
    // 修復報酬プールに移動
    RepairRewardPool::<T>::mutate(fragment_id, |pool| {
        *pool = pool.saturating_add(slash_amount);
    });
    
    // PendingRewardsから差し引き
    PendingRewards::<T>::mutate(who, |balance| {
        *balance = balance.saturating_sub(slash_amount);
    });
    
    // ProofRecordにslashedフラグを設定
    ProofRecords::<T>::mutate(fragment_id, who, |record| {
        record.slashed = true;
    });
    
    Self::deposit_event(Event::NodeSlashed { ... });
    Ok(())
}
```

**根拠**:
- デポジット不要でノード参加障壁を低く維持
- 50%は十分な抑止力（2回で退場）ながら、一度の失敗で全損しない
- 即時復帰可で善意のオペレーターに配慮

---

## 5. FragmentState管理

### 決定事項

**状態遷移図**:
```
Active ──[holder ≤ 4]──▶ AtRisk
  ▲                        │
  │                        ▼
  └──[holder = 5]── Repairing ──[60分タイムアウト]──┐
                           │                        │
                [confirm_repair成功]                 │
                           │                        │
                           ▼                        │
                        Active ◀────────────────────┘
                           
AtRisk ──[holder ≤ 2]──▶ Lost ──[30日後]──▶ (削除)
```

**ストレージ設計**:
```rust
#[pallet::storage]
pub type FragmentStates<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    FragmentId,
    FragmentState<BlockNumberFor<T>>,
    ValueQuery,
>;

pub struct FragmentState<BlockNumber> {
    pub state: StateKind,
    pub state_changed_at: BlockNumber,
    pub repair_started_at: Option<BlockNumber>,
}

pub enum StateKind {
    Active,
    AtRisk,
    Repairing,
    Lost,
}
```

**状態更新トリガー**:
- `on_finalize`: PendingChallenges処理後にホルダー数チェック
- `confirm_repair`: Repairing → Active
- `evict_stale_holder`: ホルダー超過解消後の再評価

**根拠**:
- BlockNumberを保持することでタイムアウト計算が可能
- ValueQueryでデフォルトActive（新規断片は自動的にActive）

---

## Summary

| 質問 | 決定 | 根拠 |
|------|------|------|
| ProofRecord拡張 | `slashed`, `share_index`フィールド追加 | 新規ストレージ最小化 |
| AccruedRewards | 既存`PendingRewards`を再利用 | コード再利用、一貫性 |
| regenerate_share | 既存lagrange_interpolate + evaluate | 約50行で実装可能 |
| P2Pプロトコル | RepairRequest/Response追加 | 既存パターン踏襲 |
| スラッシュ率 | 50%/違反 | 抑止力と回復のバランス |
| スラッシュ復帰 | 即時復帰可 | 善意オペレーター配慮 |
| FragmentState | 4状態 + タイムスタンプ | タイムアウト管理 |
