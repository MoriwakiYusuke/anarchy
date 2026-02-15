# Research: KZG-VSS 保持証明・報酬システム

**Feature**: 011-kzg-proof-rewards  
**Date**: 2026-02-16  
**Status**: Completed

## 1. arkworks Wasm Compilation

### Decision
`ark-bls12-381`, `ark-poly`, `ark-poly-commit` を `wasm32-unknown-unknown` ターゲットでビルド。

### Rationale
- arkworksは公式にWasmをサポート（`default-features = false` + `std` feature flag）
- `wasm-pack` でブラウザ向けビルドが可能
- 既存の `packages/wasm-engine` がwasm-pack構成のため、統合が容易

### Alternatives Considered
| 選択肢 | 却下理由 |
|--------|---------|
| blst (rust-bindings) | C依存あり、Wasmビルドが複雑。arkworksの方がpure Rustで扱いやすい |
| noble-bls12-381 (JS) | TypeScript実装。パフォーマンスがRust/Wasmより劣る |
| substrate-bls12-381 | Substrate固有、フロントエンドで使用不可 |

### References
- [arkworks/algebra Wasm support](https://github.com/arkworks-rs/algebra#wasm)
- [wasm-pack book](https://rustwasm.github.io/docs/wasm-pack/)

---

## 2. Trusted Setup (SRS) Source

### Decision
Ethereum KZG Ceremony (EIP-4844) の Powers of Tau 成果物を使用。

### Rationale
- 数万人が参加した儀式で、1人でも正直なら安全（1-of-N trust assumption）
- EIP-4844と同じBLS12-381曲線を使用
- 公開済みのSRSファイルを再利用可能
- 独自儀式の実施は不要（コスト・信頼性の問題を回避）

### SRS Size Estimation
| 次数 (degree) | G1点数 | サイズ (圧縮) |
|---------------|--------|--------------|
| 1024 | 1025 | ~50KB |
| 4096 | 4097 | ~200KB |
| 32768 | 32769 | ~1.5MB |

**選択**: degree-4096（~200KB）。32KBデータを1024個の32バイトスカラーとしてエンコード可能。

### Alternatives Considered
| 選択肢 | 却下理由 |
|--------|---------|
| 独自KZG Ceremony | 参加者を集める困難さ、信頼性の問題 |
| Zcash Powers of Tau | BLS12-381ではなくBLS12-377を使用 |
| Transparent setup (FRI) | KZG commitmentの互換性がなくなる |

### References
- [Ethereum KZG Ceremony](https://ceremony.ethereum.org/)
- [c-kzg-4844](https://github.com/ethereum/c-kzg-4844) - SRSファイルフォーマット

---

## 3. BLS12-381 Pairing Performance

### Decision
オンチェーン検証は Off-chain Worker でバッチ処理。単発検証はランタイムで実行可能。

### Rationale
- BLS12-381ペアリング計算は重い（~2ms/pair on native, ~20ms on Wasm）
- Substrate Off-chain Workerで100件バッチ検証 → 結果をオンチェーンに報告
- 単発のKZG opening verification（1ペアリング）はランタイムで許容範囲

### Performance Benchmarks (estimated)
| 操作 | Native (ms) | Wasm (ms) |
|------|-------------|-----------|
| G1 scalar mul | 0.3 | 3 |
| Pairing | 2 | 20 |
| KZG verify (1 point) | 4 | 40 |
| Batch verify (100 points) | 200 | 2000 |

### Substrate Integration
```rust
// Off-chain Worker でバッチ検証
#[pallet::hooks]
impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
    fn offchain_worker(block_number: BlockNumberFor<T>) {
        // Pending proofs を取得し、バッチ検証
        // 結果を unsigned transaction で報告
    }
}
```

### Alternatives Considered
| 選択肢 | 却下理由 |
|--------|---------|
| 全てオンチェーン | ブロック時間超過のリスク |
| 全てオフチェーン | オンチェーン検証なしでは不正が検出できない |
| ZK-SNARK proof aggregation | 実装複雑度が高い。Phase 3で検討 |

---

## 4. Compression Algorithm

### Decision
gzip (flate2 crate) を使用。256バイト未満はスキップ。

### Rationale
- gzipは広くサポートされ、ブラウザ側でも `CompressionStream` API で対応
- flate2はWasmビルド可能（miniz_oxide backend）
- テキストコンテンツで30-70%の圧縮率が期待できる

### Alternatives Considered
| 選択肢 | 却下理由 |
|--------|---------|
| zstd | 圧縮率は高いが、ブラウザネイティブ非対応 |
| brotli | 圧縮率は高いが、リアルタイム圧縮が遅い |
| lz4 | 圧縮率が低い |

### Implementation
```rust
// Wasm Engine
pub fn compress(data: &[u8]) -> Vec<u8> {
    if data.len() < 256 {
        return data.to_vec(); // Skip compression
    }
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data)?;
    encoder.finish()?
}
```

---

## 5. KZG-VSS Polynomial Construction

### Decision
データを32バイトチャンクに分割し、各チャンクをBLS12-381スカラー体の元として解釈。これらを係数とする多項式を構成。

### Rationale
- BLS12-381のスカラー体は ~32バイト（253ビット）
- 多項式 f(x) = Σ(data_chunk[i] × x^i) を構成
- f(1), f(2), ..., f(n) がシェアとなる
- KZGコミットメント C = [f]₁ でコミット

### Data Encoding
```
Original data: [bytes]
    ↓ pad to 32-byte boundary
Chunks: [chunk₀, chunk₁, ..., chunk_{m-1}]
    ↓ interpret as scalars
Coefficients: [a₀, a₁, ..., a_{m-1}] ∈ Fr
    ↓ construct polynomial
f(x) = a₀ + a₁x + a₂x² + ... + a_{m-1}x^{m-1}
    ↓ evaluate at points 1..n
Shares: [f(1), f(2), ..., f(n)]
```

### Recovery (Lagrange Interpolation)
k個以上のシェア (i, f(i)) があれば、Lagrange補間で f(x) を復元し、係数からデータを再構成。

---

## 6. Score System Interface

### Decision
`trait ScoreProvider` を定義し、後にスコアシステムを実装する際にプラグイン可能にする。

### Rationale
- スコアシステムは本機能のスコープ外
- インターフェースのみ定義し、デフォルト実装では全投稿のスコア = 閾値以上とする
- 将来のスコアシステム実装時に `ScoreProvider` を差し替え

### Interface Design
```rust
pub trait ScoreProvider {
    /// 投稿のスコアを取得。None = スコア不明（デフォルトスコア使用）
    fn get_score(content_hash: H256) -> Option<u64>;
    
    /// スコア閾値を取得
    fn get_threshold() -> u64;
}

// デフォルト実装
impl ScoreProvider for () {
    fn get_score(_: H256) -> Option<u64> {
        None // → デフォルトスコア（閾値以上）として扱う
    }
    fn get_threshold() -> u64 {
        0 // 全投稿が報酬対象
    }
}
```

---

## 7. Reward Pool Economics

### Decision
投稿費用の90%を報酬プールに蓄積、10%をバーン。報酬額 = `base_reward_per_byte × data_size`。

### Rationale
- 投稿費用がストレージノードの報酬原資となる循環経済
- バーンによりデフレ圧力を維持
- データサイズ依存の報酬により、大きなデータの保持に見合ったインセンティブ

### Initial Parameters (Governance調整可能)
| パラメータ | 初期値 | 根拠 |
|-----------|--------|------|
| `REWARD_POOL_RATIO` | 90% | 運営コストとバランス |
| `BURN_RATIO` | 10% | デフレ圧力維持 |
| `BASE_REWARD_PER_BYTE` | 0.0001 MORAL | 1KB = 0.1 MORAL/day目安 |
| `SCORE_THRESHOLD` | 0 | デフォルトで全投稿報酬対象（スコアシステム実装前） |

---

## Summary of Decisions

| 領域 | 決定 | 主な理由 |
|------|------|---------|
| KZG実装 | arkworks | Pure Rust, Wasmサポート |
| Trusted Setup | Ethereum Ceremony | 1-of-N信頼、再利用可能 |
| SRS次数 | 4096 | 32KB対応、~200KB埋め込み |
| 圧縮 | gzip (flate2) | ブラウザ互換、Wasmビルド可能 |
| スコア | Trait interface | 後方互換のプラグイン設計 |
| 報酬分配 | 90%プール/10%バーン | 循環経済 + デフレ |
| データ分割 | ハイブリッド方式 | 大容量データ対応 |

---

## Hybrid Architecture Decision (2025-01)

### 背景: Pure KZG-VSSの限界

Pure KZG-VSSアプローチでは、データ全体を1つの多項式に埋め込む：
- 多項式次数 = (データスカラー数 - 1) = m - 1
- Lagrange補間による復元には **m個** のシェアが必要
- k-of-n (k < m) での復元は数学的に不可能

これは既存のSSS (sharks) と根本的に異なる：
- sharks: 各バイトが独立した次数 k-1 の多項式を持つ
- k-of-n での復元が常に可能（データサイズに依存しない）

### 採用: ハイブリッドアプローチ

```
[原文データ]
     ↓
[AES-256-GCM暗号化] ← K_post (32バイトランダム鍵)
     ↓
[暗号文]
     ↓
[Reed-Solomon k-of-n erasure coding]
     ↓
[n個のチャンク] + [各チャンクのKZGコミットメント]
     ↓ (配布)
[ストレージノード群]

[K_post]
     ↓
[SSS k-of-n 分割] (sharks既存実装を再利用)
     ↓
[n個の鍵シェア]
     ↓ (配布)
[ストレージノード群]
```

### セキュリティ分析

| 脅威 | 対策 | 状態 |
|------|------|------|
| k-1ノード共謀 | Reed-Solomonでk未満からの復元不可、鍵もk未満で復元不可 | ✅ |
| 量子計算 | AES-256はGroverで2^128相当（現時点で十分） | ⚠️ 将来注視 |
| 鍵漏洩 | 投稿ごとにランダム生成、投稿完了後に破棄 | ✅ |
| 署名鍵混同 | sk_user（署名用）は絶対に配布しない、K_postとは完全分離 | ✅ |

### 従来設計との比較

| 観点 | 従来 (SSS全体) | ハイブリッド | 差異 |
|------|---------------|-------------|------|
| 匿名性 | ✅ | ✅ | なし |
| 検閲耐性 | ✅ | ✅ | なし |
| 大容量対応 | △ (実用上限あり) | ✅ | 改善 |
| 検証可能性 | ✗ | ✅ (KZGコミットメント) | 改善 |
| フロントエンド | 平文が見える | 平文が見える | なし |

**結論**: セキュリティモデルに本質的な変更なし。検証可能性と大容量対応が向上。

### 実装計画

- **T022a**: `encryption.rs` - AES-256-GCM (aes-gcm crate)
- **T022b**: `reed_solomon.rs` - k-of-n erasure coding (reed-solomon-erasure crate)
- **T022c**: `key_sss.rs` - 32バイト鍵のSSS分割 (sharks既存利用)
- **T022d**: `hybrid.rs` - 統合API
- **T022e-g**: Wasmバインディング、テスト、フロントエンド統合
