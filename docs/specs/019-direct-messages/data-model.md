# Data Model: Direct Messages

**Feature**: 019-direct-messages
**Phase**: 1 (Design)
**Date**: 2026-04-20

spec.md の Key Entities と research.md の決定を、実装で使う型・保存場所・ライフサイクルの観点から具体化する。オンチェーン型は SCALE エンコーディング、オフチェーン型は `serde` / `Uint8Array` 前提。

---

## 1. オンチェーン型 (`pallet-messaging`)

### 1.1 `DmMetaAddress`

ユーザーが DM を受信するために公開するメタアドレス。016 の `StealthMetaAddress` と同じビット配置を持つが、DM 専用の StorageMap に記録される。

```rust
#[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, PartialEq, Eq)]
pub struct DmMetaAddress {
    /// X25519 scan public key (ECDH 用、受信者スキャン鍵の公開部)。
    /// 既存 packages/wasm-engine/src/stealth/types.rs の `view_pubkey` と**同一概念・同一バイト列**。
    /// 本 pallet では Substrate の命名慣例 (snake_case + 機能名) に合わせ `scan_pub` 名を採用。
    pub scan_pub: [u8; 32],
    /// Ed25519 compressed Edwards point (spend pubkey、stealth address 導出 `K_spend + H(s)*G` の起点)。
    /// 既存 stealth::types の `spend_pubkey` と**同一概念・同一バイト列**。
    pub spend_pub: [u8; 32],
}
```

**Validation**:
- `scan_pub` / `spend_pub` とも非ゼロ (all-zero は公開鍵として不正)。
- `spend_pub` が有効な Edwards point であることは pallet-messaging 側では検証しない (フォーマットチェックのみ)。不正な点の場合は送信者側 `derive_stealth` が失敗して送信不可になるだけで、オンチェーン状態の整合性は保たれる。

**表現境界 (N2 対応)**:

DM メタアドレスは以下の 3 表現が等価に存在する。実装は各境界で明示的に変換する。

| レイヤ | 表現 | 用途 |
|--------|------|------|
| オンチェーン storage | `DmMetaAddress` 構造体 (SCALE 64 B) | `DmReceptionKeys` StorageMap のバリュー |
| PAPI / runtime API | `DmMetaAddress` 構造体 (metadata v16 自動生成型) | フロントエンドからの読み書き |
| wasm-engine / UI 層 | 文字列 `st:anarchy:<Base58(spend_pubkey || view_pubkey)>` | 既存 `format_meta_address` / `parse_meta_address` が扱う形式 |

**変換責務**:
- **wasm-engine 側**: 既存 `stealth::address::parse_meta_address(&str) -> MetaAddressParts { spend_pubkey, view_pubkey }` を流用。DM モジュールは `MetaAddressParts` を受け取って `DmMetaAddress` 相当の 2 本のバイト列で内部処理する。
- **フロントエンド側**: 新規 `apps/frontend/src/lib/dm/api.ts` に以下のコンバータを実装:
  ```typescript
  function dmMetaFromString(s: string): { scan_pub: Uint8Array; spend_pub: Uint8Array };
  function dmMetaToString(m: { scan_pub: Uint8Array; spend_pub: Uint8Array }): string;
  ```
  既存 `lib/stealth` に同等ロジックがあればそれを import 経由で使用 (DRY)。
- **publish_dm_key extrinsic 呼出時**: UI → `dmMetaFromString` → PAPI の `DmMetaAddress` 型で tx を組み立てる。逆方向 (on-chain 読み出し→UI 表示) は `dmMetaToString` でユーザー向けに Base58 文字列化。

**フィールド名の不一致について**: 既存 stealth 側の `view_pubkey` / `spend_pubkey` と本 pallet の `scan_pub` / `spend_pub` は**概念同値・バイト同値**。改名は既存の前方互換性 (016-stealth-address の wasm-engine 境界) を壊すため行わず、layer 境界で命名差を吸収する。ドキュメント上はこの等価関係を明記し、将来の混乱を防ぐ。

### 1.2 `DmContentRef`

分散ストレージに保存された DM ciphertext への参照。`PostContent` と同構造だが、用途を区別するため独立型とする（将来フィールド分岐が発生したときに `PostContent` を汚染しないため）。

```rust
#[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, PartialEq, Eq)]
pub struct DmContentRef {
    /// MerkleRoot (Blake2b-256)
    pub root: [u8; 32],
    /// 復元に必要な最小断片数
    pub k: u32,
    /// 総断片数
    pub n: u32,
    /// 暗号文サイズ (パディング後、FR-026 のバケット値のいずれか)
    pub ciphertext_len: u64,
}

/// プロトコルバージョンはタイプレベルの定数とする (M2 対応)。
/// 将来 v2 以降を導入する場合は、`send_dm_v2` 新 extrinsic を call_index 3 で追加し、
/// `DmContentRefV2` 構造体を並行定義する (既存 v1 は変更しない = 後方互換)。
pub const DM_PROTOCOL_VERSION: u8 = 1;
```

**Validation**:
- `k > 0 && k <= n && n <= 255`
- `ciphertext_len` が FR-026 のバケット集合 `{1_024, 4_096, 16_384, 65_536, 262_144}` のいずれかと等しい (R4 で 256B バケットは不採用に決定済み)
- protocol version は本 pallet の MVP = v1 のみ (`DM_PROTOCOL_VERSION = 1`、拡張時は新 extrinsic と新構造体で並行定義。M2 参照)

### 1.3 `DmDispatch`

`send_dm` 成立時にブロック単位で積み上げる ephemeral pubkey + stealth address + content ref のエントリ。受信者スキャナがこれを舐めて自分宛を検出する。

```rust
#[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, PartialEq, Eq)]
pub struct DmDispatch<AccountId> {
    /// 受信者 stealth address (送信者が ephemeral_pub × recipient_scan_pub から導出)
    pub recipient_stealth: AccountId,
    /// sender が生成した ephemeral 公開鍵
    pub ephemeral_pubkey: [u8; 32],
    /// 分散ストレージ参照
    pub content: DmContentRef,
}
```

### 1.4 Storage Items

```text
pallet-messaging Storage:
  DmReceptionKeys: StorageMap<AccountId, DmMetaAddress>
    - キー: 受信者メインアカウント
    - 値: 公開メタアドレス
    - 書込: publish_dm_key (メインアカウント署名)
    - 削除: revoke_dm_key (メインアカウント署名)

  DmDispatchesByBlock: StorageMap<BlockNumber, BoundedVec<DmDispatch<AccountId>, MaxDispatchesPerBlock>>
    - キー: ブロック番号
    - 値: 当該ブロックで発行された DM のエントリリスト
    - 書込: send_dm (sender_stealth アカウント署名)
    - MaxDispatchesPerBlock: 256 (pallet-stealth の MaxEntriesPerBlock と同等スケール)

  NextMessageId: StorageValue<u64, ValueQuery>
    - 内部的なメッセージ連番 (イベント発火に使う。プライバシーを損なわないよう、単調増加のみで participant と紐付かない)

  DmMessagesByRoot: StorageMap<[u8; 32], u64>
    - キー: MerkleRoot
    - 値: 発行時に割り当てた message_id
    - 用途: **storage-layer replay 防止のみ** (同一 MerkleRoot を持つ ciphertext が 2 回
      `send_dm` されることを pallet レベルで拒否する)。
    - スコープの限界: ユーザー再送 UX で平文または ephemeral が 1 bit でも変われば
      ciphertext 全体が変わり、結果として MerkleRoot も変わる。その場合は別メッセージと
      して扱われる。すなわち本 StorageMap は「ユーザー意図の重複送信」を防ぐものではなく、
      「storage-node に同一フラグメント群が再登録されるのを防ぐ + チェーン状態の一意性を
      保つ」ための低レベル防御。
    - ユーザー視点の重複送信防止 (連打) は `apps/frontend/src/lib/dm/sender.ts` の UX
      (送信ボタン disable + in-flight tx トラッキング) で担保する。
    - **永続肥大化の懸念 (M1 対応)**: 1 エントリ = 40 B (32 B MerkleRoot + 8 B message_id)。
      mainnet で年間 1000 万 DM 発行を仮定すると年間 400 MB 増加。MVP では on_idle フックも
      GC も持たないため単調増加する。Phase 3.4 の popularity-driven GC (FR-018) 実装時に
      `on_initialize` で「GC 済み DmDispatch に対応する DmMessagesByRoot エントリを
      削除する」ロジックを同時に導入することで、storage 肥大を DmDispatchesByBlock の
      ライフサイクルに追随させる。Phase 3.4 まではノード運用側で archival node 相当の
      disk 計画を前提とする (apps/blockchain 側で運用メモに追記予定)。
```

### 1.5 Runtime Constants (`Config` パラメータ)

```text
type MaxDispatchesPerBlock: Get<u32>  // 256
type DmBaseCost: Get<Balance>          // 例: 1 MORAL (base fee)
type DmByteCost: Get<Balance>          // 例: 0.05 MORAL/byte (post の 0.1 の半分)
type MaxDmCiphertextLen: Get<u64>      // 262_144 (= 256KB バケット上限)
type StoragePool: StorageInterface     // post と同じ 80% 流入先
type BurnRatio: Get<Permill>           // 10% (post と同じ)
type StealthRewardRatio: Get<Permill>  // 10% (stealth reward pool への還流、post の Reaction 相当)
```

### 1.6 Events / Errors

```rust
#[pallet::event]
pub enum Event<T: Config> {
    /// DM メタアドレスが公開された
    DmKeyPublished { account: T::AccountId },
    /// DM メタアドレスが取り消された
    DmKeyRevoked { account: T::AccountId },
    /// DM が発行された
    DmDispatched {
        message_id: u64,
        block_number: BlockNumberFor<T>,
        recipient_stealth: T::AccountId,
        ephemeral_pubkey: [u8; 32],
        content_hash: [u8; 32],   // MerkleRoot
    },
}

#[pallet::error]
pub enum Error<T> {
    /// 受信者が DM 受信鍵を公開していない
    ReceptionKeyNotPublished,
    /// ciphertext_len がパディングバケットに一致しない
    InvalidPaddingBucket,
    /// 同一 MerkleRoot が既に存在 (重複送信)
    DuplicateContent,
    /// 当ブロックのエントリ上限超過
    TooManyDispatchesInBlock,
    /// k/n パラメータが不正
    InvalidKNParameters,
    /// 送信者 (stealth account) の残高不足
    InsufficientStealthBalance,
    /// 無効なメタアドレス (all-zero pubkey 等)
    InvalidMetaAddress,
}
```

### 1.7 State Transitions (エンティティライフサイクル)

**DmReceptionKey**
```text
  [absent] --publish_dm_key--> [published]
  [published] --publish_dm_key (更新)--> [published (new key)]
  [published] --revoke_dm_key--> [absent]
```

**DmDispatch (個別 DM)**
```text
  [absent] --send_dm--> [committed in DmDispatchesByBlock]
  [committed] --(retained)--> [committed]  // MVP では削除されない
  [committed] --(Phase 3.4 GC)--> [garbage_collected]  // FR-018, 将来
```

Phase 3.4 実装時に `DmDispatchesByBlock` のエントリ削除フック（block weight 制約を踏まえた `on_idle` 等）を追加することで、オンチェーン storage をリリース可能にする。MVP ではフックを用意しない。

---

## 2. オフチェーン型 (`wasm-engine::dm`)

### 2.1 `DmEnvelope` (ciphertext の内側)

復号後に受信側がパースする構造。

```rust
#[derive(Encode, Decode, Debug, Clone)]
pub struct DmEnvelope {
    /// プロトコルバージョン (MVP = 1)
    pub version: u8,
    /// 送信者メインアカウント (AccountId32 エンコード)
    pub sender_account: [u8; 32],
    /// 送信時刻 (送信者ローカルの UNIX ms; 相対信頼、検証には使わず表示用)
    pub timestamp_ms: u64,
    /// 本文 (UTF-8 テキスト or バイナリ; FR-017 により opaque)
    pub body: Vec<u8>,
    /// Sr25519 署名 (sender のメインアカウント秘密鍵による)
    /// 署名対象: blake2b_256(
    ///   version || sender_account || recipient_stealth || ephemeral_pubkey || timestamp_ms || body_hash
    /// )
    pub signature: [u8; 64],
}
```

### 2.2 パディングバケット

```rust
pub const DM_PADDING_BUCKETS: [usize; 5] = [1_024, 4_096, 16_384, 65_536, 262_144];

/// Raw plaintext (envelope + ISO 7816-4 padding) を与え、
/// AES-GCM tag 16 バイトを足した総 ciphertext_len がバケット値に収まる最小の
/// バケットを返す。収まらなければ None (呼び出し側が BodyTooLarge エラー)。
pub fn select_padding_bucket(padded_plaintext_len: usize) -> Option<usize> {
    DM_PADDING_BUCKETS
        .iter()
        .copied()
        .find(|&b| padded_plaintext_len + 16 <= b)
}
```

### 2.3 暗号化 / 復号フロー

**encrypt (送信側)**
```text
Input:  recipient_meta_address (DmMetaAddress),
        sender_main_signer (ウォレット signer - 秘密鍵はアプリに出さない)
        body (Vec<u8>, plaintext)

1. 新鮮な X25519 ephemeral keypair (eph_pub, eph_priv) を生成
2. recipient_stealth_pubkey = dm_derive_recipient_stealth(
       scan_pub    = recipient_meta_address.scan_pub,   // X25519
       spend_pub   = recipient_meta_address.spend_pub,  // Ed25519 compressed
       eph_pub,
       eph_priv,                                          // 関数内で DH 計算に使用
   )   // W5 新設ラッパ。内部は既存 stealth::address::derive_stealth_address と同じ演算
       // P_stealth = K_spend + H(X25519(eph_priv, K_scan)) * G (Ed25519 Edwards point)
       // 返値は 32 バイト compressed Edwards point (= AccountId32 互換)
3. shared = X25519(eph_priv, recipient_meta_address.scan_pub)
4. hkdf_okm = HKDF-SHA256(
       salt = b"anarchy-dm-v1",
       ikm  = shared,
       info = recipient_stealth_pubkey || eph_pub
   )  -> 44 bytes (32 key + 12 nonce)
5. inner_signed_hash = blake2b_256(
       0x01 || sender_main_account || recipient_stealth_pubkey || eph_pub
         || timestamp_ms.to_le_bytes() || blake2b_256(body)
   )
   signature = sender_main_signer.sign(inner_signed_hash)   // Sr25519 (wallet signer)
6. envelope = DmEnvelope {
       version: 1,
       sender_account: sender_main_account,
       timestamp_ms,
       body,
       signature,                                             // 64 bytes
   }
7. padded_plaintext = pad_iso7816_4(
       encode(envelope),
       select_padding_bucket(encode(envelope).len() + 1 /* terminator */)
         .ok_or(BodyTooLarge)? - 16 /* AES-GCM tag */
   )
8. ciphertext = AES-256-GCM.encrypt(
       key  = hkdf_okm[0..32],
       nonce = hkdf_okm[32..44],
       aad   = recipient_stealth_pubkey || eph_pub
               || (padded_plaintext.len() as u32 BE) || 0x01 /* protocol_version */,
       plaintext = padded_plaintext,
   )  // 出力 ciphertext.len() == bucket
9. eph_priv をゼロクリア (FR-021)
10. Fragment ciphertext via SSS+Merkle (既存 pipeline)
11. Upload fragments to storage-node 群 → MerkleRoot / k / n / ciphertext_len 確定
12. Return (recipient_stealth_pubkey, eph_pub, content_ref)
```

**decrypt (受信側スキャン)**
```text
Input:  eph_pub (32B), ciphertext (再構成済み),
        own_scan_priv (X25519, 32B), own_spend_pub (Ed25519, 32B),
        purported_recipient_stealth_pubkey (on-chain AccountId32 の生バイト, 32B)

1. // 既存 packages/wasm-engine/src/stealth/scan.rs::scan_transaction と同じ式
   expected_stealth_pubkey = K_spend + H(X25519(own_scan_priv, eph_pub)) * G
2. if expected_stealth_pubkey != purported_recipient_stealth_pubkey: return None  // 自分宛でない
3. shared = X25519(own_scan_priv, eph_pub)
4. hkdf_okm = HKDF-SHA256(
       salt = b"anarchy-dm-v1",
       ikm  = shared,
       info = purported_recipient_stealth_pubkey || eph_pub
   )
5. padded_plaintext = AES-256-GCM.decrypt(
       key   = hkdf_okm[0..32],
       nonce = hkdf_okm[32..44],
       aad   = purported_recipient_stealth_pubkey || eph_pub
               || (padded_plaintext.len() as u32 BE) || 0x01,
       ciphertext,
   )   // 失敗なら None (改ざん検出)
6. envelope_bytes = strip_iso7816_4(padded_plaintext)
7. envelope = DmEnvelope::decode(envelope_bytes)
8. signed_hash = blake2b_256(
       0x01 || envelope.sender_account || purported_recipient_stealth_pubkey
         || eph_pub || envelope.timestamp_ms.to_le_bytes()
         || blake2b_256(envelope.body)
   )
9. signature_valid = sr25519_verify(envelope.signature, signed_hash, envelope.sender_account)
   // 失敗時でも decode 結果は返す (signature_valid フィールドに false を立てる)。
   // 呼び出し側 (sender.ts) が false のメッセージを UI から除外する (FR-004)。
10. Return Some(DmDecryptedEnvelope {
       sender_main_account: envelope.sender_account,
       timestamp_ms: envelope.timestamp_ms,
       body: envelope.body,
       signature_valid,
   })
```

**注**: 受信者が stealth の Ed25519 秘密鍵 (`k_spend + H(shared) mod L`) を再構成して tx 署名したい場合は別ユーティリティ `dm_derive_recipient_stealth_seckey(own_spend_priv, own_scan_priv, eph_pub)` を用意する (MVP の DM 受信フローでは tx 発行が発生しないため未使用だが、将来の `claim`/`transfer` に備えてヘルパだけ提供)。

### 2.4 ローカル永続化 (ブラウザ IndexedDB)

MVP で保存するのは**インデックスのみ**（鍵マテリアルはセッションメモリ）。

```typescript
interface DmScanIndex {
  lastScannedBlock: number;
  conversations: Map<SenderAccount, ConversationState>;
}

interface ConversationState {
  counterparty: AccountId;          // 送信者メインアカウント (復号後に判明)
  messages: DmMessageRecord[];
  unreadCount: number;
  blocked: boolean;
}

interface DmMessageRecord {
  messageId: number;                // pallet-messaging が発火する連番 (参考)
  blockNumber: number;
  direction: "incoming" | "outgoing";
  counterparty: AccountId;
  timestampMs: number;              // envelope 由来 (incoming) or 送信時刻 (outgoing)
  body: Uint8Array;                 // 復号済み本文
  deliveryState: "sent" | "delivered" | "read";  // outgoing のみ、incoming は不要
}
```

**Persistence Rules**:
- `body` はセッション中のみメモリに置き、IndexedDB には暗号化して格納 (wasm-engine の既存 `stealth::backup` 相当の AES-GCM ラッパを流用)。
- バックアップエクスポート時はこの DB を丸ごと AES-GCM + PBKDF2 100k で封入して 1 ファイルにする (FR-022)。

**バックアップスキーマ** (`.dm-backup.bin` の decrypted payload):

```typescript
interface DmBackup {
  version: 1;                              // バックアップフォーマットバージョン
  dm_meta: {
    scan_priv: Uint8Array;                 // 32 B (X25519 secret)
    spend_priv: Uint8Array;                // 32 B (Ed25519 secret, 将来の stealth 署名用)
    spend_pub: Uint8Array;                 // 32 B (Ed25519 compressed, 冗長だが復元時検証用)
    published_at_block: bigint;            // 最初に publish_dm_key した block 番号
  };
  dm_scan_index: {
    lastScannedBlock: bigint;              // この端末がスキャン済みの最終 block
    conversations: SerializedConversation[]; // 会話ごとに (counterparty, messages[], blocked, unreadCount)
  };
  dm_block_list: {
    version: 1;
    entries: AccountIdString[];            // SS58 文字列
  };
  exported_at_ms: number;                  // この backup を作成した unix ms
  device_id: string;                       // ランダム UUID (マージ時の tie-breaker)
}
```

**新端末での復元 & マージ規則** (FR-022 + SC-004 の整合):

新端末でバックアップをインポートしたとき、`dm_scan_index.lastScannedBlock` を起点にした差分スキャンで最新まで追いつく。これにより SC-004 (1000 会話 ≤ 3 秒) を新端末でも達成する。具体手順:

1. **鍵材料の取り込み**: `dm_meta` を Zustand store にロード。IndexedDB に暗号化して永続化。
2. **スキャン開始点**: `lastScannedBlock` が存在すればそれ以降を差分スキャン。存在しない (初回エクスポートが壊れている等の例外ケース) ときのみ `published_at_block` にフォールバック。ゼロから舐め直すことは**しない**。
3. **会話マージ** (既に別端末で会話がある状態でバックアップ A をインポート):
   - 受信メッセージ (`direction === "incoming"`) は `messageId + blockNumber` を主キーとして集合和 (重複は後入れ優先で無視)。
   - 送信メッセージ (`direction === "outgoing"`) はその端末でしか知り得ない情報なので、バックアップ側に存在すれば追加、新端末側と重複したら `exported_at_ms` と `device_id` で tie-breaker。
   - `blocked`: OR (どちらかでブロックされていれば結合後もブロック扱い)。
   - `unreadCount`: 再計算 (マージ後の会話内で `messageId > lastReadMessageId` の incoming 件数)。
4. **スキャン追いつき**: マージ後に `scanDmInbox({ fromBlock: max(lastScannedBlock_backup, lastScannedBlock_current) + 1 })` を 1 回だけ実行。以降は通常の Web Worker スキャンサイクル。

**SC-004 達成の根拠**:
- バックアップの `dm_scan_index` があれば、初回ロード時に IndexedDB から 1000 会話分のインデックスを SELECT するだけで済む (フルスキャン不要)。
- 実装上は IndexedDB の object store `dm_conversations` に `(counterparty)` キー + compound index `(blockNumber, counterparty)` を張り、`openCursor` で 1000 件取得 → Zustand へ投入する経路。計測ターゲットは 3 秒中 500ms 以内。

### 2.5 Block List

```typescript
interface BlockList {
  version: 1;
  entries: AccountId[];  // ブロック済みメインアカウント
}
```

バックアップファイル内に `dm_block_list` として同梱。オンチェーンには書き込まない (R9)。

---

## 3. 外部接点まとめ (Cross-Reference)

| FR | モデル上の実装箇所 |
|----|-------------------|
| FR-001 | `send_dm` extrinsic (on-chain) + `sender.ts` orchestrator (frontend) |
| FR-002 | `DmEnvelope` inside AES-256-GCM ciphertext |
| FR-003 | `DmDispatchesByBlock`: recipient は常に stealth address |
| FR-004 | `DmEnvelope.signature` (Sr25519 over envelope hash) |
| FR-005 | `DmBaseCost` + `DmByteCost` × `ciphertext_len` |
| FR-006 | `Error::ReceptionKeyNotPublished`, `Error::InsufficientStealthBalance` |
| FR-007/008 | ローカル `ConversationState` |
| FR-009 | `DmDispatchesByBlock` に commit されれば honest client は eventually 発見 |
| FR-010 | モデル上に削除 extrinsic なし (意図) |
| FR-011 | ローカル `BlockList` |
| FR-012 | `decrypt()` ステップ 1–2 (expected_stealth 照合) |
| FR-013 | `MaxDmCiphertextLen` + `InvalidPaddingBucket` |
| FR-014 | wasm-engine 内で鍵操作完結 |
| FR-015 | `publish_dm_key` / `revoke_dm_key` extrinsics |
| FR-016 | `DmMessageRecord.deliveryState` (ローカル派生) |
| FR-017 | `DmEnvelope.body: Vec<u8>` (opaque bytes) |
| FR-018 | `DmDispatchesByBlock` は MVP 非削除、Phase 3.4 hook 予約 |
| FR-019 | モデル上 group chat の型を一切持たない (1:1 専用) |
| FR-020 | `eph_pub` per-message + HKDF 派生鍵 |
| FR-021 | クライアントコード側で `eph_priv` を即時破棄 (テスト必須) |
| FR-022 | backup ファイル (AES-GCM + PBKDF2) が DM 鍵と `DmScanIndex` を同梱 |
| FR-023 | UI コンポーネント `MissingBackupNotice` (後述の contract) |
| FR-024 | `sender.ts` の 2 段 tx フロー |
| FR-025 | UI コンポーネント `PreFundStep` (後述) |
| FR-026 | `DM_PADDING_BUCKETS` + `pad_iso7816_4` |
| FR-027 | モデル上には存在しない (明示的に out-of-scope) |
