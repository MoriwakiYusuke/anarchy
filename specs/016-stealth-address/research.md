# Research: Stealth Address Integration

## Overview

本ドキュメントは、ステルスアドレス統合機能の技術選定と設計判断に関するリサーチ結果をまとめる。spec.mdで回答済みの5つの明確化事項を基に、実装詳細を整理する。

---

## 1. プロトコル標準: EIP-5564互換

### Decision
EIP-5564 (Ethereum Stealth Address) の暗号プロトコルをSubstrate/Anarchyに適用する。

### Rationale
- **業界標準**: EIP-5564はEthereumエコシステムで広く認知されており、将来のクロスチェーン互換性を視野に入れられる
- **検証済みプロトコル**: セキュリティ監査を受けたプロトコル設計
- **シンプルな導出**: `S = H(s・P) + V` の公式で一意のワンタイムアドレスを導出

### Protocol Details (EIP-5564)
```
送信者:
1. 受信者の stealth meta-address (K_spend, K_view) を取得
2. ランダムなエフェメラル秘密鍵 r を生成
3. エフェメラル公開鍵 R = r・G を計算
4. 共有シークレット s = r・K_view を計算
5. ステルスアドレス P_stealth = K_spend + H(s)・G を計算
6. R をオンチェーンに公開、P_stealth に送金

受信者 (スキャン):
1. オンチェーンの R を取得
2. 共有シークレット s' = k_view・R を計算
3. 期待されるステルスアドレス P'_stealth = K_spend + H(s')・G を計算
4. P'_stealth がトランザクションの宛先と一致すれば自分宛
5. 秘密鍵: p_stealth = k_spend + H(s')
```

### Adaptation for Substrate
- 楕円曲線: Curve25519 (X25519鍵交換) を使用
- ハッシュ関数: Blake2b-256 (Substrate標準)
- アドレス形式: SS58 (Substrate標準)

### Alternatives Considered
| Alternative | Rejected Because |
|-------------|------------------|
| 独自プロトコル設計 | セキュリティリスク大、監査コスト高 |
| 完全なERC-5564コントラクト移植 | EVMレイヤー不要、Substrate nativeで十分 |
| Moneroスタイル | 複雑すぎる、リング署名は過剰 |

---

## 2. 暗号ライブラリ選定

### Decision
`x25519-dalek` をWasm向けステルスアドレス暗号に使用する。

### Rationale
- **Pure Rust**: システム依存なし、Wasm32ターゲットで動作確認済み
- **dalek-cryptography**: 業界標準の暗号ライブラリ群（ed25519-dalek、curve25519-dalekなど）
- **no_std互換**: ブロックチェーンランタイムでも使用可能
- **既存使用実績**: wasm-engine内で類似パターン（ark-bls12-381）を既に使用

### Implementation Pattern
```rust
// packages/wasm-engine/src/stealth/keys.rs
use x25519_dalek::{PublicKey, StaticSecret};
use rand_core::OsRng;

pub struct StealthKeyPair {
    pub spend_key: StaticSecret,
    pub view_key: StaticSecret,
}

impl StealthKeyPair {
    pub fn generate() -> Self {
        Self {
            spend_key: StaticSecret::random_from_rng(OsRng),
            view_key: StaticSecret::random_from_rng(OsRng),
        }
    }
    
    pub fn meta_address(&self) -> StealthMetaAddress {
        StealthMetaAddress {
            spend_pubkey: PublicKey::from(&self.spend_key),
            view_pubkey: PublicKey::from(&self.view_key),
        }
    }
}
```

### Alternatives Considered
| Alternative | Rejected Because |
|-------------|------------------|
| ring (AWS LibCrypto) | Wasm32ターゲットでのビルドが複雑 |
| sodiumoxide | C bindingあり、Wasmで問題 |
| subtle-crypto (Web Crypto API) | X25519 DHのみ、stealth導出には不十分 |

---

## 3. フルスキャン開始ブロック

### Decision
Genesis Block (ブロック0) からのフルスキャンをオプション提供（暫定）。

### Rationale
- **データ損失防止**: 過去のすべてのステルス受信を確実に検出
- **暫定的判断**: 将来インデクサーが完成すれば、この方式は非推奨になる
- **ユーザー選択**: 「最近N日」「特定ブロック以降」などのオプションも提供

### Implementation Considerations
- **パフォーマンス**: 大量ブロックのスキャンは時間がかかる
  - Web Worker内でバックグラウンド実行
  - 進捗表示でUX改善
  - ブロック範囲をバッチ処理（1000ブロック/バッチ）
- **RPC負荷**: 標準Substrate RPCを使用、レート制限に注意
- **将来計画**: Subqueryインデクサーでエフェメラル公開鍵をインデックス化

### Scan Strategy
```typescript
// apps/frontend/src/lib/stealth/scanner.ts
async function* scanBlocks(
  api: PolkadotApi,
  viewKey: Uint8Array,
  startBlock: number,
  endBlock: number
): AsyncGenerator<DetectedTransaction> {
  const BATCH_SIZE = 1000;
  
  for (let i = startBlock; i <= endBlock; i += BATCH_SIZE) {
    const batchEnd = Math.min(i + BATCH_SIZE - 1, endBlock);
    const ephemeralKeys = await fetchEphemeralKeys(api, i, batchEnd);
    
    for (const { blockNum, txHash, ephemeralPubkey, recipient } of ephemeralKeys) {
      const isOurs = await checkOwnership(viewKey, ephemeralPubkey, recipient);
      if (isOurs) {
        yield { blockNum, txHash, recipient };
      }
    }
    
    // Report progress
    postMessage({ type: 'progress', block: batchEnd });
  }
}
```

---

## 4. エフェメラル公開鍵クエリ方式

### Decision
標準Substrate RPC（`state_getStorage`, `chain_getBlockHash`）を使用。

### Rationale
- **追加RPC不要**: カスタムRPCの実装・メンテナンスコストを回避
- **Substrate標準**: 既存のPAPI（polkadot-api）でそのまま使用可能
- **将来拡張**: インデクサー移行時にフロントエンド側の変更のみで済む

### Query Pattern
```typescript
// エフェメラル公開鍵のストレージ構造
// pallet_stealth::EphemeralKeys<T>: StorageMap<BlockNumber, Vec<EphemeralKeyEntry>>

interface EphemeralKeyEntry {
  ephemeralPubkey: Uint8Array;  // 32 bytes
  stealthAddress: string;        // SS58
  transactionHash: Uint8Array;  // 32 bytes
}

// フロントエンドからのクエリ
const keys = await api.query.stealthPallet.ephemeralKeys(blockNumber);
```

### Future Enhancement: Indexer
```graphql
# Subquery schema (将来)
type StealthTransaction @entity {
  id: ID!
  blockNumber: Int!
  ephemeralPubkey: Bytes!
  stealthAddress: String!
  amount: BigInt!
}

query StealthTransactions($since: Int!) {
  stealthTransactions(filter: { blockNumber_gte: $since }) {
    ephemeralPubkey
    stealthAddress
    amount
  }
}
```

---

## 5. 秘密鍵ストレージ

### Decision
セッションメモリのみ、永続化なし。バックアップファイルからの都度インポート。

### Rationale
- **既存実装との一貫性**: 現在のAnarchy実装では秘密鍵を保存しない方式
- **セキュリティ最大化**: ブラウザストレージ攻撃（XSS、拡張機能）からの保護
- **ユーザー責任明確化**: バックアップファイルの管理をユーザーに委ねる

### Implementation Pattern

#### Key Manager (Session Memory)
```typescript
// apps/frontend/src/lib/stealth/keyManager.ts

class StealthKeyManager {
  private keyPair: StealthKeyPair | null = null;
  
  async importFromBackup(
    encryptedBackup: Uint8Array,
    password: string
  ): Promise<void> {
    // 1. パスワードでAES-GCM復号
    const decrypted = await decryptWithPassword(encryptedBackup, password);
    
    // 2. 鍵ペアをメモリにロード
    this.keyPair = deserializeKeyPair(decrypted);
    
    // 3. セッション終了時のクリーンアップを登録
    window.addEventListener('beforeunload', () => this.destroy());
  }
  
  async export(password: string): Promise<Uint8Array> {
    if (!this.keyPair) throw new Error('No key pair loaded');
    
    // AES-256-GCM で暗号化
    const serialized = serializeKeyPair(this.keyPair);
    return encryptWithPassword(serialized, password);
  }
  
  destroy(): void {
    if (this.keyPair) {
      // セキュアワイプ（ゼロクリア）
      secureWipe(this.keyPair.spendKey);
      secureWipe(this.keyPair.viewKey);
      this.keyPair = null;
    }
  }
  
  getViewKey(): Uint8Array | null {
    return this.keyPair?.viewKey ?? null;
  }
  
  getSpendKey(): Uint8Array | null {
    return this.keyPair?.spendKey ?? null;
  }
}

// シングルトンエクスポート
export const stealthKeyManager = new StealthKeyManager();
```

#### Backup File Format
```typescript
interface StealthBackupV1 {
  version: 1;
  encrypted: {
    ciphertext: Uint8Array;  // AES-256-GCM encrypted payload
    nonce: Uint8Array;       // 12 bytes
    salt: Uint8Array;        // 16 bytes for PBKDF2
  };
}

// Payload (decrypted)
interface StealthKeyPayload {
  spendKey: Uint8Array;  // 32 bytes
  viewKey: Uint8Array;   // 32 bytes
  createdAt: number;     // Unix timestamp
}
```

### UX Flow
1. **初回**: 鍵生成 → バックアップファイルダウンロード必須
2. **再訪問**: バックアップファイル選択 → パスワード入力 → ロード
3. **セッション終了**: メモリからの鍵破棄（自動）
4. **機種変更**: バックアップファイルを新デバイスへ移行

---

## 6. Web Worker アーキテクチャ

### Decision
Dedicated Web Worker + MessageChannel パターンを使用。

### Rationale
- **メインスレッド非ブロック**: 暗号処理とスキャンはCPU集約的
- **Wasm統合**: wasm-engineをWorker内でロード
- **TypeScript型安全**: comlink または手動MessageChannel

### Architecture
```typescript
// apps/frontend/src/lib/stealth/worker.ts
import init, { generate_stealth_keys, scan_transaction } from 'anarchy-wasm-engine';

let wasmInitialized = false;

self.onmessage = async (event: MessageEvent<WorkerMessage>) => {
  if (!wasmInitialized) {
    await init();
    wasmInitialized = true;
  }
  
  const { type, payload, id } = event.data;
  
  switch (type) {
    case 'generateKeys':
      const keys = generate_stealth_keys();
      self.postMessage({ id, result: keys });
      break;
      
    case 'scanTransaction':
      const { viewKey, ephemeralPubkey, recipient } = payload;
      const isOurs = scan_transaction(viewKey, ephemeralPubkey, recipient);
      self.postMessage({ id, result: isOurs });
      break;
      
    case 'deriveSendAddress':
      // ... stealth address derivation for sending
      break;
  }
};

// Worker message types
type WorkerMessage =
  | { type: 'generateKeys'; id: string }
  | { type: 'scanTransaction'; id: string; payload: ScanPayload }
  | { type: 'deriveSendAddress'; id: string; payload: DerivePayload };
```

### Main Thread Client
```typescript
// apps/frontend/src/lib/stealth/client.ts
class StealthWorkerClient {
  private worker: Worker;
  private pending = new Map<string, { resolve: Function; reject: Function }>();
  
  constructor() {
    this.worker = new Worker(
      new URL('./worker.ts', import.meta.url),
      { type: 'module' }
    );
    
    this.worker.onmessage = (event) => {
      const { id, result, error } = event.data;
      const handler = this.pending.get(id);
      if (handler) {
        error ? handler.reject(error) : handler.resolve(result);
        this.pending.delete(id);
      }
    };
  }
  
  async generateKeys(): Promise<StealthKeyPair> {
    return this.call('generateKeys', {});
  }
  
  private call<T>(type: string, payload: any): Promise<T> {
    const id = crypto.randomUUID();
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.worker.postMessage({ type, payload, id });
    });
  }
}
```

---

## 7. Pallet設計パターン

### Decision
軽量なデータ格納パレットとして設計。ビジネスロジック最小化。

### Rationale
- **シンプルさ**: エフェメラル公開鍵の格納と照会のみ
- **将来の拡張性**: インデクサー移行後もパレット変更不要
- **既存パターン準拠**: pallet-post, pallet-storage と同様の構造

### Storage Design
```rust
// apps/blockchain/pallets/stealth/src/lib.rs

#[pallet::storage]
pub type EphemeralKeys<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    BlockNumberFor<T>,
    BoundedVec<EphemeralKeyEntry<T::AccountId>, T::MaxEntriesPerBlock>,
    ValueQuery,
>;

#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct EphemeralKeyEntry<AccountId> {
    pub ephemeral_pubkey: [u8; 32],
    pub stealth_address: AccountId,
}
```

### Extrinsics
```rust
#[pallet::call]
impl<T: Config> Pallet<T> {
    /// ステルスアドレス宛の送金（エフェメラル公開鍵を記録）
    #[pallet::call_index(0)]
    #[pallet::weight(T::WeightInfo::send_to_stealth())]
    pub fn send_to_stealth(
        origin: OriginFor<T>,
        stealth_address: T::AccountId,
        ephemeral_pubkey: [u8; 32],
        amount: BalanceOf<T>,
    ) -> DispatchResult {
        let sender = ensure_signed(origin)?;
        
        // 1. 送金実行
        T::Currency::transfer(
            &sender,
            &stealth_address,
            amount,
            ExistenceRequirement::KeepAlive,
        )?;
        
        // 2. エフェメラル公開鍵を記録
        let block_num = frame_system::Pallet::<T>::block_number();
        EphemeralKeys::<T>::try_mutate(block_num, |entries| {
            entries.try_push(EphemeralKeyEntry {
                ephemeral_pubkey,
                stealth_address: stealth_address.clone(),
            })
        }).map_err(|_| Error::<T>::TooManyEntriesInBlock)?;
        
        // 3. イベント発行
        Self::deposit_event(Event::StealthTransfer {
            sender,
            stealth_address,
            amount,
        });
        
        Ok(())
    }
}
```

---

## 8. テスト戦略

### Unit Tests

#### Wasm Engine (Rust)
```rust
// packages/wasm-engine/src/stealth/tests.rs
#[test]
fn test_stealth_address_derivation() {
    let receiver = StealthKeyPair::generate();
    let meta_address = receiver.meta_address();
    
    // Sender derives stealth address
    let (stealth_addr, ephemeral_pubkey) = derive_stealth_address(&meta_address);
    
    // Receiver scans and detects
    let detected = scan_for_address(
        &receiver.view_key,
        &ephemeral_pubkey,
        &stealth_addr,
    );
    
    assert!(detected);
}

#[test]
fn test_stealth_private_key_derivation() {
    let receiver = StealthKeyPair::generate();
    let meta_address = receiver.meta_address();
    
    let (stealth_addr, ephemeral_pubkey) = derive_stealth_address(&meta_address);
    
    // Receiver derives private key for stealth address
    let stealth_privkey = derive_stealth_private_key(
        &receiver.spend_key,
        &receiver.view_key,
        &ephemeral_pubkey,
    );
    
    // Verify: public key from derived private key matches stealth address
    let derived_pubkey = PublicKey::from(&stealth_privkey);
    assert_eq!(derived_pubkey.as_bytes(), stealth_addr);
}
```

#### Pallet (Rust)
```rust
// apps/blockchain/pallets/stealth/src/tests.rs
#[test]
fn send_to_stealth_works() {
    new_test_ext().execute_with(|| {
        let sender = 1;
        let stealth_addr = 2;
        let ephemeral = [1u8; 32];
        let amount = 100;
        
        assert_ok!(StealthPallet::send_to_stealth(
            RuntimeOrigin::signed(sender),
            stealth_addr,
            ephemeral,
            amount,
        ));
        
        // Check balance transferred
        assert_eq!(Balances::free_balance(stealth_addr), amount);
        
        // Check ephemeral key recorded
        let entries = StealthPallet::ephemeral_keys(System::block_number());
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].ephemeral_pubkey, ephemeral);
    });
}
```

#### Frontend (TypeScript/Jest)
```typescript
// apps/frontend/tests/lib/stealth/scanner.test.ts
describe('StealthScanner', () => {
  it('detects own transactions', async () => {
    const keyPair = await generateStealthKeys();
    const { stealthAddr, ephemeralPubkey } = deriveStealthAddress(
      keyPair.metaAddress
    );
    
    const isOurs = await scanTransaction(
      keyPair.viewKey,
      ephemeralPubkey,
      stealthAddr
    );
    
    expect(isOurs).toBe(true);
  });
  
  it('ignores others transactions', async () => {
    const myKeyPair = await generateStealthKeys();
    const otherKeyPair = await generateStealthKeys();
    
    const { stealthAddr, ephemeralPubkey } = deriveStealthAddress(
      otherKeyPair.metaAddress
    );
    
    const isOurs = await scanTransaction(
      myKeyPair.viewKey,
      ephemeralPubkey,
      stealthAddr
    );
    
    expect(isOurs).toBe(false);
  });
});
```

---

## Summary

すべての技術選定完了。NEEDS CLARIFICATION項目なし。

| # | Topic | Decision |
|---|-------|----------|
| 1 | Protocol | EIP-5564互換、X25519 + Blake2b |
| 2 | Crypto Library | x25519-dalek (Wasm) |
| 3 | Full Scan Start | Genesis block (暫定) |
| 4 | Query Method | Standard Substrate RPC |
| 5 | Key Storage | Session memory only |
| 6 | Worker Pattern | Dedicated Worker + MessageChannel |
| 7 | Pallet Design | Lightweight data storage |
| 8 | Testing | Unit (Rust + Jest) + Integration |
