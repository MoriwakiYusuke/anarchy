# Research: Identity Pallet

**Date**: 2026-02-07  
**Feature**: 001-identity-pallet

---

## 1. WebAuthn公開鍵フォーマット

### Decision: COSEキー構造（可変長バイト列）で保存

### Rationale

WebAuthn（FIDO2）では公開鍵はCOSE（CBOR Object Signing and Encryption）フォーマットで提供される。

**ES256（P-256/secp256r1）の場合**:
- kty (key type): 2 (EC)
- alg (algorithm): -7 (ES256)
- crv (curve): 1 (P-256)
- x: 32 bytes
- y: 32 bytes

合計サイズ: 約77-91 bytes（CBORエンコーディングのオーバーヘッド含む）

**保存形式の選択**:
```rust
// Option A: 生のCOSEバイト列を保存（選択）
pub type PublicKeyBytes = BoundedVec<u8, ConstU32<256>>;

// Option B: 構造化して保存（未採用）
// 理由: WebAuthn署名検証時にCOSE形式が必要なため、変換コストが発生
```

### Alternatives Considered

1. **非圧縮ECポイント (65 bytes)**: WebAuthn署名検証に追加処理が必要
2. **構造化ストレージ**: 変換コスト、将来の他アルゴリズム対応が困難

---

## 2. Substrateでの可変長データ保存

### Decision: BoundedVec を使用

### Rationale

Substrate では `no_std` 環境のため、動的サイズのコレクションには制約がある。

```rust
use frame_support::BoundedVec;
use sp_runtime::traits::Get;

// 公開鍵（最大256バイト）
pub type PublicKeyBytes<T> = BoundedVec<u8, <T as Config>::MaxPublicKeyLength>;

// Passkey一覧（最大10個）
pub type PasskeyList<T> = BoundedVec<Passkey<T>, <T as Config>::MaxPasskeys>;
```

**ストレージコスト最適化**:
- `StorageMap` で Identity ID → Identity データのマッピング
- `StorageMap` で PasskeyId → Identity ID の逆引き（重複チェック用）
- Passkey一覧は Identity 構造体内に `BoundedVec` で保持（1回のストレージアクセス）

### Alternatives Considered

1. **Vec**: `no_std` で使用可能だが、サイズ制限がなくDoS攻撃に脆弱
2. **固定配列**: 柔軟性がなく、未使用スロットでストレージ浪費

---

## 3. PasskeyIdの導出方法

### Decision: Blake2b-256ハッシュ

### Rationale

PasskeyIdは公開鍵の一意識別子として使用。System全体で重複を防ぐため、決定論的に導出。

```rust
use sp_core::blake2_256;

pub type PasskeyId = [u8; 32];

fn derive_passkey_id(public_key: &[u8]) -> PasskeyId {
    blake2_256(public_key)
}
```

**選択理由**:
- Substrate標準のハッシュ関数（sp-coreに含まれる）
- 32バイト固定長で効率的なストレージキー
- 衝突耐性が十分（256ビット）

### Alternatives Considered

1. **SHA-256**: 外部クレート依存、Substrate標準ではない
2. **Keccak-256**: Ethereum互換だが、Anarchyでは不要
3. **blake2b-512**: オーバースペック、ストレージ効率が悪い

---

## 4. 既存パレット設計パターン

### Decision: moral/post パレットの構造を踏襲

### Rationale

既存パレットの構造分析:

```rust
// moral パレット
#[pallet::storage]
pub type Balances<T> = StorageMap<_, Blake2_128Concat, T::AccountId, BalanceOf<T>>;

#[pallet::storage]
pub type TotalSupply<T> = StorageValue<_, BalanceOf<T>>;

// post パレット  
#[pallet::storage]
pub type Posts<T> = StorageMap<_, Blake2_128Concat, u64, Post<T>>;

#[pallet::storage]
pub type NextPostId<T> = StorageValue<_, u64>;
```

**Identity パレットでの適用**:
```rust
// Identity ID → Identity データ
#[pallet::storage]
pub type Identities<T> = StorageMap<_, Blake2_128Concat, u64, Identity<T>>;

// 次の Identity ID
#[pallet::storage]
pub type NextIdentityId<T> = StorageValue<_, u64>;

// PasskeyId → Identity ID（逆引き）
#[pallet::storage]
pub type PasskeyOwner<T> = StorageMap<_, Blake2_128Concat, PasskeyId, u64>;
```

**テスト構造**: `tests.rs` を別ファイルとして分離（moral/post と同様）

---

## 5. WebAuthn署名検証との連携（将来）

### 現時点の方針

Identity Pallet（本実装）では、公開鍵の**形式検証のみ**行う:
- 長さチェック（1-256バイト）
- 空でないこと

WebAuthn署名検証（Phase 1.4）実装後に以下を追加:
- 公開鍵のCOSEフォーマット検証
- 登録時のattestation検証
- 認証時のassertion検証

```rust
// Phase 1: 形式検証のみ
fn validate_public_key(key: &[u8]) -> Result<(), Error> {
    ensure!(!key.is_empty(), Error::<T>::EmptyPublicKey);
    ensure!(key.len() <= 256, Error::<T>::PublicKeyTooLong);
    Ok(())
}

// Phase 1.4で追加予定: COSE検証
// fn validate_cose_key(key: &[u8]) -> Result<CoseKey, Error> { ... }
```

---

## Summary

| 項目 | 決定 |
|------|------|
| 公開鍵保存形式 | COSEバイト列（BoundedVec<u8, 256>） |
| コレクション型 | BoundedVec（最大10 Passkeys） |
| PasskeyId | Blake2b-256ハッシュ（32バイト固定） |
| ストレージ構造 | StorageMap + 逆引きMap |
| 署名検証 | Phase 1.4で追加（本実装では形式検証のみ） |
