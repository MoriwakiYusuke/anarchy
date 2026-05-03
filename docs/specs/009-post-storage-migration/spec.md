# Feature Specification: Post Storage Migration（オンチェーン・ダイエット）

**Feature Branch**: `009-post-storage-migration`  
**Created**: 2026-02-10  
**Status**: Draft  
**Input**: User description: "Phase 1.5: Post Storage Migration - 投稿データをチェーンから分散ストレージへ移行"

## 概要

投稿コンテンツをブロックチェーンから分散ストレージへ移行し、オンチェーンストレージコストを削減しつつ大容量コンテンツに対応する。シャミアの秘密分散(SSS)とマークルツリーを組み合わせ、データの冗長性と検証可能性を両立させる。

## User Scenarios & Testing *(mandatory)*

### User Story 1 - 投稿作成（新フロー）(Priority: P1)

投稿者がコンテンツを作成すると、フロントエンドでデータが自動的にn個の断片に分割・暗号化され、複数のストレージノードへ並列アップロードされる。チェーンにはマークルルートとメタデータのみが記録される。

**Why this priority**: 本機能の中核。これなしには他の機能が動作しない。

**Independent Test**: テストネットで投稿を作成し、チェーン上にコンテンツ本体が保存されず、MerkleRootのみが記録されることを確認。

**Acceptance Scenarios**:

1. **Given** ユーザーが投稿フォームにテキストを入力, **When** 投稿ボタンを押す, **Then** Wasmエンジンがデータをn個の断片に分割し、各断片がストレージノードにアップロードされる
2. **Given** 断片が全ストレージノードに保存された, **When** Post Pallet呼び出し, **Then** チェーンにMerkleRoot、k、n、サイズのみが記録される
3. **Given** ネットワークエラーで一部ノードへのアップロードが失敗, **When** k個以上のノードに保存成功, **Then** 投稿は成功として完了する

---

### User Story 2 - 投稿表示（断片取得・復元）(Priority: P1)

ユーザーがタイムラインを閲覧すると、フロントエンドがストレージノードからk個以上の断片を取得し、クライアント側で元のコンテンツを復元して表示する。

**Why this priority**: 投稿作成と対で必要。閲覧できなければ投稿機能が無意味。

**Independent Test**: 既存の分散保存された投稿を表示し、元のコンテンツが正しく復元されることを確認。

**Acceptance Scenarios**:

1. **Given** 投稿がn個の断片として分散保存されている, **When** タイムラインを表示, **Then** k個以上の断片を取得し、SSSで元データを復元して表示
2. **Given** n個中一部のノードがオフライン, **When** k個以上のノードがオンライン, **Then** コンテンツは正常に復元・表示される
3. **Given** 利用可能なノードがk個未満, **When** コンテンツ取得試行, **Then** 「一時的に表示できません」エラーを表示

---

### User Story 3 - Storage Node断片受信・保持表明 (Priority: P1)

ストレージノードがBlockchain Nodeから検証済み断片を受信し、ローカル保存後にチェーンにdeclare_holdingを送信する。

**Why this priority**: ストレージノード側の実装なしにはシステムが成立しない。

**Independent Test**: Blockchain Node経由で断片をアップロードし、declare_holdingがチェーンに記録されることを確認。

**Acceptance Scenarios**:

1. **Given** Blockchain Nodeが検証済み断片をlibp2pで転送, **When** Storage Nodeが受信, **Then** 断片をローカルディスクに保存
2. **Given** 断片保存完了, **When** 保存成功, **Then** チェーンにdeclare_holdingを自動送信
3. **Given** ディスク容量不足, **When** 保存失敗, **Then** エラーレスポンスをBlockchain Nodeに返す

---

### User Story 4 - ローカルキャッシュによる高速表示 (Priority: P2)

頻繁にアクセスされるコンテンツをフロントエンドがローカルにキャッシュし、再訪問時に高速表示する。

**Why this priority**: UX向上機能だが、基本機能より優先度低い。

**Independent Test**: 同じ投稿を2回表示し、2回目はネットワークリクエストなしで即座に表示されることを確認。

**Acceptance Scenarios**:

1. **Given** 投稿を初めて表示, **When** 復元成功, **Then** 復元データをIndexedDB/localStorageにキャッシュ
2. **Given** キャッシュ済み投稿を再表示, **When** キャッシュヒット, **Then** ネットワークアクセスなしで即座に表示
3. **Given** キャッシュ容量が上限に達した, **When** 新しいコンテンツをキャッシュ, **Then** LRUポリシーで古いキャッシュを削除

---

### Edge Cases

- 全ストレージノードがオフラインの場合、投稿作成は失敗する（キューイングはスコープ外）
- 断片サイズが極端に小さい（< 32 bytes）場合、分割せず単一断片として扱う
- ネットワーク遅延による重複アップロードはMerkleRoot + FragmentIndexで重複排除
- 大容量コンテンツ（> 1MB）はチャンク分割後にSSS適用
- **アップロード失敗時は3回リトライ後、別のStorage Nodeへフォールバック**

## Requirements *(mandatory)*

### Functional Requirements

#### Post Pallet改修

- **FR-001**: システムは`Contents<T>` StorageMapを完全に削除し、`ContentRefs<T>: StorageMap<PostId, PostContent>`に置き換えなければならない
- **FR-002**: `create_post`トランザクションは`merkle_root`, `k`（しきい値）, `n`（分割数）, `total_size`をパラメータとして受け取らなければならない
- **FR-003**: 投稿コストは「基本料金 + サイズ係数 + Storage報酬デポジット」で計算されなければならない（デポジットはdeclare_holding手数料に充当）

#### Blockchain Node カスタムRPC拡張

- **FR-004**: Blockchain Nodeは`storage_uploadFragment(postId, index, data, proof)` RPCを提供しなければならない
- **FR-005**: Blockchain Nodeは受信した断片のBlake2bハッシュを計算し、添付されたMerkleProofで検証しなければならない
- **FR-006**: MerkleProof検証成功時、Blockchain NodeはStorage Nodeにlibp2p経由で断片を転送しなければならない
- **FR-007**: Blockchain Nodeは`storage_getFragment(postId, index)` RPCを提供し、Storage Nodeから断片を取得して返却しなければならない
- **FR-008**: Blockchain Nodeは`storage_listHolders(fragmentId)` RPCを提供し、チェーン上の保持者リストを返却しなければならない

#### Storage Node拡張

- **FR-009**: Storage Nodeはlibp2p request-responseで断片アップロードリクエストを受け付けなければならない
- **FR-010**: Storage Nodeは受信した断片をローカルディスクに保存しなければならない（検証はBlockchain Node側で完了済み）
- **FR-011**: 断片保存後、Storage Nodeはsubxt経由でチェーンに`declare_holding`を自動送信しなければならない
- **FR-012**: Storage Nodeはlibp2p request-responseで断片取得リクエストに応答しなければならない

#### Wasm暗号エンジン

- **FR-013**: システムはシャミアの秘密分散（k-of-n）でデータを分割する関数を提供しなければならない
- **FR-014**: システムはk個以上の断片からデータを復元する関数を提供しなければならない
- **FR-015**: システムはn個の断片からマークルツリーを構築し、MerkleRootを計算しなければならない
- **FR-016**: 各断片に対するMerkleProofを生成できなければならない
- **FR-017**: Wasmモジュールとしてエクスポートされ、ブラウザで実行可能でなければならない

#### フロントエンド改修

- **FR-018**: 投稿作成時、コンテンツをWasmエンジンでn個の断片に分割しなければならない
- **FR-019**: 断片をPAPI経由で`storage_uploadFragment` RPCを呼び出してアップロードしなければならない
- **FR-020**: k個以上のアップロード成功を確認後、Post PalletをPAPI経由で呼び出さなければならない
- **FR-021**: 投稿表示時、PAPI経由で`storage_getFragment` RPCを呼び出してk個以上の断片を取得し復元しなければならない
- **FR-022**: 頻繁アクセスコンテンツをローカルキャッシュし、再表示時に利用しなければならない

### Key Entities

- **MerkleRoot**: 32バイトのBlake2bハッシュ。n個の断片ハッシュから構築されたマークルツリーのルート
- **Fragment**: 秘密分散された断片データ。FragmentIndex（0〜n-1）で識別
- **MerkleProof**: 特定の断片がMerkleRootに含まれることを証明するハッシュの配列
- **PostMetadata V2**: MerkleRoot, k, n, TotalSizeを含む投稿メタデータ（V1はContentsを直接保持）

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 投稿作成コストが現行の50%以下に削減される（バイト単価廃止による）
- **SC-002**: 1MB以上の大容量投稿が作成可能になる（現行は約64KB制限）
- **SC-003**: 投稿作成から表示まで5秒以内で完了する（通常ネットワーク環境）
- **SC-004**: k個のストレージノードがオンラインであれば100%のコンテンツ可用性を維持
- **SC-005**: 2回目以降の同一投稿表示はキャッシュにより1秒以内

## Assumptions

- 初期デフォルト値: k=3, n=5（3-of-5で復元可能）**固定値としてシステム設定のみ変更可、ユーザーカスタマイズ不可**
- 断片最大サイズ: 256KB（設定変更可能）
- マイグレーション戦略: 開発環境のため既存データは破棄、V2形式のみサポート（後方互換性不要）
- ストレージノードは既にPhase 1で構築済み（008-distributed-storage）
- WasmエンジンはブラウザのメインスレッドではなくWeb Workerで実行
- フロントエンドはBlockchain Nodeのみに接続（PAPI経由で統一）
- Blockchain NodeがカスタムRPCでStorage Nodeへのプロキシとして機能
- Storage Node間はlibp2pで通信（既存のまま）

## Design Notes

### アーキテクチャ

```
┌─────────────────┐    WS JSON-RPC     ┌──────────────────┐    libp2p      ┌──────────────────┐
│   Frontend      │ ◀────────────────▶ │  Blockchain Node │ ◀────────────▶ │  Blockchain Node │
│   (Browser)     │     (PAPI)         │  + カスタムRPC    │  (sc-network)  │                  │
└─────────────────┘                    └────────┬─────────┘                └──────────────────┘
                                        │
                                        │ libp2p (request-response)
                                        ▼
                           ┌─────────────────┐    libp2p      ┌──────────────────┐
                           │  Storage Node   │ ◀────────────▶ │  Storage Node    │
                           │                 │                │                  │
                           └─────────────────┘                └──────────────────┘
```

- **フロントエンド**: Blockchain Nodeのみに接続（PAPI経由で統一）
- **Blockchain Node**: カスタムRPCでStorage操作を提供、内部でlibp2p経由でStorage Nodeに転送
- **Blockchain Node間**: libp2p（sc-network経由、GossipSub/GRANDPA/Kademlia）
- **Blockchain Node → Storage Node**: libp2p（request-response）
- **Storage Node間**: libp2p（request-response）

### Blockchain Node カスタムRPC

PAPI経由でフロントエンドから呼び出し可能：

- `storage_uploadFragment(postId, index, data, proof)` → 内部でStorage Nodeに転送
- `storage_getFragment(postId, index)` → Storage Nodeから取得して返却
- `storage_listHolders(fragmentId)` → チェーン上の保持者リストを返却

### データ構造（V2オンリー）

開発環境のため後方互換性は不要。V2形式のみをサポート：

```rust
struct PostContent {
    root: [u8; 32],  // MerkleRoot
    k: u32,          // しきい値
    n: u32,          // 分割数
    size: u64,       // 元データサイズ
}
```

Enum不要でシンプルな構造体に。マイグレーション時は既存の`Contents<T>`を単純削除。

### マークルツリーの粒度

- n=5〜32程度ならツリー深さは3〜5段、検証負荷は無視できるレベル
- n=100以上の将来拡張時は、深さ7段（128リーフ）まで許容
- 葉（Leaf）は「断片全体のBlake2bハッシュ」とする（シンプルさ重視）

### チャンク分割戦略

1MB超のデータを扱う場合の方針：
- SSSに投入する前に固定サイズチャンク（例: 64KB）に分割
- 1投稿 = 1マークルツリー、葉 = 「断片ごとのハッシュ」（チャンクごとではない）
- 大容量データは複数の断片に分散され、各断片が独立してMerkleProof検証可能

## Dependencies

- **008-distributed-storage Phase 1**: Storage Pallet MVP、Storage Node Daemon MVP（完了済み）
- **pallet-post**: 現行のPost Pallet（改修対象）
- **packages/wasm-engine**: 新規作成するWasm暗号エンジン

## Clarifications

### Session 2026-02-10

- Q: 投稿作成時のk/n値はユーザーがカスタマイズ可能か？ → A: 固定値（k=3, n=5）、システム設定のみ変更可
- Q: 断片（Fragment）の最大サイズ制限は？ → A: 256KB（後から設定変更可能）
- Q: Storage Nodeへのアップロード失敗時のリトライ戦略は？ → A: 3回リトライ後、別のStorage Nodeへフォールバック
- Q: declare_holdingトランザクションの手数料負担は？ → A: 投稿コストの一部から引き落とし（Storage報酬プールにデポジット）
- Q: 投稿コストの構成比率は？ → A: 基本料金50% : サイズ係数30% : Storage報酬デポジット20%
