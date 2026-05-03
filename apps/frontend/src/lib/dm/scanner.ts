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
import { decodeReceiptBody, type DmReceiptPayload } from './receipt';
import { resolveChainRpcEndpoint } from './sender';
import { fetchCiphertextFromStorage } from './storageFetch';

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
        from: number,
        to: number,
      ) => Promise<Array<[bigint | number, DmDispatch[]]>>;
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
  /** Chain-node JSON-RPC HTTP エンドポイント。`storage_getFragment` を叩く
   *  ため必要 (CLAUDE.md Security Principle #5)。設定されている場合、dispatch
   *  毎に ciphertext を再構成してから `dm_decrypt_scan` にかける。未設定 or
   *  取得失敗時はテスト用の `dispatch.ciphertext` フィールドへ fallback。 */
  chainRpcEndpoint?: string;
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
      newReceipts: [],
    };
  }

  const newMessages: DmMessageRecord[] = [];
  const newReceipts: ScanDmResult['newReceipts'] = [];
  let cursor = fromBlock;
  while (cursor <= bestHead) {
    const pageEnd = cursor + SCAN_PAGE_SIZE - 1n > bestHead
      ? bestHead
      : cursor + SCAN_PAGE_SIZE - 1n;
    const page = await typed.apis.DmScanApi.dispatches_range(
      Number(cursor),
      Number(pageEnd),
    );

    for (const [blockNumber, dispatches] of page) {
      const bn = typeof blockNumber === 'bigint' ? blockNumber : BigInt(blockNumber);
      for (const rawDispatch of dispatches) {
        const dispatch = normalizeDispatch(rawDispatch);
        const withCipher = dispatch as DmDispatch & { ciphertext?: Uint8Array };
        // ciphertext が dispatch に含まれない場合は chain-node 経由で再構成する。
        // ctx.chainRpcEndpoint が無ければ環境変数 / dev fallback で解決する。
        if (!withCipher.ciphertext) {
          const endpoint = resolveChainRpcEndpoint(ctx.chainRpcEndpoint);
          const reconstructed = await fetchCiphertextFromStorage(
            endpoint,
            new Uint8Array(dispatch.content.root),
            dispatch.content.n,
            Number(dispatch.content.ciphertextLen),
          );
          if (reconstructed) withCipher.ciphertext = reconstructed;
        }
        if (!withCipher.ciphertext) {
          // ciphertext が再構成できないと、その dispatch が本当に「自分宛」だったか
          // 判定する手段がない (recipient_stealth match の判定だけでは ECDH 公開鍵
          // から secret を得る攻撃を考慮すると不十分)。確証なしの GC placeholder を
          // すべての dispatch について作ると、他人宛の trafiic で inbox が汚染される。
          // → silent skip (受信できなかった事実は表示しない)。
          //
          // 真の "for-us GC" を後で表示したい場合は、wasm に
          // `dm_check_stealth_match(scan_priv, spend_pub, eph_pub, recipient_stealth)`
          // のような軽量チェック関数を追加して、自分宛確認後にだけ placeholder を
          // 出すリファクタリングが必要。
          continue;
        }

        const decrypted = decryptOne(wasm, ctx, withCipher);
        if (!decrypted) continue;
        if (!decrypted.signature_valid) continue; // FR-004

        const body = new Uint8Array(decrypted.body);
        const counterparty = bytesToSs58Sync(decrypted.sender_main_account);

        // T077: body が receipt フォーマットなら通常メッセージではなく receipt として分類。
        const receipt: DmReceiptPayload | null = decodeReceiptBody(body);
        if (receipt) {
          newReceipts.push({
            counterparty,
            refMessageId: receipt.refMessageId,
            kind: receipt.kind,
          });
          continue;
        }

        newMessages.push({
          messageId: deriveMessageId(bn, dispatch),
          blockNumber: bn,
          direction: 'incoming',
          counterparty,
          timestampMs: Number(decrypted.timestamp_ms),
          body,
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
    newReceipts,
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
 * PAPI が返す snake_case / Binary フィールドを TS 型 `DmDispatch` にそろえる。
 */
function normalizeDispatch(raw: unknown): DmDispatch & { ciphertext?: Uint8Array } {
  const asBytes = (v: unknown): Uint8Array => {
    if (v instanceof Uint8Array) return v;
    const b = v as { asBytes?: () => Uint8Array };
    if (b?.asBytes) return b.asBytes();
    return new Uint8Array();
  };
  const r = raw as {
    recipient_stealth?: string;
    recipientStealth?: string;
    ephemeral_pubkey?: unknown;
    ephemeralPubkey?: unknown;
    content?: {
      root?: unknown;
      k?: number;
      n?: number;
      ciphertext_len?: bigint | number;
      ciphertextLen?: bigint | number;
    };
    ciphertext?: unknown;
  };
  const contentRaw = r.content ?? {};
  return {
    recipientStealth: (r.recipient_stealth ?? r.recipientStealth ?? '') as AccountId,
    ephemeralPubkey: asBytes(r.ephemeral_pubkey ?? r.ephemeralPubkey),
    content: {
      root: asBytes(contentRaw.root),
      k: Number(contentRaw.k ?? 0),
      n: Number(contentRaw.n ?? 0),
      ciphertextLen: BigInt(contentRaw.ciphertext_len ?? contentRaw.ciphertextLen ?? 0),
    },
    ciphertext: r.ciphertext instanceof Uint8Array ? r.ciphertext : undefined,
  };
}

/**
 * `DmDispatch` から messageId を導出する。pallet 側 NextMessageId を直接取得する API が
 * まだ無いので、merkle_root の最初の 8 バイトを u64 として使う暫定 ID。receipt wire
 * format (`encodeReceiptBody`) が refMessageId を u64 で運ぶため、必ず u64 範囲内に
 * 収める必要がある。merkleRoot は Blake2b 出力で衝突耐性 2^-64 あり、実用上十分。
 */
function deriveMessageId(_blockNumber: bigint, d: DmDispatch): bigint {
  const merkle = d.content.root;
  let id = 0n;
  for (let i = 0; i < 8 && i < merkle.length; i += 1) id = (id << 8n) | BigInt(merkle[i]);
  return id;
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
