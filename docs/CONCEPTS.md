# Anarchy 構想事項（検討中）

> **ステータス**: 構想・検討段階  
> **関連ドキュメント**: [TODO.md](TODO.md), [architecture.md](architecture.md), [StorageStrategy.md](StorageStrategy.md)

以下は将来的に実装を検討している機能。優先度・実現可能性は未確定。

---

## 経済設計（トークノミクス）

**背景**:
現在のAnarchyはバリデーター報酬が存在しない。開発段階では問題ないが、
本番環境ではバリデーターにインセンティブがないとネットワークが維持できない。

**現状**:
- TX手数料: 完全無料（`WeightToFee=0`, `LengthToFee=0`）
- 投稿コスト: $moral burn（バリデーターには行かない）
- バリデーター報酬: なし
- Faucet報酬: 100 MORAL（新規mint）

**選択肢**:

| 案 | TX手数料 | 投稿コスト | バリデーター報酬 | 備考 |
|----|----------|------------|------------------|------|
| **A: 現状維持 + ブロック報酬** | 0 | burn | ブロック生成時にmint | シンプル、インフレ |
| **D: Ethereum方式** | Base→burn, Tip→バリデーター | burn | Base Fee + Tip | バランス良、複雑 |

**案A詳細（シンプル）**:
```
TX手数料:       0（無料）
投稿コスト:     burn（デフレ圧力）
ブロック報酬:   X MORAL/block を新規mint → バリデーターへ
Faucet:         unsigned tx のまま（影響なし）
```

**案D詳細（Ethereum EIP-1559方式）**:
```
Base Fee:       動的計算 → burn（デフレ圧力）
Priority Fee:   ユーザー任意 → バリデーターへ（インセンティブ）
投稿コスト:     burn（追加のデフレ圧力）
Faucet:         unsigned tx のまま（影響なし）
```

**検討事項**:
- インフレ率とデフレ圧力のバランス
- バリデーター数の想定（PoA? NPoS?）
- pallet_staking 導入の是非
- Treasury の必要性

**暫定方針**:
- 開発〜テストネット: 現状維持（報酬なし）
- メインネット: 案A or 案D を選択して実装

---

## コンセンサス方式の検討（PoA → PoW）

**背景**:
現在のAnarchyは Aura/GRANDPA（PoA）を使用。バリデーターは許可制で、
chain specに公開鍵が登録されたノードのみがブロック生成可能。
「匿名・分散」を掲げるAnarchyとしては、誰でもマイニング参加できるPoWが理想的。

**現状**:
- コンセンサス: Aura（ブロック生成）+ GRANDPA（ファイナリティ）
- バリデーター: 許可制（well-known keys: Alice, Bob等）
- ブロック時間: 6秒固定

**PoW移行時の変更点**:
| コンポーネント | 現在（PoA） | PoW移行後 |
|---|---|---|
| ブロック生成 | Aura（ラウンドロビン） | sha3pow/RandomX |
| ファイナリティ | GRANDPA（即時） | 確率的（longest chain） |
| 参加条件 | 許可制 | **誰でも参加可能** |
| 電力消費 | 低 | 高 |
| ブロック時間 | 6秒固定 | 難易度調整で変動 |

**PoWアルゴリズム選択肢**:
| アルゴリズム | 特徴 | ASIC耐性 |
|---|---|---|
| sha3pow | シンプル、実装例多い | 低 |
| RandomX | Monero採用、CPU向け | 高 |
| Ethash | Ethereum旧PoW | 中 |

**Hybrid案（NPoS）**:
- $moralをステークして誰でもバリデーター候補に
- pallet_staking / pallet_election_provider 導入
- Polkadot/Kusamaと同じ方式

**暫定方針**:
- 開発〜テストネット初期: PoA維持（安定性重視）
- テストネット後期: PoW or NPoS をテスト
- メインネット: PoW / NPoS のいずれかを採用（ハードフォーク）

---

## ブラウザ拡張ウォレット連携

**背景**: 
現在の実装ではフロントエンドにシードフレーズを直接入力するため、
悪意あるフロントに秘密鍵を抜かれるリスクがある。
WebAuthnなら秘密鍵はハードウェアから出なかったが、廃止により保護が弱まった。

**解決策案**:
- Polkadot.js Extension / Talisman / SubWallet 等と連携
- シードフレーズはウォレット内に保存（フロントに渡さない）
- フロントエンドは署名リクエストのみ送信、ユーザーがウォレットUIで承認
- PAPIは `@polkadot-api/pjs-signer` で拡張ウォレットと連携可能

**暫定対応**:
- 現在のシードフレーズ入力は「開発用 / SBOM検証済みフロント専用」として運用
- 本番環境ではウォレット拡張連携を必須とする予定

**備考**: ハイドラ戦略（複数フロントエンド運営者）を維持するための前提条件

---

## オンチェーンガバナンス

**背景**:
現在のランタイムアップグレードは `pallet_sudo` 経由でSudoキー保有者（開発時はAlice）のみが実行可能。
「支配なき秩序」を掲げるAnarchyとしては、本番環境での中央集権的な管理は矛盾する。

**現状**:
- `pallet_sudo` のみ（単一管理者）
- `System.set_code(new_wasm)` でランタイム書き換え

**選択肢**:
| 方式 | 説明 | 複雑度 |
|------|------|--------|
| Sudo維持 | 単一管理者（開発用） | 低 |
| Multisig | 複数署名で承認 | 中 |
| Democracy | トークン投票で決定 | 高 |
| OpenGov | Track別の投票システム | 最高 |

**検討事項**:
- $moral保有量ベースの投票権は、経済的攻撃（買い占め）のリスク
- Conviction voting（ロック期間に応じた投票力増加）の導入
- 技術的提案とコミュニティ提案の分離
- 緊急時の対応（セキュリティパッチ等）

**暫定方針**:
- 開発〜テストネット: Sudo維持
- メインネット初期: Multisig（信頼できるコア開発者数名）
- メインネット安定後: Democracy/OpenGovへ移行

---

## 残高保護機能（Keep Alive強制）

**背景**:
ストレージ報酬を払い続けるためには、アカウントに最低限の残高が必要。
うっかり全額送金してストレージ報酬が払えなくなると、投稿データが「忘却」される。

**構想**:

| 操作 | 関数 | 動作 | 備考 |
|------|------|------|------|
| **普段の送金** | `transfer` | 残高が100 MORALを切るなら**失敗** | Keep Alive強制 |
| **脱退・全額出金** | `close_account` | 残高を0にしてアカウント**削除** | 専用ボタン |

**実装案**:
```rust
// 通常のtransferをオーバーライド
fn transfer(origin, dest, amount) {
    let who = ensure_signed(origin)?;
    let balance_after = T::Currency::free_balance(&who)
        .saturating_sub(amount);
    
    // 100 MORAL以上を維持（ストレージ報酬用）
    ensure!(balance_after >= T::MinimumBalance::get(), Error::<T>::KeepAlive);
    
    T::Currency::transfer(&who, &dest, amount, KeepAlive)?;
}

// 明示的な脱退（全額出金 + アカウント削除）
fn close_account(origin, dest) {
    let who = ensure_signed(origin)?;
    let balance = T::Currency::free_balance(&who);
    
    // 全額送金（アカウント削除を許可）
    T::Currency::transfer(&who, &dest, balance, AllowDeath)?;
    
    // TODO: ストレージ報酬の停止処理
    // TODO: 投稿データの「忘却」トリガー
}
```

**UI設計**:
- 送金画面: 「残高が100 MORALを下回る送金はできません」警告
- プロフィール画面: 「アカウントを閉じる」ボタン（確認ダイアログ付き）

**検討事項**:
- 100 MORALの根拠（ストレージ報酬コストに連動すべき？）
- `close_account`後のストレージ報酬停止タイミング
- 「忘却」までの猶予期間の設定

**暫定方針**:
- 開発〜テストネット: 未実装（自由に送金可能）
- メインネット: 分散ストレージ導入後に実装

---

## 投稿人気度システム

**背景**:
分散ストレージのコスト効率化のため、需要のない投稿は自然淘汰される仕組みが必要。
ユーザーの評価行動に基づいて人気度を計算し、一定値を下回った投稿は削除対象とする。

**人気度スコアの計算**:

| アクション | スコア変動 | 備考 |
|------------|-----------|------|
| 高評価（Like） | +N | 重み付け調整可能 |
| 低評価（Dislike） | +M | 低評価も「関心」として加点 |
| 時間経過 | -decay | 減衰関数で徐々に減少 |

**フェッチ（閲覧）スコアは採用しない** (2026-05-03 決定):
- **Sybil 攻撃に脆弱**: 攻撃者が自分の post を反復取得して人気度を水増し可能
- **匿名性と矛盾**: Tor 強制下では IP/identity ベースの dedup ができない。閲覧をオンチェーン化すると "誰が何を読んだか" が事実上トラッキング可能になる
- **処理リソース**: fetch ごとに storage node → chain への report 経路が必要 = validator 負荷増 + state bloat + 新しい trust boundary
→ react (Like/Boost/Bad) のオンチェーンカウントだけで人気度を回す。

**減衰方式の選択肢**:

| 方式 | 説明 | 特徴 |
|------|------|------|
| **絶対減衰** | `score -= fixed_amount / block` | 古い投稿は確実に消える |
| **相対減衰** | `score *= decay_rate` (例: 0.99/day) | 人気投稿は長期間残る |
| **ランキング相対** | 下位N%を削除対象 | ストレージ容量に連動 |

**削除フロー**:
```
1. ブロック生成時に人気度を更新（減衰適用）
2. 閾値（例: score < 10）を下回った投稿をマーク
3. 猶予期間（例: 7日）経過後、ストレージノードに削除指示
4. オンチェーンのメタデータも削除
```

**オンチェーン保存データ**:
```rust
pub struct PostPopularity<BlockNumber> {
    pub score: u64,
    pub last_interaction: BlockNumber,
    pub like_count: u32,
    pub dislike_count: u32,
    // fetch_count は採用しない (Sybil / 匿名性 / 処理リソースの観点で却下、上記参照)
}
```

**検討事項**:
- 初期スコアの設定（投稿時に付与する基礎スコア）
- 減衰率のパラメータ調整（ガバナンスで変更可能にすべきか）
- Sybil攻撃対策（自演でスコアを上げる行為の防止）
- 「永続化」オプション（追加料金で削除対象外にする機能）

**暫定方針**:
- 開発〜テストネット: 未実装（投稿は永続）
- メインネット: 分散ストレージ導入と同時に実装

---

## フロントエンド報酬システム

**背景**:
ハイドラ戦略（複数フロントエンド運営者）を維持するためには、
フロントエンド運営者にも経済的インセンティブが必要。
投稿手数料の一部をフロントエンド運営者に還元する仕組みを導入する。

**仕組み**:
```
投稿手数料 (例: 10 MORAL)
    ├── X% → burn（デフレ圧力）
    └── Y% → フロントエンド運営者のアドレスへ
    ※ 比率は未定
```

**実装案**:

1. **フロントエンドに報酬アドレスを設定**
   - 各フロントエンドは自身の報酬受取アドレスを設定
   - 投稿時にそのアドレスを `frontend_address` としてExtrinsicに含める

2. **課題: クライアントサイド改ざん**
   - ブラウザの開発者ツールでJSを改変すれば、`frontend_address` を任意のアドレスに変更可能
   - .env等のブラウザの開発者ツールからアクセスできない場所にアドレスを配置
   - 自分でフロントを起動するユーザーは自由に書き換えを許容

2. **Pallet側の処理**
   ```rust
   fn create_post(
       origin,
       content_hash: H256,
       frontend_address: Option<AccountId>,
   ) {
       let who = ensure_signed(origin)?;
       let fee = T::PostFee::get();
       
       // burn分（比率は未定）
       let burn_amount = fee * T::BurnRatio::get() / 100;
       T::Currency::withdraw(&who, burn_amount, WithdrawReasons::FEE, ExistenceRequirement::KeepAlive)?;
       
       // フロントエンド報酬分
       if let Some(frontend) = frontend_address {
           let reward = fee - burn_amount;
           T::Currency::transfer(&who, &frontend, reward, KeepAlive)?;
       } else {
           // アドレス未指定の場合は全額burn
           T::Currency::withdraw(&who, fee - burn_amount, ...)?;
       }
       
       // 投稿処理...
   }
   ```

**報酬比率の選択肢**（未定）:

| burn率 | フロント報酬率 | 備考 |
|--------|---------------|------|
| 100% | 0% | 現状（フロント報酬なし） |
| 80% | 20% | バランス型 |
| 50% | 50% | フロント重視 |
| 可変 | 可変 | ガバナンスで調整 |

※ 最適な比率は運用データを見て決定する

**セキュリティ考慮**:
- フロントエンドが任意のアドレスを指定できるため、悪意あるフロントが全額を自分に送る可能性
- → ユーザーは信頼できるフロントエンドを選ぶ責任がある（ハイドラ戦略の前提）
- → SBOMやソースコード公開で透明性を担保

**検討事項**:
- 報酬比率のデフォルト値
- `frontend_address` が未指定の場合の処理（全額burn? Treasury?）
- フロントエンド登録制度の是非（オンチェーンで「公認フロント」を管理）
- 不正フロントの通報・ブラックリスト機能

**暫定方針**:
- 開発〜テストネット: 未実装（全額burn）
- メインネット初期: ハードコード方式で導入
- メインネット安定後: ガバナンスによる比率調整機能を追加

---

## ファイル配信機能

**背景**:
現在の投稿はテキストのみ対応。画像・動画・音声などのメディアファイルを投稿・配信できるようにする。

**対応ファイル形式**（案）:

| カテゴリ | 形式 | 備考 |
|----------|------|------|
| 画像 | JPEG, PNG, WebP, GIF | 一般的な形式 |
| 動画 | MP4, WebM | ブラウザ互換性重視 |
| 音声 | MP3, OGG, WAV | ポッドキャスト等 |
| その他 | PDF, ZIP等 | 必要に応じて追加 |

**アーキテクチャ**:
```
ユーザー → フロントエンド → ストレージノード（Tor経由）
                ↓
           オンチェーン（メタデータのみ）
           - content_hash (ファイルのハッシュ)
           - file_type (MIME type)
           - file_size
           - storage_cid (ストレージ参照ID)
```

**オンチェーン保存データ**:
```rust
pub struct FileMetadata {
    pub content_hash: H256,      // ファイル全体のハッシュ
    pub mime_type: BoundedVec<u8, MaxMimeLength>,  // "video/mp4" 等
    pub file_size: u64,          // バイト数
    pub storage_cid: BoundedVec<u8, MaxCidLength>, // ストレージ参照ID
}
```

**検討事項**:
- ファイルサイズ上限（ストレージコストとの兼ね合い）
- 投稿コストの計算（サイズに応じた従量課金?）
- サムネイル/プレビュー生成（フロントエンド側? ストレージノード側?）
- ストリーミング対応（大容量動画の分割配信）
- 不適切コンテンツへの対応（検閲なしの原則との整合性）

**暫定方針**:
- 開発〜テストネット: テキストのみ
- メインネット: 分散ストレージと同時に画像対応
- 将来: 動画・音声対応を順次追加
