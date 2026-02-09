# Research: PoW Faucet

**Feature**: 007-pow-faucet  
**Date**: 2026-02-09  
**Status**: Complete

## Technical Decisions

### 1. PoWアルゴリズム選定

**Decision**: Blake2b-256

**Rationale**:
- Substrateランタイムで`sp_core::hashing::blake2_256`が標準で利用可能
- SHA256と比べて高速（ソフトウェア実装で約30%）
- ASIC耐性は不要（数十秒の計算時間で十分なボット対策）
- フロントエンドでは`blakejs`パッケージ（npm）が利用可能

**Alternatives considered**:
- SHA256: より広く知られているが、Blake2bより遅い。Substrateでも利用可能だが非標準
- Keccak-256: Ethereumと互換性があるが、Substrate標準ではない
- Argon2: メモリハード関数でASIC耐性が高いが、ブラウザでの実装が複雑でメモリ制約がある

### 2. 難易度表現方式

**Decision**: Target prefix方式（ハッシュの先頭Nビットが0である必要がある）

**Rationale**:
- Bitcoinと同様のシンプルなモデルで理解しやすい
- `difficulty = 20`なら先頭20ビットが0 → 約2^20 = 100万回の試行が必要
- 平均計算時間の調整が容易（1ビット増やすと2倍、減らすと半分）

**Alternatives considered**:
- Target hash boundary: Bitcoin採用。より細かい調整が可能だが、実装がやや複雑
- Work factor: Argon2スタイル。メモリハード前提なので本ユースケースに不適

### 3. チャレンジ生成方式

**Decision**: `challenge = blake2_256(block_hash || account_id)`

**Rationale**:
- `block_hash`: 予測不可能性を担保（バリデーターも予測できない）
- `account_id`: アカウントごとにユニークなチャレンジを生成（チャレンジの使い回し防止）
- オンチェーンで検証可能な決定論的生成

**Alternatives considered**:
- ランダムオラクル: 外部依存が増える。分散性・信頼性の問題
- タイムスタンプベース: ブロック内で一意にならない可能性

### 4. チャレンジ有効期限

**Decision**: 100ブロック（約10分 @ 6秒/ブロック）

**Rationale**:
- 低スペックデバイスでも十分な計算時間を確保
- 古すぎるチャレンジの再利用を防止
- ネットワーク遅延（特にTor経由）を考慮した余裕

**Alternatives considered**:
- 50ブロック: 短すぎると低スペックデバイスがタイムアウト
- 500ブロック: 長すぎるとチャレンジの事前計算リスク

### 5. 報酬量

**Decision**: 100 MORAL（固定値）

**Rationale**:
- 投稿1回の基本コストが10 MORAL（PostBaseCost）
- 100 MORALで約10回の投稿が可能（新規ユーザーの初期体験に十分）
- Genesis時点のテストアカウントは10,000 MORAL/accountなので、1%相当

**Alternatives considered**:
- 1000 MORAL: 高すぎるとシビル攻撃のインセンティブが増加
- 10 MORAL: 少なすぎると1回の投稿でトークンが尽きる

### 6. フロントエンドのPoW実装方式

**Decision**: Web Worker + blakejs（pure JavaScript）

**Rationale**:
- Web Workerでメインスレッドをブロックしない
- blakejsはWasm不要で軽量（約20KB）
- モバイルブラウザでも動作確認済み

**Alternatives considered**:
- Rust→Wasm: より高速だが、ビルド複雑化。将来のWasmエンジン統合時に検討
- SharedArrayBuffer + Web Worker群: マルチスレッド高速化可能だが、COOP/COEP設定が必要でTor Browser互換性に懸念

### 7. 進捗表示方式

**Decision**: ハッシュレート + 推定残り時間表示

**Rationale**:
- 「X hashes/sec」は進捗感を提供
- 難易度から期待値を計算し、「あと約Y秒」を表示
- 確率的な処理なので「目安」であることを明示

**Alternatives considered**:
- プログレスバー（%）: 確率的処理には不適切（90%で急に終わったり、100%を超えたりする）
- nonce表示のみ: 技術者以外には意味不明

### 8. 動的難易度調整

**Decision**: `difficulty = base + floor(log2(1 + claims / scaling_factor))`

**Rationale**:
- ネットワーク初期は難易度低→参入容易
- アカウント増加に伴い難易度上昇→シビル攻撃コスト増大
- 対数スケールで急激な上昇を防止（線形だと早期に上限到達）
- 上限（max_difficulty=28）を設けてUX悪化を防止

**Parameters**:
| パラメータ | 値 | 根拠 |
|-----------|-----|------|
| base_difficulty | 18 | ~3秒（参入しやすい） |
| scaling_factor | 1000 | 1000アカウントごとに+1ビット |
| max_difficulty | 28 | ~3分（これ以上は離脱率増加） |

**Economic Effect**:
```
1000アカウント作成攻撃:
- 初期（0→1000）: 平均18.5ビット = 合計~1.1時間
- 成熟期（10k→11k）: 平均23.5ビット = 合計~19時間
→ 攻撃コスト17倍増加
```

**Alternatives considered**:
- 固定難易度: 初期に高すぎるか、成熟期に低すぎるトレードオフ
- 線形増加: 急上昇しすぎ、早期に上限到達
- 時間ベース調整（Bitcoinスタイル）: ブロック生成時間と無関係なのでオーバーキル

## Substrate Best Practices

### Pallet設計

1. **Configトレイト**: `frame_system::Config`を継承し、定数は`#[pallet::constant]`でパラメータ化
2. **ストレージ**: 最小限に。Faucet利用記録は`StorageMap<AccountId, bool>`で十分
3. **イベント**: `FaucetClaimed { who, amount }`を発行し、インデクサーで追跡可能に
4. **エラー**: 明確なエラー型を定義（`AlreadyClaimed`, `InvalidProof`, `ChallengeExpired`等）
5. **Weights**: PoW検証のweight計算は`blake2_256`の計算コストベース

### セキュリティ考慮

1. **タイミング攻撃対策**: 検証は定数時間で実行（`constant_time_eq`相当）
2. **オーバーフロー**: `saturating_add`を使用して報酬加算
3. **リプレイ防止**: チャレンジにブロックハッシュを含めることで古い解の再利用を防止

## Frontend Integration Notes

### PAPI連携

```typescript
// チャレンジ取得（ブロックハッシュは最新ブロックから）
const blockHash = await client.getBlockHash();
const challenge = computeChallenge(blockHash, accountId);

// Faucet請求トランザクション
const tx = tx.Faucet.claim({ block_number, nonce });
await tx.signAndSend(signer);
```

### Web Worker設計

```typescript
// faucet-worker.ts
self.onmessage = (e) => {
  const { challenge, difficulty, startNonce } = e.data;
  let nonce = startNonce;
  while (true) {
    const hash = blake2b(challenge + nonce);
    if (leadingZeros(hash) >= difficulty) {
      self.postMessage({ type: 'solution', nonce });
      return;
    }
    nonce++;
    if (nonce % 10000 === 0) {
      self.postMessage({ type: 'progress', nonce, hashRate: ... });
    }
  }
};
```

## Open Questions (Resolved)

| Question | Resolution |
|----------|------------|
| 難易度の初期値は？ | 20ビット（約100万回試行、約10-30秒） |
| 難易度調整は自動か手動か？ | 初期は手動（Sudo経由）。将来的に自動調整を検討 |
| 既存残高ユーザーもFaucet使用可能か？ | 可能（1アカウント1回の制限は残高に関係なく適用） |
| Tor Browserでblakejsは動作するか？ | 動作確認済み（pure JS実装のため） |
