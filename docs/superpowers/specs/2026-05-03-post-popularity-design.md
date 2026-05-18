# 投稿人気度システム 設計書

**Date**: 2026-05-03
**Scope**: TODO.md §3.4 投稿人気度システム の v1 実装
**Reference**: [docs/CONCEPTS.md §投稿人気度システム](../../vision/concepts.md#投稿人気度システム)

## 1. 目的とスコープ

オンチェーンの react カウント (Like / Bad) から **投稿ごとの人気度スコア** を算出し、減衰によって関心を失った投稿を **猶予期間付きで削除候補化** する仕組みを導入する。CONCEPTS.md の方針通り、fetch ベースのスコアは採用しない (Sybil 脆弱 / 匿名性矛盾 / 処理リソースの三重苦)。

### 1.1 含む

- 新規 `pallet-popularity` (score / 減衰 / 削除マーク / 削除実行)
- `pallet-reaction` の `ReactionType` から `Boost` を削除し `Like` / `Bad` の 2 種に整理
- `pallet-reaction` → `pallet-popularity` への push (`react()` 時に score 加算)
- `pallet-storage::StorageInterface` に `do_release_fragment` を追加 (削除確定時にフラグメント参照解放 + storage node への忘却シグナル)
- `pallet-post` に `PostMutator` impl を追加 (`Posts` / `ContentRefs` / `MerkleRootToPostId` / `UserPosts` の prune)
- on_finalize での bounded batch scan + bounded batch deletion
- Runtime API (effective_score / net_count 取得用)

### 1.2 含まない (v1 では out-of-scope)

- **永続化オプション** (追加料金で削除対象外にする機能) — v2 で別 spec
- **Reactor reputation / age weighting** によるスコア重み — 既存の `AlreadyReacted` + PoW + faucet rate-limit + 投稿コスト (`PostBaseCost = 10 MORAL`) で Sybil 耐性を確保
- **Governance による decay rate 動的変更** — v1 は `#[pallet::constant]` のみ
- **Frontend UI** (人気度バッジ、削除予告通知、ランキング表示) — v1 は chain layer の API 提供まで
- **on-chain sorted index による top-N ランキング** — frontend が Runtime API で全件取得しソートする方式とする

## 2. アーキテクチャ

### 2.1 パレット境界

```
pallet-reaction --(push score delta via PopularityInterface)--> pallet-popularity
                                                                       |
                                                                       | (on_finalize:
                                                                       |   decay + mark/unmark + delete)
                                                                       v
                                                              prune エントリ
                                                                       |
                                                       +---------------+----------------+
                                                       v                                v
                                              pallet-post                       pallet-storage
                                          (Posts / ContentRefs               (do_release_fragment
                                           / MerkleRootToPostId               → FragmentMetadata /
                                           / UserPosts 削除                     KzgFragments /
                                           via PostMutator trait)               ProofRecords 削除
                                                                                + ForgottenByPolicy event)
```

#### 循環依存回避

- `pallet-reaction` と `pallet-post` は新 trait `PopularityInterface` に依存 (現状の `ReactionInterface` を `pallet-post` が利用しているのと同じパターン)
- `pallet-popularity` は新 trait `PostMutator` / 拡張 `StorageInterface` 経由で `pallet-post` / `pallet-storage` に書き込み
- `runtime/src/lib.rs` の `construct_runtime!` ではどの順でも構わない (tight coupling は trait 経由で解決済み)

### 2.2 スコアモデル — 遅延減衰 (lazy relative decay)

毎ブロック全投稿スキャンは O(N) で許容できないため、次の方針を取る。

| 項目 | 方式 |
|------|------|
| 減衰方式 | **相対減衰** `score *= decay_rate ^ Δblocks` |
| 適用タイミング | **lazy** — read / react / on_finalize scan 時に `(stored_score, last_touched)` から effective_score を再計算し書き戻す |
| on_finalize 処理 | **bounded scan** — 1 ブロックあたり最大 `MaxPostsScannedPerBlock` 件 (例: 8) を round-robin 走査 |

これで毎ブロック全件スキャンを避けつつ、untouched で react のない古い投稿も最終的に処理される。

#### 固定小数点

- `score: u64`
- `decay_rate: Permill` (例: 999_950 → 1 ブロックあたり 0.999950 倍)
- 減衰計算は `decay.rs` に分離 (純関数 → unit test 容易)
- Δblocks は `MaxDecaySteps` (例: 1_000_000) で clamp し、ループまたは事前計算による累乗で算出

### 2.3 状態遷移

```
 [Active] --(reaction)--> stored_score 加算, last_touched=now,
   ^                       marked_for_deletion_at をクリア
   |
   | (effective_score >= LowPopularityThreshold + HysteresisMargin で復帰)
   |
 [MarkedForDeletion(at: BlockNumber)]
   |
   | (on_finalize scan: effective_score < LowPopularityThreshold)
   |   ↑ Active からこの状態へ遷移するときに DeletionQueue に挿入
   |
   | (current_block - at >= GracePeriod, 例: 7 days = 100_800 blocks at 6s)
   v
 [Deleted]  ← prune Posts/ContentRefs/UserPosts/MerkleRootToPostId,
              call do_release_fragment, emit PostDeleted event,
              PostScores から完全削除
```

ヒステリシス: `pallet-storage` の forgetting candidate 復帰条件 (`recovery_threshold = threshold + hysteresis_margin`) と同じパターンで flapping を防止する。

## 3. データ構造

### 3.1 pallet-popularity の Storage

```rust
#[derive(Encode, Decode, MaxEncodedLen, TypeInfo, Clone, RuntimeDebug, PartialEq, Eq, Default)]
pub struct PostPopularity<BlockNumber> {
    /// 減衰込みの interest score (削除判定用)
    pub stored_score: u64,
    /// 最終 touch ブロック (decay 計算の基準点)
    pub last_touched: BlockNumber,
    /// 累積 Like 数 (減衰なし)
    pub like_count: u32,
    /// 累積 Bad 数 (減衰なし)
    pub dislike_count: u32,
    /// 削除候補としてマークされたブロック (None なら Active)
    pub marked_for_deletion_at: Option<BlockNumber>,
}

// 投稿ごとの人気度
#[pallet::storage]
pub type PostScores<T: Config> = StorageMap<
    _, Blake2_128Concat, u64 /* post_id */,
    PostPopularity<BlockNumberFor<T>>, OptionQuery
>;

// 削除予定キュー (eligible_at = marked_at + GracePeriod)
#[pallet::storage]
pub type DeletionQueue<T: Config> = StorageMap<
    _, Blake2_128Concat, u64 /* post_id */,
    BlockNumberFor<T> /* eligible_at */, OptionQuery
>;

// round-robin scan のカーソル
#[pallet::storage]
pub type ScanCursor<T: Config> = StorageValue<_, u64, ValueQuery>;
```

**設計判断**: `net_count` (= `like_count - dislike_count`) は state に持たず、Runtime API 側で派生計算する。理由は §6.4 参照。

### 3.2 pallet-reaction の変更

```rust
// 変更前
pub enum ReactionType { Like, Boost, Bad }
impl ReactionType { pub fn weight(&self) -> u128 { Like=>1, Boost=>5, Bad=>0 } }

pub struct ReactionStats {
    pub likes: u32, pub boosts: u32, pub bads: u32,
    pub total_weight: u128,
}

// 変更後
pub enum ReactionType { Like, Bad }
// weight() は削除 (Bad のみ payout 除外という分岐は既存のまま維持)

pub struct ReactionStats {
    pub likes: u32,
    pub bads: u32,
    // boosts / total_weight は削除
}
```

**互換性**: CLAUDE.md の互換性ポリシーに従い、既存 chain state は破棄して dev chain を再生成する。マイグレーションコードは書かない。

## 4. Config 定数

すべて `#[pallet::constant]`。Runtime での値はチューニング前提の暫定値。

| 名前 | 型 | 暫定値 | 役割 |
|------|----|----|------|
| `InitialScore` | `u64` | 100_000 | 投稿時の base score (`on_post_created` でセット) |
| `LikeWeight` | `u64` | 100 | Like の score delta |
| `DislikeWeight` | `u64` | 50 | Bad の score delta (CONCEPTS.md「低評価も関心として加点」) |
| `DecayRatePermill` | `Permill` | 999_950 | per-block 減衰率 (約 1% / 200 blocks ≒ 20 min) |
| `LowPopularityThreshold` | `u64` | 1_000 | これ未満で `marked_for_deletion_at` をセット |
| `HysteresisMargin` | `u64` | 500 | 復帰条件: `eff_score >= threshold + margin` |
| `GracePeriod` | `BlockNumberFor<T>` | 100_800 | 7 days @ 6s/block |
| `MaxPostsScannedPerBlock` | `u32` | 8 | on_finalize での scan 件数上限 |
| `MaxDeletionsPerBlock` | `u32` | 4 | on_finalize での delete 件数上限 |
| `MaxDecaySteps` | `u32` | 1_000_000 | 減衰 Δblocks の clamp 上限 |

**寿命の概算** (react 0 件の投稿の場合):

`InitialScore = 100_000`、`DecayRatePermill = 999_950` で `LowPopularityThreshold = 1_000` に到達するまで約 92_103 ブロック (= ln(1000/100_000) / ln(0.999950) ≒ 6.4 日)、+ `GracePeriod = 7 日` = 約 **13.4 日** で削除される。妥当なオーダーかは testnet チューニング項目とする。

## 5. インターフェース (Trait)

### 5.1 PopularityInterface (pallet-popularity が提供)

```rust
#[derive(Clone, Copy)]
pub enum PopularityReactionType { Like, Dislike }

pub trait PopularityInterface {
    /// 投稿作成時に呼ばれる。InitialScore を持つ Active 状態の PostPopularity を作成。
    fn on_post_created(post_id: u64);

    /// reaction 時に呼ばれる。decay 適用 + score delta 加算 + counter 更新 + mark クリア。
    fn on_reaction(post_id: u64, kind: PopularityReactionType);
}

// pallet-reaction::Config に追加: type Popularity: PopularityInterface;
// pallet-post::Config に追加:     type Popularity: PopularityInterface;
// () impl も提供 (test-only runtime 用)
```

### 5.2 PostMutator (pallet-post が提供) — 新設

```rust
pub trait PostMutator<AccountId> {
    /// 削除確定時に pallet-popularity から呼ばれる。
    /// Posts / ContentRefs / MerkleRootToPostId / UserPosts を prune し、
    /// Storage 側に release を依頼するため merkle_root を返す。
    fn delete_post(post_id: u64) -> Result<[u8; 32] /* merkle_root */, DispatchError>;
}

// pallet-popularity::Config に追加: type PostMutator: PostMutator<Self::AccountId>;
```

### 5.3 StorageInterface 拡張

```rust
pub trait StorageInterface<AccountId, BlockNumber> {
    // ... 既存 do_register_fragment / do_register_kzg_fragment / do_deposit_to_reward_pool ...

    /// 投稿削除確定時に呼ばれる。FragmentMetadata / KzgFragments / ProofRecords を prune し、
    /// `ForgottenByPolicy { content_hash }` event を emit。
    /// storage node はこの event を監視し、保有フラグメントを物理削除する。
    fn do_release_fragment(content_hash: ContentHash) -> DispatchResult;
}
```

### 5.4 Runtime API

```rust
sp_api::decl_runtime_apis! {
    pub trait PopularityApi {
        /// 現時点の effective score (decay 適用後) を返す。
        /// State は更新しない (read-only)。
        fn get_effective_score(post_id: u64) -> Option<u64>;

        /// like_count - dislike_count を i64 で返す (Reddit 的 net シグナル)。
        fn get_net_count(post_id: u64) -> Option<i64>;

        /// すべての人気度情報を一括取得。
        fn get_post_popularity(post_id: u64) -> Option<PostPopularityRpc>;
    }
}

#[derive(Encode, Decode, TypeInfo)]
pub struct PostPopularityRpc {
    pub effective_score: u64,
    pub like_count: u32,
    pub dislike_count: u32,
    pub net_count: i64,
    pub marked_for_deletion_at: Option<u32>,
    pub last_touched: u32,
}
```

## 6. 主要ロジック

### 6.1 on_post_created

```rust
fn on_post_created(post_id: u64) {
    let now = frame_system::Pallet::<T>::block_number();
    PostScores::<T>::insert(post_id, PostPopularity {
        stored_score: T::InitialScore::get(),
        last_touched: now,
        like_count: 0,
        dislike_count: 0,
        marked_for_deletion_at: None,
    });
}
```

`pallet-post::create_post` の末尾 (event 発行直前) から呼び出す。

### 6.2 on_reaction

```rust
fn on_reaction(post_id: u64, kind: PopularityReactionType) {
    let now = frame_system::Pallet::<T>::block_number();
    PostScores::<T>::mutate(post_id, |entry| {
        let p = entry.get_or_insert_with(|| PostPopularity {
            stored_score: T::InitialScore::get(),
            last_touched: now,
            like_count: 0,
            dislike_count: 0,
            marked_for_deletion_at: None,
        });
        // 1. decay 適用
        let delta_blocks = now.saturating_sub(p.last_touched);
        p.stored_score = decay::apply(p.stored_score, delta_blocks, T::DecayRatePermill::get(), T::MaxDecaySteps::get());
        p.last_touched = now;

        // 2. delta 加算 + counter 更新
        let delta = match kind {
            PopularityReactionType::Like    => { p.like_count = p.like_count.saturating_add(1); T::LikeWeight::get() }
            PopularityReactionType::Dislike => { p.dislike_count = p.dislike_count.saturating_add(1); T::DislikeWeight::get() }
        };
        p.stored_score = p.stored_score.saturating_add(delta);

        // 3. mark の即時解除 (リカバリ条件を満たすなら)
        if let Some(_) = p.marked_for_deletion_at {
            let recovery = T::LowPopularityThreshold::get().saturating_add(T::HysteresisMargin::get());
            if p.stored_score >= recovery {
                p.marked_for_deletion_at = None;
                DeletionQueue::<T>::remove(post_id);
                // emit PostUnmarkedForDeletion
            }
        }
    });
}
```

`pallet-reaction::react()` の末尾 (event 発行直前) から、`Bad` は `Dislike` にマップして呼ぶ。

### 6.3 on_finalize

```rust
fn on_finalize(now: BlockNumberFor<T>) {
    let max_post_id = T::PostCountProvider::next_post_id(); // pallet-post から取得
    let scan_limit = T::MaxPostsScannedPerBlock::get();
    let mut cursor = ScanCursor::<T>::get();
    let mut scanned = 0u32;

    // (a) Scan: bounded 件数だけ effective_score を再計算 + mark/unmark
    while scanned < scan_limit && max_post_id > 0 {
        if cursor >= max_post_id { cursor = 0; }
        if let Some(mut p) = PostScores::<T>::get(cursor) {
            let delta_blocks = now.saturating_sub(p.last_touched);
            let eff = decay::apply(p.stored_score, delta_blocks, T::DecayRatePermill::get(), T::MaxDecaySteps::get());
            let threshold = T::LowPopularityThreshold::get();
            let recovery = threshold.saturating_add(T::HysteresisMargin::get());

            if eff < threshold && p.marked_for_deletion_at.is_none() {
                p.marked_for_deletion_at = Some(now);
                DeletionQueue::<T>::insert(cursor, now.saturating_add(T::GracePeriod::get()));
                Self::deposit_event(Event::PostMarkedForDeletion { post_id: cursor, marked_at: now });
            } else if eff >= recovery && p.marked_for_deletion_at.is_some() {
                p.marked_for_deletion_at = None;
                DeletionQueue::<T>::remove(cursor);
                Self::deposit_event(Event::PostUnmarkedForDeletion { post_id: cursor });
            }
            p.stored_score = eff;
            p.last_touched = now;
            PostScores::<T>::insert(cursor, p);
        }
        cursor = cursor.saturating_add(1);
        scanned = scanned.saturating_add(1);
    }
    ScanCursor::<T>::put(if cursor >= max_post_id { 0 } else { cursor });

    // (b) Deletion: DeletionQueue から eligible_at <= now を bounded 件数削除
    let del_limit = T::MaxDeletionsPerBlock::get();
    let mut deleted = 0u32;
    let to_delete: Vec<(u64, BlockNumberFor<T>)> = DeletionQueue::<T>::iter()
        .filter(|(_, eligible_at)| now >= *eligible_at)
        .take(del_limit as usize)
        .collect();
    for (post_id, _) in to_delete {
        match T::PostMutator::delete_post(post_id) {
            Ok(merkle_root) => {
                let _ = T::Storage::do_release_fragment(merkle_root); // best-effort
                PostScores::<T>::remove(post_id);
                DeletionQueue::<T>::remove(post_id);
                Self::deposit_event(Event::PostDeleted { post_id });
                deleted = deleted.saturating_add(1);
            }
            Err(_) => {
                // post 既に消えている等のレースは queue から外して続行
                DeletionQueue::<T>::remove(post_id);
            }
        }
    }
}
```

`PostCountProvider` トレイト (新設) を `pallet-post` が impl して `NextPostId` の値を返す。Storage への直接依存を避ける。

### 6.4 net_count を派生にする理由

- on-chain ロジックは `interest_score` (= `stored_score`) のみで動作し、`net_count` を読まない → state に持つ必然性がない
- `like_count` と `dislike_count` は同じ `PostPopularity` struct 内にあるので、storage read は派生でも 1 回 (物理コスト同じ)
- i64 を別フィールド化すると **同期ずれ** リスク (片方だけ更新する code path が混入する余地)、エンコードサイズ +8 bytes/post も増える
- ランキング (top-N by net_count) は frontend / indexer が Runtime API 経由で全件取得 → ソートで対応する。on-chain sorted index は YAGNI

### 6.5 decay::apply 純関数

```rust
pub fn apply(score: u64, delta_blocks: u32, decay_rate: Permill, max_steps: u32) -> u64 {
    let steps = delta_blocks.min(max_steps);
    if steps == 0 || score == 0 { return score; }
    let rate = decay_rate.deconstruct() as u128; // out of 1_000_000
    let mut s = score as u128;
    for _ in 0..steps {
        s = s.saturating_mul(rate) / 1_000_000;
        if s == 0 { return 0; }
    }
    s as u64
}
```

実装上は naive ループでも、`MaxDecaySteps` を `1_000_000` に設定すると 1 read あたり最悪 1M 回乗算で重い。**最適化**として `pow_table: [u64; 32]` 風の事前計算 (2 のべき乗ステップごとの decay_rate^N を事前計算し、Δ を 2 進分解して乗算回数を log(Δ) に落とす) を入れる。これはパフォーマンステストで判明したら追加するレベルで、初版は naive で良い (テストで `MaxDecaySteps` を小さくして動作確認)。

## 7. Sybil 対策

CONCEPTS.md §投稿人気度システム の「自演スコア操作の防止」項目への回答。

**v1 では既存防御層に依存**:

| 既存防御 | 効果 |
|---------|------|
| `pallet-reaction` の `AlreadyReacted` チェック | 同一アカウントによる多重 react を防止 |
| PoW (`react()` で nonce 検証 + 動的難易度調整) | 大量 react 攻撃のコストを上げる |
| `pallet-faucet` のレート制限 | アカウント大量生成の経済コスト |
| `PostBaseCost` (10 MORAL) + 焼却 | 自分の投稿を持ち上げるための投稿コスト自体が発生 |

**追加対策は v1 では入れない**:

- Reactor の age / reputation 重み付け: over-engineering。実害が見られたら v2 で検討
- Bad の経済コスト (例: bad は 0.1 MORAL のミニマル fee): 既存の PoW で十分
- `BoostWeight` (削除済) のような高重み reaction は廃止

## 8. ファイル / モジュール配置

```
apps/blockchain/pallets/popularity/
├── Cargo.toml
└── src/
    ├── lib.rs           # Config / Storage / Events / Errors / on_finalize / impls / Runtime API decl
    ├── decay.rs         # 純関数 apply (テスト容易)
    ├── mock.rs          # mock runtime (pallet-balances + minimum)
    └── tests.rs         # extensive unit tests
```

### 8.1 既存パレットへの修正

| ファイル | 修正内容 |
|---------|---------|
| `pallets/reaction/src/lib.rs` | `ReactionType::Boost` 削除 / `weight()` 削除 / `ReactionStats.boosts` `total_weight` 削除 / `react()` から `Popularity::on_reaction()` 呼び出し / `Config::Popularity` 追加 |
| `pallets/reaction/src/tests.rs` | Boost 系テスト削除 / Popularity stub 接続 |
| `pallets/post/src/lib.rs` | `create_post()` 末尾に `Popularity::on_post_created()` 呼び出し / `Config::Popularity` 追加 / `PostMutator` impl |
| `pallets/storage/src/lib.rs` | `StorageInterface::do_release_fragment` 追加 / `ForgottenByPolicy` event 追加 |
| `runtime/src/lib.rs` | `pallet_popularity` 追加 / `Config::Popularity` を Reaction / Post に配線 / Runtime API 実装 |

### 8.2 frontend / integration への波及

- `apps/frontend/`: Boost UI / 関連 i18n / state 削除 (実装時に grep で洗い出し)
- `apps/blockchain/tests/integration/`: Boost を使うシナリオを Like / Bad に置換、popularity 削除フローを通すシナリオを追加

## 9. テスト戦略

`superpowers:test-driven-development` + プロジェクトの `.claude/skills/tdd-workflow/SKILL.md` に従う。

### 9.1 単体 (decay.rs)

- Δ=0 で score 不変
- score=0 で常に 0
- score=u64::MAX, Δ=u32::MAX (clamp 後) でオーバーフローなし
- 既知の値での減衰結果 (e.g. `apply(100_000, 200, 999_950) ≈ 99_004`)

### 9.2 単体 (pallet-popularity tests.rs)

| ケース | 検証 |
|-------|------|
| `on_post_created` | `PostScores` に `InitialScore` で entry 作成 |
| `on_reaction(Like)` | `like_count +=1`, `stored_score += LikeWeight`, `last_touched` 更新 |
| `on_reaction(Dislike)` | `dislike_count +=1`, `stored_score += DislikeWeight` |
| 時間経過 + read | effective_score が decay する |
| `on_finalize` scan | threshold 跨ぎで `marked_for_deletion_at` がセットされ、`DeletionQueue` に登録される |
| 復帰 (hysteresis) | `eff >= threshold + margin` で mark 解除、それ以下では解除されない (flapping 防止) |
| `GracePeriod` 経過 | `on_finalize` で `delete_post` + `do_release_fragment` が呼ばれ、`PostScores` から削除 |
| `MaxPostsScannedPerBlock` | 1 ブロックでの scan 件数が上限を超えない |
| `MaxDeletionsPerBlock` | 1 ブロックでの delete 件数が上限を超えない |
| ScanCursor wrap-around | `next_post_id` を超えたら 0 に戻る |
| `delete_post` 失敗 | queue から該当エントリを除外して続行 (panic しない) |

### 9.3 pallet-reaction の修正テスト

- Boost 系ケース削除
- `Like` で `Popularity::on_reaction(Like)` が呼ばれる (mock counter で確認)
- `Bad` で `Popularity::on_reaction(Dislike)` が呼ばれる
- 既存の reward / difficulty / dedup ロジックは Like / Bad のみで pass

### 9.4 pallet-post の修正テスト

- `create_post` で `Popularity::on_post_created` が呼ばれる
- `PostMutator::delete_post` が `Posts` / `ContentRefs` / `MerkleRootToPostId` / `UserPosts` をすべて削除する
- 削除後に `merkle_root` を返す

### 9.5 pallet-storage の修正テスト

- `do_release_fragment` で `FragmentMetadata` / `KzgFragments` / `ProofRecords` が削除される
- `ForgottenByPolicy` event が emit される
- 既に存在しない `content_hash` を渡しても panic しない (idempotent)

### 9.6 統合テスト (`apps/blockchain/tests/integration/`)

`shell + PAPI` ベースで:

1. create_post → score = InitialScore
2. react(Like) × 数件 → score 上昇
3. 時間経過 (block 進行) → score decay
4. 反応なしで `LowPopularityThreshold` 割れ → marked
5. `GracePeriod` 経過 → 削除確認 (`Posts` / `ContentRefs` 消失、`PostDeleted` event)
6. `do_release_fragment` の効果として `KzgFragments` も消失

### 9.7 Runtime API テスト

- `get_effective_score` がリアルタイムの decay 適用済み値を返す
- `get_net_count` が `like_count - dislike_count` を i64 で返す
- 存在しない post_id で None

## 10. 実装フェーズ分割

`superpowers:writing-plans` で詳細化される際の目安。

| Phase | 内容 | 完了基準 |
|-------|------|---------|
| **P0** | Boost 削除 (pallet-reaction + 波及) | 既存 `cargo test` が新 ReactionType (Like/Bad) で全て pass、frontend ビルドが通る |
| **P1** | pallet-popularity スケルトン + decay + on_post_created + on_reaction (storage 書き込みのみ) | `cargo test -p pallet-popularity` のスコア更新系テストが pass |
| **P2** | on_finalize bounded scan + mark/unmark/hysteresis | mark / unmark / flapping 防止のテストが pass |
| **P3** | GracePeriod + bounded deletion + StorageInterface 拡張 + PostMutator | 削除フローのテストが pass、integration test の create→react→decay→delete シナリオ通過 |
| **P4** | Runtime API + chain spec への定数反映 | `get_effective_score` / `get_net_count` の RPC 経由動作確認 |
| **P5** (out-of-scope) | frontend UI (人気度バッジ / 削除予告 / ランキング) | 別 spec |

各フェーズ完了で `cargo test --workspace` がグリーン、Phase 3 完了で integration test scenario 通過、を完了基準とする。「動作確認なし」は CLAUDE.md AI Agent Rules #1 で禁止。

## 11. 既知の検討事項 / 未決事項

| 項目 | 状態 |
|------|------|
| 暫定値のチューニング (InitialScore / DecayRate / Threshold / GracePeriod) | testnet で実測してから調整。v1 では固定値 |
| decay の最適化 (naive ループ vs pow_table) | 初版 naive、ベンチで問題が出たら pow_table 化 |
| storage node への削除通知 | `ForgottenByPolicy` event を pallet-storage が emit、storage node 側の event subscribe 実装は別 PR |
| 削除済 post への late reaction | `on_reaction` 内で `PostScores::contains_key(post_id)` をチェックし、なければ no-op (pallet-reaction 側の `react()` でも `pallet-post::Posts::contains_key` を見るので二重 guard) |
| 永続化オプション (有料で削除対象外) | v2 別 spec |

## 12. CLAUDE.md 互換性ポリシーへの準拠

- 既存 chain state (Boost を含む `ReactionStats`、`pallet-popularity` 不在の状態) は破棄して dev chain 再生成
- `StorageVersion` migration / 互換 shim は書かない
- frontend の IndexedDB / localStorage に Boost 由来データがあれば wipe 前提で対応

---

**承認後**: `superpowers:writing-plans` を呼び出して、各 Phase の詳細実装計画 (タスク分解 + TDD のテストファースト順序) を作成する。
