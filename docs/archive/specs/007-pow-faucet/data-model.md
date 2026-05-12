# Data Model: PoW Faucet

**Feature**: 007-pow-faucet  
**Date**: 2026-02-09  
**Status**: Complete

## Entity Definitions

### 1. FaucetClaim

Faucet利用記録。アカウントごとの二重利用を防止するための記録。

| Field | Type | Description |
|-------|------|-------------|
| account_id | AccountId | Faucetを利用したアカウント（Primary Key） |
| block_number | u32 | 利用時のブロック番号 |
| amount | Balance | 付与された$moral量 |

**Storage Type**: `StorageMap<AccountId, FaucetClaimRecord>`

**Notes**:
- アカウントが存在すれば「利用済み」、存在しなければ「未利用」
- 軽量化のため、単純なフラグ（bool）ではなく記録を保持し、将来の分析に活用可能

### 2. TotalClaims

Faucet利用済みアカウント総数。動的難易度調整に使用。

| Field | Type | Description |
|-------|------|-------------|
| value | u64 | Faucet利用済みアカウント総数 |

**Storage Type**: `StorageValue<u64, ValueQuery>`

**Notes**:
- claim成功時に+1される
- 難易度計算の入力値として使用

### 3. DifficultyConfig

動的PoW難易度設定。Runtime Constantsとして定義。

| Field | Type | Description |
|-------|------|-------------|
| base_difficulty | u8 | 初期難易度（先頭0ビット数） |
| scaling_factor | u64 | 難易度+1に必要なアカウント数の倍率 |
| max_difficulty | u8 | 難易度上限 |

**Storage Type**: Runtime Constants (`#[pallet::constant]`)

**Default Values**:
- base_difficulty: 18（約5秒）
- scaling_factor: 1000
- max_difficulty: 28（約3分）

**難易度計算式**:
```
difficulty = min(
  base_difficulty + floor(log2(1 + total_claims / scaling_factor)),
  max_difficulty
)
```

**難易度曲線**:
| 利用数 | 難易度 | 計算時間目安 |
|---------|--------|-------------|
| 0-999 | 18 | ~3秒 |
| 1,000-1,999 | 19 | ~6秒 |
| 2,000-3,999 | 20 | ~12秒 |
| 4,000-7,999 | 21 | ~24秒 |
| 8,000-15,999 | 22 | ~48秒 |
| 16,000-31,999 | 23 | ~1.5分 |
| 32,000+ | 24-28 | ~3分（上限） |

### 3. FaucetConfig

Faucet全般の設定。Runtime Constantsとして定義。

| Field | Type | Description |
|-------|------|-------------|
| reward_amount | Balance | 1回のFaucet利用で付与される$moral量 |
| challenge_validity | u32 | チャレンジの有効期限（ブロック数） |

**Storage Type**: Runtime Constants

**Notes**:
- reward_amount初期値: 100 MORAL（100_000_000_000_000 planck）
- challenge_validity初期値: 100ブロック

## Derived/Computed Values

### Challenge

PoWパズルの問題。ストレージには保存せず、オンチェーンで計算・検証。

```
challenge = blake2_256(block_hash || account_id)
```

| Component | Type | Description |
|-----------|------|-------------|
| block_hash | H256 | 参考ブロックのハッシュ |
| account_id | AccountId | 請求者のアカウントID |

### Solution Verification

PoW解の検証。ストレージには保存しない。

```
hash = blake2_256(challenge || nonce)
valid = leading_zeros(hash) >= difficulty
```

| Field | Type | Description |
|-------|------|-------------|
| nonce | u64 | ユーザーが計算したnonce値 |
| hash | H256 | 検証用ハッシュ |

## State Transitions

### Faucet Claim Flow

```
┌─────────────────┐
│  Unclaimed      │  FaucetClaims[account] = None
└────────┬────────┘
         │
         │ claim(block_number, nonce)
         │
         ▼
┌─────────────────┐
│  Claimed        │  FaucetClaims[account] = Some(FaucetClaimRecord)
└─────────────────┘
```

### Validation Rules

1. **AlreadyClaimed**: `FaucetClaims[who].is_some()` → Error
2. **ChallengeExpired**: `current_block - block_number > ChallengeValidity` → Error
3. **InvalidProof**: `leading_zeros(hash) < Difficulty` → Error

## Relationships

```
┌───────────────────────────────────────────────────────────────┐
│                        pallet-faucet                          │
│                                                               │
│  ┌─────────────────┐         ┌─────────────────────────────┐ │
│  │ FaucetClaims    │         │ Config (Constants)          │ │
│  │ ────────────────│         │ ─────────────────────────── │ │
│  │ AccountId → Rec │         │ Difficulty: u8              │ │
│  │                 │         │ RewardAmount: Balance       │ │
│  └─────────────────┘         │ ChallengeValidity: u32      │ │
│                              └─────────────────────────────┘ │
│                                           │                   │
│                                           │ fund_account      │
│                                           ▼                   │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │                    pallet-balances                       │ │
│  │  Balances[AccountId].free += RewardAmount                │ │
│  └─────────────────────────────────────────────────────────┘ │
└───────────────────────────────────────────────────────────────┘
```

## Frontend State

### FaucetButton State (WalletConnect内)

| Field | Type | Description |
|-------|------|-------------|
| status | 'idle' \| 'mining' \| 'submitting' \| 'success' \| 'error' | 現在の状態 |
| error | FaucetError \| null | エラー情報 |

**Notes**:
- ボタンは常にクリック可能（`mining`/`submitting`中はdisabled）
- `success`/`error`後は自動的に`idle`に戻る（数秒後）
- 重複制限はブロックチェーン側で行う（フロントエンドでは判定しない）

### FaucetError

| Field | Type | Description |
|-------|------|-------------|
| code | 'AlreadyClaimed' \| 'ChallengeExpired' \| 'InvalidProof' \| 'NetworkError' | エラーコード |
| message | string | ローカライズされたエラーメッセージ |

### i18n Error Messages

追加するキー（`apps/frontend/src/i18n/translations/ja.json`, `en.json`）:

```json
// ja.json
{
  "faucet.button": "Faucet",
  "faucet.mining": "計算中...",
  "faucet.submitting": "送信中...",
  "faucet.success": "100 MORALを受け取りました！",
  "error.alreadyClaimed": "既にFaucetを利用済みです",
  "error.challengeExpired": "チャレンジの有効期限が切れました。再試行してください",
  "error.invalidProof": "PoW計算結果が無効です。再試行してください",
  "error.blockNotFound": "指定されたブロックが見つかりません",
  "error.faucetNetworkError": "ネットワークエラーが発生しました。再試行してください"
}

// en.json
{
  "faucet.button": "Faucet",
  "faucet.mining": "Mining...",
  "faucet.submitting": "Submitting...",
  "faucet.success": "Received 100 MORAL!",
  "error.alreadyClaimed": "Faucet already claimed",
  "error.challengeExpired": "Challenge expired. Please try again",
  "error.invalidProof": "Invalid PoW proof. Please try again",
  "error.blockNotFound": "Block not found",
  "error.faucetNetworkError": "Network error. Please try again"
}
```
