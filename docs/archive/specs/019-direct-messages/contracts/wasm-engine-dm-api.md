# Contract: `wasm-engine` DM Module API

**Feature**: 019-direct-messages
**Target crate**: `packages/wasm-engine` (new module `src/dm/`)
**Phase**: 1 (Design)

WebAssembly 経由でフロントエンドが呼び出す DM 暗号処理 API。`#[wasm_bindgen]` で公開する関数のシグネチャを定める。秘密鍵はすべてこのモジュール内で完結処理し、ブラウザ JavaScript 側に漏らさない。

---

## W1. `dm_encrypt_and_pad`

**Purpose**: 平文本文を (1) envelope 構築, (2) ISO 7816-4 パディング, (3) AES-256-GCM 暗号化までをクライアント側で実行する。フラグメント化は別関数 (既存 `merkle::split`) に委ねる。

**鍵タイプと役割**:

| 引数 | 鍵種別 | 所有者 |
|------|--------|--------|
| `recipient_scan_pub` | X25519 (32B) | 受信者公開 (DmMetaAddress.scan_pub) |
| `recipient_spend_pub` | Ed25519 compressed Edwards point (32B) | 受信者公開 (DmMetaAddress.spend_pub) |
| `sender_main_account` | Sr25519 AccountId32 (32B) | 送信者公開 |
| `sender_signature` | Sr25519 署名 (64B) | 送信者が外部 wallet signer で事前に作成 (Constitution II) |

```rust
#[wasm_bindgen]
pub fn dm_encrypt_and_pad(
    recipient_scan_pub: &[u8],       // 32 bytes (X25519)
    recipient_spend_pub: &[u8],      // 32 bytes (Ed25519 compressed Edwards point)
    sender_main_account: &[u8],      // 32 bytes (Sr25519 AccountId32)
    sender_signature: &[u8],         // 64 bytes (Sr25519, W6 の inner_signed_hash を外部で署名済み)
    body: &[u8],                     // 任意長 (ただし最大バケット - envelope_overhead - 16 以下)
    timestamp_ms: u64,
) -> Result<DmEncryptedOutput, JsError>;

#[wasm_bindgen]
pub struct DmEncryptedOutput {
    /// パディング済み envelope を暗号化した結果。`.len()` は必ずバケット値と一致。
    pub ciphertext: Vec<u8>,
    /// X25519 公開鍵 (32B)。on-chain DmDispatch.ephemeral_pubkey になる。
    pub ephemeral_pubkey: [u8; 32],
    /// Ed25519 compressed Edwards point (32B)。AccountId32 として送信される。
    pub recipient_stealth: [u8; 32],
    /// 採用されたバケット値 (1024, 4096, 16384, 65536, 262144)。
    pub padding_bucket: u32,
}
```

**Preconditions**:
- `recipient_scan_pub.len() == 32`、かつ非ゼロ。
- `recipient_spend_pub.len() == 32`、かつ `CompressedEdwardsY(...).decompress()` が `Some(_)` (= 有効な Edwards 点)。
- `sender_main_account.len() == 32`。
- `sender_signature.len() == 64`。**呼出側は W6 `dm_compute_inner_signed_hash` で得たハッシュをウォレット signer に渡し、事前に署名を取得してから本関数を呼ぶ。**
- `encode(envelope).len() + 1 + 16 <= 262_144` (envelope + ISO 7816 terminator + AES-GCM tag)。超過時 `JsError("body too large")`。

**Behavior**:
1. X25519 fresh keypair 生成 (`eph_pub`, `eph_priv`) via `x25519_dalek::StaticSecret::random_from_rng(OsRng)`。
2. 既存 `stealth::address` と同じ式で `recipient_stealth = K_spend + H(X25519(eph_priv, K_scan)) * G` を導出。内部は W5 `dm_derive_recipient_stealth_internal` を呼ぶ純粋ヘルパ。
3. `envelope = DmEnvelope { version: 1, sender_main_account, timestamp_ms, body, signature: sender_signature }` を SCALE で符号化。
4. `bucket = select_padding_bucket(encoded.len() + 1)`、失敗なら `body too large`。
5. `padded = pad_iso7816_4(encoded, bucket - 16)` (AES-GCM tag 16B 分を確保した後で padding)。
6. `shared = X25519(eph_priv, recipient_scan_pub)`; `hkdf_okm = HKDF-SHA256(salt=b"anarchy-dm-v1", ikm=shared, info=recipient_stealth || eph_pub)` → 44 bytes。
7. `ciphertext = AES-256-GCM.encrypt(key=hkdf_okm[..32], nonce=hkdf_okm[32..44], aad=recipient_stealth || eph_pub || (padded.len() as u32 BE) || 0x01, plaintext=padded)`。
8. `eph_priv` をゼロクリアして drop (FR-021)。
9. `DmEncryptedOutput { ciphertext, ephemeral_pubkey: eph_pub, recipient_stealth, padding_bucket: bucket }` を返す。

**Errors** (全て `JsError`):
- `"invalid scan pub length"` / `"invalid spend pub length"` / `"invalid spend pub: not on Edwards curve"`
- `"invalid sender account length"` / `"invalid signature length"`
- `"body too large"`
- `"rng failure"` (getrandom 失敗)

**Test acceptance**:
- プロパティテスト: `dm_decrypt_scan` の結果と plaintext が往復一致する (任意 body ≤ 262_000 byte, 任意 meta address)。
- 異なるメタアドレスで暗号化した ciphertext は `dm_decrypt_scan` で自分宛と誤判定しない。
- `body = b""` (空本文) でもバケット 1 KB で成功し、`ciphertext.len() == 1024`。
- `spend_pub` が Edwards 曲線上にない 32B (例: `[0xff; 32]`) を渡すと `"invalid spend pub"` でエラー。

**重要**: 送信者秘密鍵に依存する `sender_signature` は本関数の外部で作成する（理由: Constitution II。ウォレット signer は JS 側の polkadot-api で呼ばれ、WASM 層には鍵を渡さない）。署名対象のハッシュは W6 で決定論的に計算できるため、JS 側は「WASM で hash を計算 → wallet signer で sign → WASM で encrypt」の 3 ステップ構成となる。

---

## W2. `dm_decrypt_scan`

**Purpose**: 1 件の `DmDispatch` エントリ (ephemeral_pubkey + recipient_stealth + ciphertext) を受け、自分宛なら復号して envelope を返す。

**鍵タイプと役割**:

| 引数 | 鍵種別 | 所有者 |
|------|--------|--------|
| `own_scan_priv` | X25519 秘密鍵 (32B) | 受信者本人 (WASM 内で所有) |
| `own_spend_pub` | Ed25519 compressed Edwards point (32B) | 受信者本人の公開部 |
| `ephemeral_pubkey` | X25519 (32B) | オンチェーンから取得 |
| `purported_recipient_stealth` | Ed25519 compressed Edwards point (32B, AccountId32 生バイト) | オンチェーンから取得 |

**注**: `own_spend_priv` は**本関数の入力ではない**。stealth address のマッチング検証は `K_spend + H(shared) * G == purported_stealth` という公開鍵空間の恒等式で完結するため、受信者の `spend_priv` は本関数内で不要。`spend_priv` が必要なのは「stealth アカウントで tx を署名したい」場合のみで、DM 受信ではそれが発生しない。

```rust
#[wasm_bindgen]
pub fn dm_decrypt_scan(
    own_scan_priv: &[u8],                // 32 bytes (X25519 secret)
    own_spend_pub: &[u8],                // 32 bytes (Ed25519 compressed)
    ephemeral_pubkey: &[u8],             // 32 bytes (X25519)
    purported_recipient_stealth: &[u8],  // 32 bytes (Ed25519 from on-chain AccountId32)
    ciphertext: &[u8],
) -> Option<DmDecryptedEnvelope>;

#[wasm_bindgen]
pub struct DmDecryptedEnvelope {
    pub sender_main_account: [u8; 32],   // Sr25519 AccountId32
    pub timestamp_ms: u64,
    pub body: Vec<u8>,
    pub signature_valid: bool,
}
```

**Behavior**:
1. 既存 `stealth::scan::scan_transaction` と同じ式で `expected = K_spend + H(X25519(own_scan_priv, eph_pub)) * G` を計算し、`purported_recipient_stealth` と一致しなければ即 `None`。
2. X25519 DH (`shared = X25519(own_scan_priv, eph_pub)`) → HKDF-SHA256 で key/nonce 導出 (W1 と同入力)。
3. AES-256-GCM 復号 (AAD も W1 と一致)。失敗時 `None`。
4. ISO 7816-4 padding 剥ぎ取り → SCALE decode で `DmEnvelope` を取得。decode 失敗時 `None`。
5. `sig_hash = dm_compute_inner_signed_hash(sender_main_account, purported_recipient_stealth, eph_pub, timestamp_ms, body)` を内部再計算し、Sr25519 署名検証 → `signature_valid` として返す。
6. ステップ 1 で一致しなかったケースと 3–4 の復号失敗ケースは**同じ `None` を返す**（タイミングサイドチャネル緩和のため、より細かい失敗理由は返さない）。

**Test acceptance**:
- ラウンドトリップテスト (encrypt → decrypt) で `sender_main_account` / `timestamp_ms` / `body` / `signature_valid=true` が全フィールド一致。
- 別人の meta address で暗号化されたものは `None`。
- ciphertext を 1 bit 破壊すると `None`。
- 署名部分だけ改ざんすると `Some(_)` だが `signature_valid == false`。
- `purported_recipient_stealth` が all-zero (不正値) でも panic せず `None` を返す。

---

## W3. `dm_generate_sender_stealth`

**Purpose**: 送信者側で「送信者メインアカウントとも受信者メタアドレスとも独立した」新鮮な Sr25519 keypair を生成するヘルパ。生成された seed は WASM 境界を越えて JS 側で短期保持され、`pallet-messaging::send_dm` (tx2) の署名 1 回のみに使い、その直後にゼロクリアする。

```rust
#[wasm_bindgen]
pub fn dm_generate_sender_stealth() -> DmSenderStealth;

#[wasm_bindgen]
pub struct DmSenderStealth {
    pub account_id: [u8; 32],   // Sr25519 公開鍵 (= AccountId32)
    pub secret_seed: [u8; 32],  // Sr25519 seed (tx2 署名専用、使用後 JS 側で即ゼロクリア)
}
```

**Behavior**:
- `getrandom` 経由で 32 bytes のエントロピーを取得し、Sr25519 seed とする (`schnorrkel::MiniSecretKey::from_bytes`)。
- `MiniSecretKey → SecretKey → PublicKey` の順で公開鍵 32 bytes を計算。
- 両方を返す。内部変数 (MiniSecretKey, SecretKey) は関数スコープ終了時に drop。

**Constitution II (Minimal Key Exposure) との整合**:

`secret_seed: [u8; 32]` を WASM→JS に返す点は、本プロジェクトのセキュリティ原則「生秘密鍵を JS 層に出さない」に対する**限定的かつ必要な例外**である。根拠は以下:

- **鍵の寿命が 1 tx / 数秒**: sender stealth keypair は「`send_dm` tx1 つの署名」のためだけに存在する使い捨て鍵。永続化しない。FR-021 のエフェメラル鍵と同じ寿命モデル。
- **保護対象が「送信者と DM の紐付け」のみ**: この鍵が漏れても、攻撃者が得るのは「sender_stealth が発行した tx2 を偽造できる」能力であり、送信者メインアカウント (Sr25519) の残高・アイデンティティには到達しない。主アカウント鍵 (Constitution II の本来の保護対象) は別経路。
- **代替手段なし**: polkadot-api が tx を送信するには `SigningPair` 相当の秘密鍵にアクセスする必要があり、既存 polkadot-api ウォレット signer 統合は「ユーザー所有の main Sr25519 鍵」を前提としていて fresh keypair を受け付けない。将来 `SubstrateSigner` トレイトを WASM 経由で提供する拡張を検討する余地はあるが、MVP スコープ外。

この例外は **Complexity Tracking** 項目として plan.md に記録し、Phase 3 以降で `SubstrateSigner` WASM ラッパで代替できるか再評価する (既存ウォレットライブラリの API が変わった場合の移行計画も含む)。

**Test acceptance**:
- 2 連続呼出で異なる `seed` / `account_id` が得られる。
- `seed` から Sr25519 鍵ペアを再構築したとき `account_id` が一致する。
- `account_id` をそのまま `AccountId32` として Substrate tx に使えること (ラウンドトリップテスト: WASM 生成 → JS tx 組立 → mock runtime で signature verify が通る)。

---

## W5. `dm_derive_recipient_stealth`

**Purpose**: 送信者側で「受信メタアドレス + eph_priv」から受信者 stealth 公開鍵 (Ed25519) を導出する薄いラッパ。既存 `stealth::address::derive_stealth_address` は内部で ephemeral を生成するため、DM では `eph_priv` を再利用して HKDF 入力も揃える必要があり、新規ラッパが必要。

```rust
#[wasm_bindgen]
pub fn dm_derive_recipient_stealth(
    recipient_scan_pub: &[u8],    // 32 bytes (X25519)
    recipient_spend_pub: &[u8],   // 32 bytes (Ed25519 compressed Edwards point)
    ephemeral_priv: &[u8],        // 32 bytes (X25519 secret、呼出側が生成しすぐ破棄)
) -> Result<DmStealthDerivation, JsError>;

#[wasm_bindgen]
pub struct DmStealthDerivation {
    pub stealth_pubkey: [u8; 32],   // Ed25519 compressed Edwards (recipient stealth AccountId32)
    pub ephemeral_pubkey: [u8; 32], // X25519 (ephemeral_priv の公開部)
    pub shared_secret: [u8; 32],    // X25519 DH 結果 (HKDF への ikm)
}
```

**Behavior**:
- `eph_pub = X25519_base * ephemeral_priv`。
- `shared = X25519(ephemeral_priv, recipient_scan_pub)`。
- `h = blake2b_256(shared)`; `h_scalar = Scalar::from_bytes_mod_order(h)`。
- `P_stealth = CompressedEdwardsY(recipient_spend_pub).decompress()? + ED25519_BASEPOINT_TABLE * h_scalar`。
- `stealth_pubkey = P_stealth.compress().to_bytes()`。
- `ephemeral_priv` は関数スコープ終了時に drop (呼出側もゼロクリア推奨)。

**注**: 本関数は内部用途で、実運用では `dm_encrypt_and_pad` が自前で ephemeral を生成して内部呼出する (W1 ステップ 1–2)。`dm_derive_recipient_stealth` が単独で JS から呼ばれるのは「将来の別機能 (複数受信者向け etc.) で使い回す」ためのオプション。MVP では `dm_encrypt_and_pad` に統合しても良い。

**Test acceptance**:
- 既存 `derive_stealth_address(format_meta_address(spend_pub, scan_pub))` の結果と、同じ `ephemeral_priv` を本関数に与えた結果が**一致**する (既存実装との互換性担保)。
- `recipient_spend_pub` が Edwards 曲線上にないと `JsError`。

---

## W6. `dm_compute_inner_signed_hash`

**Purpose**: `sender_signature` (W1 の引数) の対象ハッシュを決定論的に計算する純関数。JS 側から wallet signer に渡すハッシュを WASM で計算することで、送受信双方の hash 計算がビット一致する (実装ドリフト防止)。

```rust
#[wasm_bindgen]
pub fn dm_compute_inner_signed_hash(
    sender_main_account: &[u8],         // 32 bytes (Sr25519 AccountId32)
    recipient_stealth: &[u8],           // 32 bytes (Ed25519 compressed)
    ephemeral_pubkey: &[u8],            // 32 bytes (X25519)
    timestamp_ms: u64,
    body: &[u8],
) -> Result<Vec<u8>, JsError>;   // 32 bytes (blake2b_256)
```

**Behavior**:
- `body_hash = blake2b_256(body)`。
- `payload = [0x01 /* version */] || sender_main_account || recipient_stealth || ephemeral_pubkey || timestamp_ms.to_le_bytes() || body_hash`。
- `return blake2b_256(payload)`。

**Test acceptance**:
- 同一入力で常に同一の 32 byte ハッシュ。
- 入力の 1 bit を変えるとハッシュが一致しない (衝突耐性)。
- W1 / W2 の内部計算と**完全一致** (プロパティテストで検証)。

---

## W4. `dm_fragment_ciphertext`

**Purpose**: ciphertext を既存の SSS + Merkle パイプラインに通すラッパ（post パイプラインの再利用を明示）。

```rust
#[wasm_bindgen]
pub fn dm_fragment_ciphertext(
    ciphertext: &[u8],
    k: u32,
    n: u32,
) -> Result<DmFragmentedOutput, JsError>;

#[wasm_bindgen]
pub struct DmFragmentedOutput {
    pub merkle_root: [u8; 32],
    pub fragments: Vec<Fragment>,  // 既存 merkle::Fragment 型を再利用
}
```

**Behavior**:
- 既存 `merkle::split(ciphertext, k, n)` をそのまま呼ぶ。
- MVP では k/n は UI/環境設定から渡される値を尊重（例: k=3, n=5 を既定推奨）。

**Test acceptance**:
- 既存 post パイプラインの単体テストと同じ入力で同じ MerkleRoot を得る。

---

## Stability & Versioning

- すべての関数は `protocol_version = 1` を前提とする。v2 に上がる場合は `dm_encrypt_and_pad_v2` 等の並行 API を追加し、旧 API は維持して受信側の後方互換性を担保する。
- `#[wasm_bindgen]` 越しの struct 変更は ABI 破壊と見なす。フィールド追加のみ、削除・名前変更は禁止。

---

## Non-API Helpers (内部)

以下は `#[wasm_bindgen]` で公開しない内部関数。公開 API のテストを通してのみ検証する。

- `pad_iso7816_4(input: &[u8], target_len: usize) -> Vec<u8>`
- `strip_iso7816_4(padded: &[u8]) -> Option<&[u8]>`
- `hkdf_okm(shared: &[u8; 32], salt: &[u8], info: &[u8]) -> [u8; 44]`
- `blake2b_256(bytes: &[u8]) -> [u8; 32]` (既存 `stealth::hash` から再エクスポート)
