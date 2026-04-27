/**
 * T058: keyManager.exportDmBackup / importDmBackup の round-trip + マージ規則テスト。
 *
 * 確認:
 *   - exportDmBackup → importDmBackup でストアに鍵 / scan index / block list が復元される。
 *   - 既に別端末で会話がある状態でインポートすると incoming は和集合 + 重複なし、
 *     blocked は OR、unreadCount は再計算 (data-model.md §2.4)。
 *   - 不正パスワードでは BackupImportFailed が throw される。
 *
 * 注: stealth keyManager の getter (`getViewKey`, `getSpendKey`, `getViewPubkey`,
 * `getSpendPubkey`) を mock して鍵マテリアルだけ供給する。WebCrypto は jsdom + Node18+
 * の `webcrypto` を直接 globalThis.crypto に紐付ける。
 */

/* eslint-disable @typescript-eslint/no-explicit-any */

import { webcrypto } from 'crypto';
if (!(globalThis as any).crypto?.subtle) {
  Object.defineProperty(globalThis, 'crypto', {
    configurable: true,
    writable: true,
    value: webcrypto,
  });
}

const fakeKeys = {
  scan_priv: new Uint8Array(32).fill(0xa1),
  spend_priv: new Uint8Array(32).fill(0xb2),
  scan_pub: new Uint8Array(32).fill(0xc3),
  spend_pub: new Uint8Array(32).fill(0xd4),
};

let stealthLoaded = true;

jest.mock('../../stealth/keyManager', () => ({
  stealthKeyManager: {
    getViewKey: () => (stealthLoaded ? fakeKeys.scan_priv : null),
    getSpendKey: () => (stealthLoaded ? fakeKeys.spend_priv : null),
    getViewPubkey: () => (stealthLoaded ? fakeKeys.scan_pub : null),
    getSpendPubkey: () => (stealthLoaded ? fakeKeys.spend_pub : null),
    loadFromBackup: jest.fn((scanPriv: Uint8Array, spendPriv: Uint8Array) => {
      stealthLoaded = true;
      fakeKeys.scan_priv = new Uint8Array(scanPriv);
      fakeKeys.spend_priv = new Uint8Array(spendPriv);
    }),
  },
}));

import { exportDmBackup, importDmBackup } from '../keyManager';
import { useDmStore } from '../store';
import type { AccountId, DmMessageRecord } from '../types';
import { DmError } from '../types';

function msg(
  counterparty: string,
  blockNumber: bigint,
  messageId: bigint,
  body: string,
  direction: 'incoming' | 'outgoing' = 'incoming',
): DmMessageRecord {
  return {
    messageId,
    blockNumber,
    direction,
    counterparty: counterparty as AccountId,
    timestampMs: Number(blockNumber) * 6000,
    body: new TextEncoder().encode(body),
    bodyState: 'plaintext',
    signatureValid: direction === 'incoming' ? true : undefined,
  };
}

beforeEach(() => {
  stealthLoaded = true;
  fakeKeys.scan_priv = new Uint8Array(32).fill(0xa1);
  fakeKeys.spend_priv = new Uint8Array(32).fill(0xb2);
  useDmStore.setState({
    conversations: new Map(),
    blockList: new Set(),
    lastScannedBlock: 0n,
    isScanning: false,
  });
});

describe('keyManager backup export/import (T058 / FR-022)', () => {
  it('round-trips an empty store', async () => {
    const cipher = await exportDmBackup('correct-horse-battery-staple');

    // 状態をクリアした後でインポートし、scan index と block list が復元されることを確認。
    useDmStore.setState({
      conversations: new Map(),
      blockList: new Set(),
      lastScannedBlock: 100n, // インポート前の値は上書きされるはず。
      isScanning: false,
    });

    await importDmBackup(cipher, 'correct-horse-battery-staple');
    const s = useDmStore.getState();
    expect(s.conversations.size).toBe(0);
    expect(s.blockList.size).toBe(0);
    expect(s.lastScannedBlock).toBe(0n);
  });

  it('round-trips conversations + block list', async () => {
    useDmStore.getState().addIncoming(msg('5Alice', 10n, 1n, 'hi'));
    useDmStore.getState().addOutgoing(msg('5Bob', 11n, 2n, 'reply', 'outgoing'));
    useDmStore.getState().blockSender('5Charlie' as AccountId);
    useDmStore.setState({ lastScannedBlock: 50n });

    const cipher = await exportDmBackup('pw');

    useDmStore.setState({
      conversations: new Map(),
      blockList: new Set(),
      lastScannedBlock: 0n,
      isScanning: false,
    });

    await importDmBackup(cipher, 'pw');
    const s = useDmStore.getState();
    expect(s.conversations.size).toBe(2);
    expect(s.blockList.has('5Charlie' as AccountId)).toBe(true);
    expect(s.lastScannedBlock).toBe(50n);

    const aliceMsgs = s.conversations.get('5Alice' as AccountId)?.messages;
    expect(aliceMsgs).toHaveLength(1);
    expect(new TextDecoder().decode(aliceMsgs![0].body)).toBe('hi');
  });

  it('throws BackupImportFailed on wrong password', async () => {
    const cipher = await exportDmBackup('right-pw');
    useDmStore.setState({
      conversations: new Map(),
      blockList: new Set(),
      lastScannedBlock: 0n,
      isScanning: false,
    });

    await expect(importDmBackup(cipher, 'wrong-pw')).rejects.toThrow(
      DmError.BackupImportFailed,
    );
  });

  it('merges incoming messages by (blockNumber, messageId), no duplicates', async () => {
    // 端末 A: Alice から 1 件だけ既知。
    useDmStore.getState().addIncoming(msg('5Alice', 10n, 1n, 'hi-A'));
    // 端末 B のバックアップ: Alice から 2 件 (うち 1 件は重複)。
    useDmStore.setState({
      conversations: new Map([
        [
          '5Alice' as AccountId,
          {
            counterparty: '5Alice' as AccountId,
            messages: [
              msg('5Alice', 10n, 1n, 'hi-B'), // 同 (block, id) → 重複扱い (後入れ無視 = 既存優先)
              msg('5Alice', 12n, 2n, 'hi-B-2'),
            ],
            unreadCount: 2,
            blocked: false,
            lastActivityBlock: 12n,
          },
        ],
      ]),
      blockList: new Set(),
      lastScannedBlock: 12n,
    });
    const cipherFromB = await exportDmBackup('pw');

    // 端末 A の状態を再現。
    useDmStore.setState({
      conversations: new Map([
        [
          '5Alice' as AccountId,
          {
            counterparty: '5Alice' as AccountId,
            messages: [msg('5Alice', 10n, 1n, 'hi-A')],
            unreadCount: 1,
            blocked: false,
            lastActivityBlock: 10n,
          },
        ],
      ]),
      blockList: new Set(),
      lastScannedBlock: 10n,
    });

    await importDmBackup(cipherFromB, 'pw');
    const s = useDmStore.getState();
    const merged = s.conversations.get('5Alice' as AccountId);
    // 重複は除外、message_id 1 と 2 の 2 件が残る。
    expect(merged?.messages).toHaveLength(2);
    const ids = merged!.messages.map((m) => Number(m.messageId)).sort();
    expect(ids).toEqual([1, 2]);
    // lastScannedBlock は max(端末A=10, バックアップB=12) = 12 に追従。
    expect(s.lastScannedBlock).toBe(12n);
  });

  it('OR-merges blocked flag (either side blocked → blocked after merge)', async () => {
    useDmStore.setState({
      conversations: new Map([
        [
          '5Alice' as AccountId,
          {
            counterparty: '5Alice' as AccountId,
            messages: [],
            unreadCount: 0,
            blocked: true, // バックアップ側で blocked
            lastActivityBlock: 0n,
          },
        ],
      ]),
      blockList: new Set(['5Alice' as AccountId]),
      lastScannedBlock: 0n,
    });
    const cipher = await exportDmBackup('pw');

    // 端末側は blocked=false の状態。
    useDmStore.setState({
      conversations: new Map([
        [
          '5Alice' as AccountId,
          {
            counterparty: '5Alice' as AccountId,
            messages: [],
            unreadCount: 0,
            blocked: false,
            lastActivityBlock: 0n,
          },
        ],
      ]),
      blockList: new Set(),
      lastScannedBlock: 0n,
    });

    await importDmBackup(cipher, 'pw');
    const s = useDmStore.getState();
    expect(s.conversations.get('5Alice' as AccountId)?.blocked).toBe(true);
    expect(s.blockList.has('5Alice' as AccountId)).toBe(true);
  });
});
