# ストレージ戦略: 地図と宝の分離

> **ステータス**: 構想 (未実装)  
> **関連ドキュメント**: [architecture.md](architecture.md), [memo.md](memo.md), [TODO.md](TODO.md)

---

## 1. 現状の問題

### 1.1 オンチェーンストレージの限界

現在の `pallet-post` は投稿本文を直接オンチェーンに保存している：

```rust
#[pallet::storage]
pub type Posts<T: Config> = StorageMap<_, Blake2_128Concat, PostId, Post<T>>;

pub struct Post<T: Config> {
    pub author: T::AccountId,
    pub content: BoundedVec<u8, T::MaxContentLength>,  // ← 本文がここに
    pub created_at: BlockNumberFor<T>,
}
```

**問題点:**
- ストレージ容量に上限がない（スパム攻撃に脆弱）
- バリデーターが全投稿本文を保持（プライバシーリスク）
- 「忘れられる権利」を実装不可能（ブロックチェーンは不変）
- 法的要求に対してバリデーターが本文を提出可能

---

## 2. 設計思想: 地図と宝

### 2.1 比喩

```
┌─────────────────────────────────────────────────────────────┐
│                     バリデーター（脳）                        │
│                                                             │
│   「宝の地図」だけを保持                                      │
│   - 投稿ID                                                   │
│   - 作成者ハッシュ                                            │
│   - 断片ロケーション（どのストレージノードが持つか）            │
│   - 報酬配分ロジック                                          │
│                                                             │
│   本文は一切持たない → 法的要求に「提出不能」                  │
└─────────────────────────────────────────────────────────────┘
                              │
                              │ 参照
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                   ストレージノード（体）                      │
│                                                             │
│   「宝」＝暗号化された断片を保持                               │
│   - SSS で分割された断片                                      │
│   - 単体では意味をなさない                                    │
│   - 報酬と引き換えに保持を継続                                │
│                                                             │
│   復号鍵を持たない → 内容を知ることが不可能                    │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 原則

1. **バリデーターは本文を持たない**: メタデータ（地図）のみ
2. **ストレージノードは鍵を持たない**: 暗号化断片（宝）のみ
3. **クライアントだけが完全な情報を持つ**: 暗号化・復号はすべてローカル

---

## 3. 技術構成

### 3.1 SSS（Shamir's Secret Sharing）

投稿本文を数学的に断片化：

```
原文 "Hello, World!"
    │
    ▼ クライアント側で暗号化
暗号文 (ChaCha20-Poly1305)
    │
    ▼ SSS で分割 (k=3, n=5)
┌───┬───┬───┬───┬───┐
│ S1│ S2│ S3│ S4│ S5│  ← 5つの断片
└───┴───┴───┴───┴───┘
  │   │   │   │   │
  ▼   ▼   ▼   ▼   ▼
 Node Node Node Node Node
  A    B    C    D    E

復元には任意の3断片が必要
```

**パラメータ:**
- `n` (総断片数): 保存先ノード数
- `k` (閾値): 復元に必要な最小断片数
- 推奨: `k = ceil(n * 0.6)` (過半数 + α)

### 3.2 Proof of Spacetime

ストレージノードが断片を実際に保持していることを検証：

```rust
// ストレージノードは定期的に証明を提出
fn prove_storage(
    fragment_id: FragmentId,
    challenge: Challenge,
    proof: SpacetimeProof,
) -> Result<(), Error>;
```

証明に失敗したノード → 報酬停止 → 断片を他ノードに再配布

### 3.3 経済的忘却

```
投稿の寿命 = Σ(ステーク報酬) - Σ(ストレージコスト)

報酬 > コスト → 断片は保持され続ける
報酬 < コスト → 断片は「忘却」される
```

誰も注目しない（リアクションがない）投稿 → 経済的に消滅

---

## 4. データフロー

### 4.1 投稿作成

```
1. ユーザーが投稿を作成
2. クライアント (Wasm) が暗号化
3. クライアントが SSS で断片化
4. 各断片をストレージノードに送信
5. メタデータをチェーンに登録 (Storage Pallet)
6. $moral を支払い
```

### 4.2 投稿閲覧

```
1. チェーンからメタデータを取得
2. 断片ロケーションを確認
3. k 個以上の断片をストレージノードから取得
4. SSS で復元
5. クライアントで復号
6. 表示
```

### 4.3 投稿削除（経済的忘却）

```
1. 投稿者がストレージ報酬の支払いを停止
2. ストレージノードにとって保持が赤字に
3. 断片を破棄（経済的に合理的な選択）
4. k 個未満になると復元不可能に
5. 投稿は実質的に「忘却」される
```

---

## 5. 否認可能性（Plausible Deniability）

### 5.1 法的要求への対応

```
当局: 「投稿 #12345 の本文を提出せよ」

バリデーター: 「我々はメタデータ（断片の場所）しか持っていません」

ストレージノード: 「我々は暗号化された断片を持っていますが、
                   復号鍵がないため内容は不明です」

クライアント: 「?」（オフライン / 匿名 / 複数国に分散）
```

### 5.2 技術的担保

- バリデーターのストレージに本文が存在しないことは、ランタイムコードで検証可能
- ストレージノードが保持するのは暗号化断片のみ（復号鍵なし）
- クライアントは Tor 経由で匿名

---

## 6. 実装フェーズ

### Phase 1: Storage Pallet（チェーン側）

```rust
// pallets/storage/src/lib.rs
#[pallet::storage]
pub type Fragments<T: Config> = StorageMap<_, 
    Blake2_128Concat, 
    FragmentId, 
    FragmentMetadata<T>
>;

pub struct FragmentMetadata<T: Config> {
    pub post_id: PostId,
    pub shard_index: u8,
    pub storage_node: T::AccountId,
    pub proof_deadline: BlockNumberFor<T>,
    pub reward_per_block: Balance,
}
```

**タスク:**
- [ ] Storage Pallet 作成
- [ ] FragmentMetadata 構造体
- [ ] register_fragment エクストリンシック
- [ ] prove_storage エクストリンシック
- [ ] slash_missing_proof ロジック

### Phase 2: Storage Node Daemon

```
storage-node-daemon/
├── src/
│   ├── main.rs
│   ├── storage/       # 断片の保存・取得
│   ├── proof/         # Proof of Spacetime 生成
│   ├── network/       # libp2p + Tor 通信
│   └── rpc/           # クライアント向け API
└── Cargo.toml
```

**タスク:**
- [ ] daemon 骨格
- [ ] 断片ストレージ（RocksDB）
- [ ] Proof of Spacetime 実装
- [ ] libp2p 統合
- [ ] Tor Hidden Service 対応

### Phase 3: Client Wasm Engine

```typescript
// frontend/src/lib/storage-engine.ts
import init, { encrypt, decrypt, sss_split, sss_combine } from 'anarchy-wasm';

export async function createPost(content: string): Promise<PostMetadata> {
    const encrypted = await encrypt(content, key);
    const shards = await sss_split(encrypted, { n: 5, k: 3 });
    // ... ストレージノードに送信
}
```

**タスク:**
- [ ] Rust → Wasm ビルド設定
- [ ] ChaCha20-Poly1305 暗号化
- [ ] SSS 分割・結合
- [ ] TypeScript バインディング

### Phase 4: Post Pallet リファクタ

現在の `pallet-post` を改修：

```rust
// Before
pub content: BoundedVec<u8, T::MaxContentLength>,

// After
pub fragment_root: Hash,  // Merkle root of fragments
pub shard_count: u8,
pub threshold: u8,
```

**タスク:**
- [ ] Post 構造体の変更
- [ ] マイグレーション
- [ ] Storage Pallet との連携

---

## 7. 技術スタック

| レイヤー | 技術 | 役割 |
|---------|------|------|
| **Chain** | Substrate / Storage Pallet | メタデータ管理、報酬計算 |
| **Storage** | RocksDB / Custom Daemon | 断片保存、Proof 生成 |
| **Crypto** | ChaCha20-Poly1305, SSS | 暗号化、断片化 |
| **Network** | libp2p + Tor | 匿名通信 |
| **Client** | Wasm (Rust → wasm-bindgen) | ローカル暗号処理 |

---

## 8. 関連する未実装機能

TODO.md から抜粋：

- [ ] X25519 鍵交換
- [ ] スキャン鍵/閲覧鍵ペア生成
- [ ] ChaCha20-Poly1305 暗号化
- [ ] 鍵導出（HKDF）
- [ ] エフェメラル公開鍵の格納
- [ ] 復号・表示 UI

---

## 9. 参考文献

- [Shamir's Secret Sharing](https://en.wikipedia.org/wiki/Shamir%27s_Secret_Sharing)
- [Proof of Spacetime (Filecoin)](https://spec.filecoin.io/algorithms/pos/)
- [ChaCha20-Poly1305 (RFC 8439)](https://datatracker.ietf.org/doc/html/rfc8439)
- [Stealth Addresses](https://vitalik.eth.limo/general/2023/01/20/stealth.html)

---

## 10. 実現可能性の評価

> **結論: 技術的には可能だが、ストレージレイヤーはボス級の難易度**

特に **Proof of Spacetime (PoST)** と **自己修復プロトコル** は、Filecoin や Arweave といった巨大プロジェクトが数年かけて磨き上げた領域。Substrate で一から組むのはかなりエキサイティングな挑戦になる。

### 10.1 Storage Pallet & Node（難易度: 鬼）

Anarchy プロトコルの「体」になる部分。

| 機能 | 実現可能性 | 懸念点・アドバイス |
|------|------------|-------------------|
| **PoST 検証** | 可能 | オンチェーンでの検証は計算コストが高いため、**Off-chain Workers (OCW)** を使って検証の重い部分を逃がす設計が必須 |
| **自然な忘却** | **最高** | 哲学的に面白い。報酬（$moral）が切れたらデータが消えるのは、「価値のない情報は消え、誰かが愛でる情報だけが残る」というアナーキーな記憶の形 |
| **libp2p 受信** | 可能 | Tor 経由での大容量データ転送はノードの負荷が高い。ストレージ専用の `libp2p` 接続制限を設けないと、バリデーターの合意形成を邪魔するリスクあり |

### 10.2 PoW Faucet（難易度: 低〜中）

今すぐにでも実装できる **「クイックウィン」** な機能。

- **ボット対策**: 数十秒の PoW は、ユーザー体験を損なわずにシビル攻撃（大量のアカウント作成）を防ぐのに有効
- **匿名性の担保**: IP 制限をかけずに「計算量」だけで初期トークンを配るのは、Tor 前提の Anarchy と相性抜群
- **技術構成**: Pallet 側で `nonce` と `difficulty` を管理し、フロントエンドの Web Worker で `hash` を回す

### 10.3 自己修復プロトコルの罠

k-of-n（消失訂正符号）を使った再配布は、理論は完璧だが実装でハマりやすいポイントがある。

> **「ノードが 1 台死ぬたびに全ネットワークで再配布が走ったら、トラフィックが爆発してパンクする」**

これを防ぐには、**「閾値を下回った時だけ、インセンティブ付きで再配布を募集する」** という、スローで自律的なトリガーにする必要がある。

---

## 11. 推奨実装順序

一気に全部作ろうとするとコードの海に溺れるため、以下の順序で進めることを推奨：

```
Phase 0: PoW Faucet
    │
    │ アカウントを作れるようにする
    ▼
Phase 1: Simple Storage
    │
    │ 報酬なしで、まずはデータを断片化して置いておけるだけの仕組み
    ▼
Phase 2: Moral Incentive
    │
    │ PoST と $moral の分配を組み込む
    ▼
Phase 3: Self-Healing
    │
    │ 自己修復プロトコル
    ▼
完成: 真に不滅の SNS
```

### 比喩

> **「ストレージをブロックチェーンに持たせるのは、宇宙船に巨大な倉庫を連結して飛ばすようなもの。重力（ステート肥大化）との戦いになるが、成功すれば Vercel も AWS もいらない『真に不滅の SNS』が誕生する。」**

---

## 12. 次のアクション候補

1. **PoW Faucet の Pallet 定義（Rust）** - 手堅くフロントエンドとの統合も面白い
2. **「自然な忘却」のためのデータ寿命（Rent）計算式** - 経済モデルの設計
3. **Off-chain Workers (OCW) での PoST 検証設計** - 重い処理をオフチェーンに逃がす
