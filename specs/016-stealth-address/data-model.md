# Data Model: Stealth Address Integration

## Overview

ステルスアドレス統合機能のデータモデル定義。オンチェーン（Substrate Storage）、オフチェーン（ブラウザセッションメモリ）、バックアップファイルの3層で構成。

---

## 1. On-Chain Entities (Substrate Storage)

### 1.1 EphemeralKeyEntry

ステルス送金時に記録されるエフェメラル公開鍵エントリ。

```rust
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct EphemeralKeyEntry<AccountId> {
    /// エフェメラル公開鍵 (X25519 public key, 32 bytes)
    pub ephemeral_pubkey: [u8; 32],
    
    /// ステルスアドレス (送金先)
    pub stealth_address: AccountId,
}
```

| Field | Type | Size | Description |
|-------|------|------|-------------|
| ephemeral_pubkey | [u8; 32] | 32 bytes | 送信者が生成したエフェメラル公開鍵 |
| stealth_address | AccountId | 32 bytes | 導出されたワンタイムステルスアドレス |

### 1.2 Storage Maps

```rust
/// ブロック番号ごとのエフェメラル公開鍵リスト
/// Key: BlockNumber
/// Value: BoundedVec<EphemeralKeyEntry, MaxEntriesPerBlock>
#[pallet::storage]
pub type EphemeralKeys<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    BlockNumberFor<T>,
    BoundedVec<EphemeralKeyEntry<T::AccountId>, T::MaxEntriesPerBlock>,
    ValueQuery,
>;
```

### 1.3 Configuration Constants

```rust
#[pallet::config]
pub trait Config: frame_system::Config + pallet_balances::Config {
    /// 1ブロックあたりの最大エフェメラル公開鍵数
    type MaxEntriesPerBlock: Get<u32>;  // Default: 1000
    
    /// Weight計算用
    type WeightInfo: WeightInfo;
}
```

---

## 2. Off-Chain Entities (Session Memory)

### 2.1 StealthKeyPair

ユーザーのステルスアドレス鍵ペア。セッション中のみメモリに保持。

```typescript
interface StealthKeyPair {
  /** Spend秘密鍵 (32 bytes) - アドレスからの出金に使用 */
  spendKey: Uint8Array;
  
  /** View秘密鍵 (32 bytes) - トランザクションスキャンに使用 */
  viewKey: Uint8Array;
  
  /** 生成タイムスタンプ */
  createdAt: number;
}
```

| Field | Type | Size | Description |
|-------|------|------|-------------|
| spendKey | Uint8Array | 32 bytes | 支出用秘密鍵 (X25519 StaticSecret) |
| viewKey | Uint8Array | 32 bytes | 閲覧用秘密鍵 (X25519 StaticSecret) |
| createdAt | number | 8 bytes | Unix timestamp (ms) |

### 2.2 StealthMetaAddress

公開可能なステルスメタアドレス。送信者がこれを使ってワンタイムアドレスを導出。

```typescript
interface StealthMetaAddress {
  /** Spend公開鍵 (32 bytes) */
  spendPubkey: Uint8Array;
  
  /** View公開鍵 (32 bytes) */
  viewPubkey: Uint8Array;
}
```

**エンコード形式**: `st:anarchy:<base58(spendPubkey || viewPubkey)>`

例: `st:anarchy:5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY...`

### 2.3 DetectedStealthBalance

スキャンで検出されたステルス残高。

```typescript
interface DetectedStealthBalance {
  /** ステルスアドレス (SS58) */
  stealthAddress: string;
  
  /** 残高 (MORAL単位、12桁精度) */
  balance: bigint;
  
  /** 受信ブロック番号 */
  receivedAt: number;
  
  /** 送金トランザクションハッシュ */
  txHash: Uint8Array;
  
  /** 支出済みフラグ */
  spent: boolean;
  
  /** エフェメラル公開鍵 (秘密鍵導出用に保持) */
  ephemeralPubkey: Uint8Array;
}
```

| Field | Type | Description |
|-------|------|-------------|
| stealthAddress | string | SS58形式のステルスアドレス |
| balance | bigint | 残高 (最小単位) |
| receivedAt | number | 受信ブロック番号 |
| txHash | Uint8Array | トランザクションハッシュ (32 bytes) |
| spent | boolean | 支出済みか |
| ephemeralPubkey | Uint8Array | 対応するエフェメラル公開鍵 (32 bytes) |

---

## 3. Backup File Format

### 3.1 StealthBackup

暗号化されたバックアップファイル構造。

```typescript
interface StealthBackup {
  /** フォーマットバージョン */
  version: 1;
  
  /** 暗号化メタデータ */
  crypto: {
    /** 暗号化アルゴリズム */
    algorithm: 'AES-256-GCM';
    
    /** 鍵導出関数 */
    kdf: 'PBKDF2-SHA256';
    
    /** PBKDF2イテレーション回数 */
    iterations: 100000;
    
    /** ソルト (16 bytes, base64) */
    salt: string;
    
    /** IV/Nonce (12 bytes, base64) */
    nonce: string;
  };
  
  /** 暗号化されたペイロード (base64) */
  ciphertext: string;
  
  /** 認証タグ (16 bytes, base64) */
  authTag: string;
}
```

### 3.2 Decrypted Payload

復号後のペイロード構造。

```typescript
interface StealthBackupPayload {
  /** Spend秘密鍵 (hex) */
  spendKey: string;
  
  /** View秘密鍵 (hex) */
  viewKey: string;
  
  /** 生成タイムスタンプ */
  createdAt: number;
  
  /** チェックサム (Blake2b-256 of keys) */
  checksum: string;
}
```

---

## 4. State Transitions

### 4.1 Key Lifecycle

```
[No Keys] 
    │
    ├─ generateKeys() ─────────────────────────►  [Keys in Memory]
    │                                                    │
    │                                                    ├─ exportBackup() ──► [Backup File Downloaded]
    │                                                    │
    │                                                    ├─ sessionEnd ──────► [Keys Destroyed] ──► [No Keys]
    │                                                    │
[No Keys] ◄──────────────────────────────────────────────┘
    │
    └─ importBackup(file, password) ───────────►  [Keys in Memory]
```

### 4.2 Stealth Transaction Lifecycle

```
[Sender has Meta-Address]
    │
    ├─ deriveStealthAddress(metaAddress)
    │       │
    │       ▼
    │   [Stealth Address + Ephemeral Pubkey]
    │       │
    │       ├─ send_to_stealth(stealth_addr, ephemeral_pubkey, amount)
    │       │       │
    │       │       ▼
    │       │   [On-Chain: Balance + Ephemeral Key Recorded]
    │       │
    │       │
[Receiver scans]
    │
    ├─ scanBlocks(viewKey, startBlock, endBlock)
    │       │
    │       ├─ For each block:
    │       │     ├─ fetch ephemeral keys
    │       │     └─ check ownership with viewKey
    │       │
    │       ▼
    │   [DetectedStealthBalance added to session]
    │
    │
[Receiver spends]
    │
    ├─ deriveStealthPrivateKey(spendKey, viewKey, ephemeralPubkey)
    │       │
    │       ▼
    │   [Stealth Private Key]
    │       │
    │       └─ sign & submit transfer transaction
    │               │
    │               ▼
    │           [Balance spent, marked as spent in session]
```

---

## 5. Validation Rules

### 5.1 On-Chain Validation

| Rule | Location | Error |
|------|----------|-------|
| ephemeral_pubkey は 32 bytes | Pallet extrinsic | InvalidEphemeralKey |
| stealth_address は valid AccountId | Pallet extrinsic | InvalidStealthAddress |
| amount > 0 | Pallet extrinsic | ZeroAmount |
| sender has sufficient balance | Currency::transfer | InsufficientBalance |
| entries per block < MaxEntriesPerBlock | Storage mutation | TooManyEntriesInBlock |

### 5.2 Client-Side Validation

| Rule | Location | Error |
|------|----------|-------|
| Meta-address format valid | StealthSendForm | Invalid meta-address format |
| Backup file version = 1 | importBackup | Unsupported backup version |
| Backup decryption succeeds | importBackup | Invalid password |
| Checksum matches | importBackup | Backup file corrupted |
| View key matches spend key's pair | importBackup | Key pair mismatch |

---

## 6. Indexes (Future: Subquery Indexer)

現時点では未実装。将来のインデクサー導入時の設計案。

```graphql
type StealthTransaction @entity {
  id: ID!  # txHash
  blockNumber: Int! @index
  ephemeralPubkey: Bytes!
  stealthAddress: String! @index
  amount: BigInt!
  timestamp: DateTime!
}

# Query: 特定ブロック以降のすべてのステルストランザクション
query RecentStealthTxs($since: Int!) {
  stealthTransactions(
    filter: { blockNumber_gte: $since }
    orderBy: BLOCK_NUMBER_ASC
  ) {
    nodes {
      ephemeralPubkey
      stealthAddress
      amount
    }
  }
}
```

---

## 7. Size Estimates

### Storage Growth

| Scenario | Entries/Block | Storage/Block | Monthly Growth (10s blocks) |
|----------|---------------|---------------|------------------------------|
| Low usage | 10 | 640 bytes | ~166 MB |
| Medium | 100 | 6.4 KB | ~1.66 GB |
| High | 1000 | 64 KB | ~16.6 GB |

計算: `entries × 64 bytes × blocks_per_day × 30 days`
- 64 bytes = 32 (ephemeral_pubkey) + 32 (AccountId)
- blocks_per_day = 8640 (10s block time)

### Pruning Strategy

古いブロックのエフェメラル公開鍵は定期的に削除可能（オプション）。
- 受信者がスキャン済みであれば、オンチェーンのエフェメラル公開鍵は不要
- ただし新規ユーザーのフルスキャンには必要
- インデクサー導入後は、オンチェーン保持期間を短縮可能（例: 30日）
