/**
 * DM scanner (T045) — `DmScanApi::dispatches_range` を 1024 ブロックずつページングし、
 * `dm_decrypt_scan` で自分宛 dispatch を復号する。
 *
 * Contract: contracts/frontend-ui.md §1.2 / contracts/wasm-engine-dm-api.md §W2.
 */

import type {
  AccountId,
  DmContentRef,
  DmDispatch,
  DmMessageRecord,
  ScanDmResult,
} from './types';

type WasmModule = typeof import('anarchy-wasm-engine');
let wasmModule: WasmModule | null = null;

async function getWasm(): Promise<WasmModule> {
  if (wasmModule) return wasmModule;
  const module = await import('anarchy-wasm-engine');
  if (typeof window !== 'undefined' && typeof module.initSync === 'function') {
    const wasmUrl = new URL('/wasm/anarchy_wasm_engine_bg.wasm', window.location.origin);
    const response = await fetch(wasmUrl);
    if (!response.ok) {
      throw new Error(`Failed to fetch WASM: ${response.status} ${response.statusText}`);
    }
    const wasmBytes = await response.arrayBuffer();
    module.initSync({ module: new WebAssembly.Module(wasmBytes) });
  }
  wasmModule = module;
  return wasmModule;
}

/**
 * 1024 ブロックずつのページサイズ。`DmScanApi::dispatches_range` の上限と一致。
 */
export const SCAN_PAGE_SIZE = 1024n;

interface DmScanPapi {
  apis: {
    DmScanApi: {
      dispatches_range: (
        from: bigint,
        to: bigint,
      ) => Promise<Array<[bigint, DmDispatch[]]>>;
    };
  };
  query: {
    System: {
      Number: { getValue: () => Promise<bigint | number> };
    };
  };
}

/**
 * 1 件の dispatch を `dm_decrypt_scan` にかけ、自分宛なら `DmMessageRecord` に
 * 変換する。fragments の取得 (storage-node からの再構成) は本関数の責務外で、
 * `bodyState: 'plaintext'` のまま envelope に格納された body を返す。後続の T093/T094
 * (US2) で「fragment 不足→ garbage_collected」分岐を追加する想定。
 */
export interface ScanContext {
  /** PAPI unsafeApi。`apis.DmScanApi.dispatches_range` と `query.System.Number` を持つ。 */
  api: unknown;
  /** 受信者 X25519 scan secret (32B)。stealth keyManager から取得。 */
  ownScanPriv: Uint8Array;
  /** 受信者 Ed25519 spend public (32B)。stealth keyManager から取得。 */
  ownSpendPub: Uint8Array;
  /** 受信者メインアカウント (counterparty 集計用キー)。実体はメッセージ毎に
   *  envelope.sender_main_account から復元する。 */
  ownMainAccount: AccountId;
  /** 直前にスキャン済みのブロック (lastScannedBlock)。次回はこの +1 から開始。 */
  lastScannedBlock: bigint;
  /** スキャン上限 (省略時は best head)。テスト時に上限を固定するために露出。 */
  toBlockOverride?: bigint;
}

/**
 * `scanDmInbox` — 指定範囲の DM dispatch を解決する。
 *
 * - `lastScannedBlock + 1` から best head までを 1024 ブロックずつページング。
 * - 各 dispatch を `dm_decrypt_scan` にかけ、`Some` かつ `signature_valid == true`
 *   のものだけ採用 (FR-004)。
 * - 戻り値は 1 回の呼出でスキャンできた範囲と新着メッセージ。lastScannedBlock の
 *   永続化は呼出側 (worker / store) の責務。
 */
export async function scanDmInbox(ctx: ScanContext): Promise<ScanDmResult> {
  const wasm = await getWasm();
  const typed = ctx.api as DmScanPapi;

  // best head の決定。BigInt または number で返ってくる可能性に両対応。
  let bestHead: bigint;
  if (ctx.toBlockOverride !== undefined) {
    bestHead = ctx.toBlockOverride;
  } else {
    const raw = await typed.query.System.Number.getValue();
    bestHead = typeof raw === 'bigint' ? raw : BigInt(raw);
  }

  const fromBlock = ctx.lastScannedBlock + 1n;
  if (bestHead < fromBlock) {
    return {
      scannedFromBlock: fromBlock,
      scannedToBlock: ctx.lastScannedBlock,
      newMessages: [],
    };
  }

  const newMessages: DmMessageRecord[] = [];
  let cursor = fromBlock;
  while (cursor <= bestHead) {
    const pageEnd = cursor + SCAN_PAGE_SIZE - 1n > bestHead
      ? bestHead
      : cursor + SCAN_PAGE_SIZE - 1n;
    const page = await typed.apis.DmScanApi.dispatches_range(cursor, pageEnd);

    for (const [blockNumber, dispatches] of page) {
      for (const dispatch of dispatches) {
        const withCipher = dispatch as DmDispatch & { ciphertext?: Uint8Array };
        if (!withCipher.ciphertext) {
          // T094: ciphertext を再構成できない (storage-node から取得不能 / GC 済み) ケース。
          // 自分宛か判定できないため、宛先 placeholder として MerkleRoot 由来の counterparty を使う。
          // UI 側 (T094 GarbageCollectedBubble) は body を一切表示しないため、平文漏洩リスクは無い。
          newMessages.push({
            messageId: deriveMessageId(blockNumber, dispatch),
            blockNumber: BigInt(blockNumber),
            direction: 'incoming',
            counterparty: gcPlaceholderCounterparty(dispatch.content.root),
            timestampMs: 0,
            body: new Uint8Array(),
            bodyState: 'garbage_collected',
            signatureValid: false,
          });
          continue;
        }

        const decrypted = decryptOne(wasm, ctx, withCipher);
        if (!decrypted) continue;
        if (!decrypted.signature_valid) continue; // FR-004

        newMessages.push({
          messageId: deriveMessageId(blockNumber, dispatch),
          blockNumber: BigInt(blockNumber),
          direction: 'incoming',
          counterparty: bytesToSs58Sync(decrypted.sender_main_account),
          timestampMs: Number(decrypted.timestamp_ms),
          body: new Uint8Array(decrypted.body),
          bodyState: 'plaintext',
          signatureValid: true,
        });
      }
    }
    cursor = pageEnd + 1n;
  }

  return {
    scannedFromBlock: fromBlock,
    scannedToBlock: bestHead,
    newMessages,
  };
}

/**
 * Wasm `dm_decrypt_scan` ラッパ。失敗 (None / decode 失敗) は `null` を返す。
 *
 * **MVP 簡略化**: `ciphertext` は `DmContentRef.merkle_root` から storage-node 経由
 * で再構成すべきだが、現段階の `DmDispatch` は scanner 側に直接 ciphertext を
 * 配ってこないため、`content` から再ダウンロードするロジックは T094 (US2) で
 * 追加する。ここではテスト用に `dispatch` が ciphertext フィールドを持つ
 * 拡張形 (`DmDispatchWithCiphertext`) も受け付けて先行的に動作可能にしておく。
 */
function decryptOne(
  wasm: WasmModule,
  ctx: ScanContext,
  dispatch: DmDispatch & { ciphertext?: Uint8Array },
): {
  sender_main_account: Uint8Array;
  timestamp_ms: bigint;
  body: Uint8Array;
  signature_valid: boolean;
} | null {
  const ciphertext = dispatch.ciphertext;
  if (!ciphertext) {
    // 後続フェーズで storage-node からの再構成を実装。MVP では skip。
    return null;
  }
  const stealthBytes = ss58ToBytesSync(dispatch.recipientStealth);
  const result = wasm.dm_decrypt_scan(
    ctx.ownScanPriv,
    ctx.ownSpendPub,
    new Uint8Array(dispatch.ephemeralPubkey),
    stealthBytes,
    ciphertext,
  );
  if (!result) return null;
  return {
    sender_main_account: new Uint8Array(result.sender_main_account),
    timestamp_ms: result.timestamp_ms,
    body: new Uint8Array(result.body),
    signature_valid: result.signature_valid,
  };
}

/**
 * `DmDispatch` から「ブロック内で一意な messageId」を導出する。pallet 側 NextMessageId
 * を直接取得する API がまだ無いので、暫定的に `(blockNumber << 32) | merkleRootLo32`
 * のハッシュ的疑似 ID を使う。後続 (T077 等) で event ペイロードから取得する。
 */
function deriveMessageId(blockNumber: bigint, d: DmDispatch): bigint {
  const merkle = d.content.root;
  let lo = 0n;
  for (let i = 0; i < 8 && i < merkle.length; i += 1) lo = (lo << 8n) | BigInt(merkle[i]);
  return (blockNumber << 64n) | lo;
}

/**
 * GC 済み DM の counterparty placeholder。`gc:<root-prefix-hex>` 形式で
 * 一意性を維持する (UI 側で同じスレッドにまとめないために counterparty 単位で
 * 必ず別キー化する)。
 */
function gcPlaceholderCounterparty(root: Uint8Array): AccountId {
  const prefix = Array.from(root.slice(0, 8))
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('');
  return `gc:${prefix}` as AccountId;
}

// =============================================================================
// SS58 ↔ raw bytes (sender 側と同じく `@polkadot-api/substrate-bindings` を使う)
// scanner は同期パスで連続的に呼び出されるので、初回だけ async load → cache。
// =============================================================================

let ss58Toolkit: {
  toSs58: (b: Uint8Array) => string;
  fromSs58: (s: string) => Uint8Array;
} | null = null;

export async function initSs58Toolkit(): Promise<void> {
  if (ss58Toolkit) return;
  const { fromBufferToBase58, getSs58AddressInfo } =
    await import('@polkadot-api/substrate-bindings');
  ss58Toolkit = {
    toSs58: fromBufferToBase58(42),
    fromSs58: (s: string) => {
      const info = getSs58AddressInfo(s);
      if (!info.isValid) throw new Error(`invalid SS58: ${s}`);
      return new Uint8Array(info.publicKey);
    },
  };
}

function bytesToSs58Sync(bytes: Uint8Array): AccountId {
  if (!ss58Toolkit) throw new Error('ss58 toolkit not initialized; call initSs58Toolkit first');
  return ss58Toolkit.toSs58(bytes) as AccountId;
}

function ss58ToBytesSync(s: AccountId): Uint8Array {
  if (!ss58Toolkit) throw new Error('ss58 toolkit not initialized; call initSs58Toolkit first');
  return ss58Toolkit.fromSs58(s);
}

/**
 * テスト/型補強用: scanner が期待する DmDispatch 拡張型。MVP では ciphertext は
 * 別経路で結合する予定だが、T033 のテストはこの形でモックする。
 */
export type DmDispatchWithCiphertext = DmDispatch & { ciphertext: Uint8Array };

/**
 * 派生型のヘルパエクスポート (テスト用)。
 */
export type { DmContentRef };
