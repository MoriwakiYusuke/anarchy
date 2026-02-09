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
