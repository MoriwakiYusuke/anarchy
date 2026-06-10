/**
 * sentReceipts のプライバシー (receiptKey ハッシュ化) テスト。
 *
 * Asserts:
 *   - receiptKey が平文 "counterparty|messageId|kind" ではなく blake2b ハッシュ
 *     (32 hex chars) を返す — ディスクアクセスで DM の social graph が読めない。
 *   - 同一入力 → 同一キー (idempotent guard が機能する)、入力が 1 要素でも違えば別キー。
 *   - rememberReceiptSent が localStorage にハッシュ値のみを永続化する。
 *   - resetForAccountChange が永続化済みリストを削除する。
 *
 * Note: localStorage は jest.setup.ts でモック化済み (jest.fn ベース)。
 */

import { useDmStore, receiptKey } from '../store';
import type { AccountId } from '../types';

const ALICE = '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY' as AccountId;
const SENT_RECEIPTS_KEY = 'anarchy:dm:sentReceipts:v2';

beforeEach(() => {
  jest.clearAllMocks();
  useDmStore.setState({ sentReceipts: new Set<string>() });
});

describe('receiptKey (hashed)', () => {
  it('returns an opaque blake2b hex hash, not the plaintext metadata', () => {
    const key = receiptKey(ALICE, 42n, 'read');

    // 32 hex chars (blake2b 16-byte digest)
    expect(key).toMatch(/^[0-9a-f]{32}$/);
    // counterparty / messageId / kind の平文を含まない
    expect(key).not.toContain(ALICE);
    expect(key).not.toContain('42');
    expect(key).not.toContain('read');
    expect(key).not.toContain('|');
  });

  it('is deterministic for the same input', () => {
    expect(receiptKey(ALICE, 42n, 'read')).toBe(receiptKey(ALICE, 42n, 'read'));
  });

  it('differs when any component differs', () => {
    const base = receiptKey(ALICE, 42n, 'read');
    expect(receiptKey(ALICE, 42n, 'delivered')).not.toBe(base);
    expect(receiptKey(ALICE, 43n, 'read')).not.toBe(base);
    expect(receiptKey('5Bob' as AccountId, 42n, 'read')).not.toBe(base);
  });
});

describe('sentReceipts persistence', () => {
  it('persists only hashed keys to localStorage', () => {
    const key = receiptKey(ALICE, 7n, 'delivered');
    useDmStore.getState().rememberReceiptSent(key);

    expect(useDmStore.getState().sentReceipts.has(key)).toBe(true);

    const setItem = window.localStorage.setItem as jest.Mock;
    expect(setItem).toHaveBeenCalledWith(SENT_RECEIPTS_KEY, JSON.stringify([key]));
    // 永続化ペイロードに平文メタデータが含まれない
    const persisted = setItem.mock.calls.map((c: unknown[]) => String(c[1])).join('');
    expect(persisted).not.toContain(ALICE);
  });

  it('is a no-op for an already-remembered key (exactly once)', () => {
    const key = receiptKey(ALICE, 7n, 'delivered');
    useDmStore.getState().rememberReceiptSent(key);
    const setItem = window.localStorage.setItem as jest.Mock;
    const callsAfterFirst = setItem.mock.calls.length;

    useDmStore.getState().rememberReceiptSent(key);
    expect(setItem.mock.calls.length).toBe(callsAfterFirst);
    expect(useDmStore.getState().sentReceipts.size).toBe(1);
  });

  it('drops the persisted list on account change', () => {
    const key = receiptKey(ALICE, 7n, 'read');
    useDmStore.getState().rememberReceiptSent(key);

    useDmStore.getState().resetForAccountChange();

    expect(useDmStore.getState().sentReceipts.size).toBe(0);
    expect(window.localStorage.removeItem as jest.Mock).toHaveBeenCalledWith(
      SENT_RECEIPTS_KEY,
    );
  });
});
