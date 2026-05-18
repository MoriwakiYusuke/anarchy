# Wasm Engine KZG-VSS API Contract

**Package**: `anarchy-wasm-engine`  
**Version**: 2.0.0 (KZG-VSS)  
**Date**: 2026-02-16

## Overview

ブラウザ/Node.js環境で動作するWasmモジュール。KZG-VSSによるシェア生成・復元・証明を提供。

---

## Functions

### vss_split

データをKZG-VSSでシェアに分割する。

```typescript
export function vss_split(
  data: Uint8Array,
  threshold: number,  // k
  share_count: number // n
): VssSplitResult;
```

**Parameters**:
| Name | Type | Range | Description |
|------|------|-------|-------------|
| `data` | `Uint8Array` | 1 byte - 32MB | 分割するデータ |
| `threshold` | `number` | 1 ≤ k ≤ n | 復元に必要な最小シェア数 |
| `share_count` | `number` | 2 ≤ n ≤ 255 | 生成するシェア数 |

**Returns**:
```typescript
interface VssSplitResult {
  /** KZGコミットメント (48 bytes) */
  commitment: Uint8Array;
  /** 生成されたシェア */
  shares: VssShare[];
  /** 各シェアのKZG proof (各48 bytes) */
  proofs: Uint8Array[];
  /** 圧縮が適用されたか */
  compressed: boolean;
  /** 複数セグメントに分割されたか (32KB超の場合) */
  multi_segment: boolean;
  /** セグメント数 (multi_segment=true の場合) */
  segment_count?: number;
}

interface VssShare {
  /** シェアインデックス (1..n) */
  index: number;
  /** シェア値 (32 bytes) */
  value: Uint8Array;
}
```

**Behavior**:
1. `data.length < 256` の場合、圧縮をスキップ
2. `data.length >= 256` の場合、gzip圧縮を適用
3. 圧縮後データを32バイトチャンクに分割
4. 各チャンクをBLS12-381スカラーとして解釈
5. 多項式 f(x) を構成（係数 = チャンク値）
6. KZGコミットメント C = Commit(f) を生成
7. f(1), f(2), ..., f(n) を評価しシェアを生成
8. 各シェアのKZG proofを生成

**Errors**:
| Error | Description |
|-------|-------------|
| `DataTooLarge` | 32MBを超えるデータ |
| `InvalidThreshold` | k > n または k < 1 |
| `SrsNotLoaded` | Trusted Setupが未初期化 |

---

### vss_recover

k個以上のシェアから元データを復元する。

```typescript
export function vss_recover(
  shares: VssShare[],
  threshold: number,
  compressed: boolean
): Uint8Array;
```

**Parameters**:
| Name | Type | Description |
|------|------|-------------|
| `shares` | `VssShare[]` | 復元に使用するシェア (≥ k個) |
| `threshold` | `number` | 復元閾値 (k) |
| `compressed` | `boolean` | 圧縮フラグ（split時の値を使用） |

**Returns**: 復元されたデータ

**Behavior**:
1. シェア数 ≥ k を検証
2. Lagrange補間で多項式 f(x) を復元
3. 係数から32バイトチャンクを抽出
4. `compressed=true` なら解凍
5. パディングを除去してデータを返却

**Errors**:
| Error | Description |
|-------|-------------|
| `InsufficientShares` | シェア数 < k |
| `InvalidShareIndex` | 重複インデックスまたは範囲外 |
| `DecompressionFailed` | 解凍に失敗 |

---

### vss_prove

指定シェアのKZG proofを生成する。

```typescript
export function vss_prove(
  commitment: Uint8Array,
  share: VssShare,
  polynomial_coeffs: Uint8Array
): Uint8Array;
```

**Parameters**:
| Name | Type | Description |
|------|------|-------------|
| `commitment` | `Uint8Array` | KZGコミットメント (48 bytes) |
| `share` | `VssShare` | 証明対象のシェア |
| `polynomial_coeffs` | `Uint8Array` | 多項式係数（復元用に保持） |

**Returns**: KZG opening proof (48 bytes)

**Note**: 通常は `vss_split` 時に全proofを生成するため、この関数は再生成用。

---

### verify_kzg_proof

KZG proofを検証する。

```typescript
export function verify_kzg_proof(
  commitment: Uint8Array,
  index: number,
  value: Uint8Array,
  proof: Uint8Array
): boolean;
```

**Parameters**:
| Name | Type | Description |
|------|------|-------------|
| `commitment` | `Uint8Array` | KZGコミットメント (48 bytes) |
| `index` | `number` | シェアインデックス |
| `value` | `Uint8Array` | シェア値 (32 bytes) |
| `proof` | `Uint8Array` | KZG proof (48 bytes) |

**Returns**: 検証成功なら `true`

**Errors**:
| Error | Description |
|-------|-------------|
| `InvalidCommitment` | コミットメントが不正なG1点 |
| `InvalidProof` | proofが不正なG1点 |
| `SrsNotLoaded` | Trusted Setupが未初期化 |

---

### init_srs

Trusted Setup (SRS) を初期化する。

```typescript
export function init_srs(srs_bytes: Uint8Array): void;
```

**Parameters**:
| Name | Type | Description |
|------|------|-------------|
| `srs_bytes` | `Uint8Array` | SRSファイルのバイト列 |

**Note**: アプリケーション起動時に1回呼び出す。埋め込みSRSを使用する場合は自動初期化。

---

### compress

データを圧縮する（内部用、テスト公開）。

```typescript
export function compress(data: Uint8Array): Uint8Array;
```

---

### decompress

データを解凍する（内部用、テスト公開）。

```typescript
export function decompress(data: Uint8Array): Uint8Array;
```

---

## TypeScript Types

```typescript
// Re-export for consumers
export interface VssSplitResult {
  commitment: Uint8Array;
  shares: VssShare[];
  proofs: Uint8Array[];
  compressed: boolean;
  multi_segment: boolean;
  segment_count?: number;
}

export interface VssShare {
  index: number;
  value: Uint8Array;
}
```

---

## Usage Example

```typescript
import init, {
  vss_split,
  vss_recover,
  verify_kzg_proof
} from 'anarchy-wasm-engine';

// Initialize Wasm module
await init();

// Split data into shares
const data = new TextEncoder().encode('Hello, KZG-VSS!');
const result = vss_split(data, 3, 5);  // 3-of-5

console.log('Commitment:', result.commitment);
console.log('Shares:', result.shares.length);
console.log('Compressed:', result.compressed);

// Verify a proof
const isValid = verify_kzg_proof(
  result.commitment,
  result.shares[0].index,
  result.shares[0].value,
  result.proofs[0]
);
console.log('Proof valid:', isValid);

// Recover data from 3 shares
const recoveredShares = result.shares.slice(0, 3);
const recovered = vss_recover(recoveredShares, 3, result.compressed);
const text = new TextDecoder().decode(recovered);
console.log('Recovered:', text);
```

---

## Error Handling

全ての関数は失敗時に例外をスローする。

```typescript
try {
  const result = vss_split(data, 10, 5);  // k > n
} catch (e) {
  console.error('VSS split failed:', e.message);
  // "InvalidThreshold: k (10) must be <= n (5)"
}
```
