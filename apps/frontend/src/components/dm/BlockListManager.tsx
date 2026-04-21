/**
 * <BlockListManager /> — DM ブロック対象アカウントの一覧管理 (T061)。
 *
 * 仕様: contracts/frontend-ui.md §2.5 / FR-011。
 *  - dmStore.blockList の SS58 アカウント一覧を表示。
 *  - 解除ボタンで unblockSender を呼ぶ。
 *  - SS58 文字列を入力して追加するフォーム。
 *  - オンチェーンには書き込まない (R9: ローカルのみ)。
 */

'use client';

import { useState } from 'react';
import { useDmStore } from '@/lib/dm/store';
import type { AccountId } from '@/lib/dm/types';
import styles from './BlockListManager.module.css';

export function BlockListManager(): JSX.Element {
  const blockList = useDmStore((s: { blockList: Set<AccountId> }) => s.blockList);
  const blockSender = useDmStore(
    (s: { blockSender: (a: AccountId) => void }) => s.blockSender,
  );
  const unblockSender = useDmStore(
    (s: { unblockSender: (a: AccountId) => void }) => s.unblockSender,
  );
  const [input, setInput] = useState('');

  const entries = Array.from(blockList).sort();

  const handleAdd = (e: React.FormEvent): void => {
    e.preventDefault();
    const trimmed = input.trim();
    if (!trimmed) return;
    blockSender(trimmed as AccountId);
    setInput('');
  };

  return (
    <section className={styles.section}>
      <h3 className={styles.title}>ブロック中のアカウント</h3>
      {entries.length === 0 ? (
        <p className={styles.empty}>ブロック中のアカウントはありません。</p>
      ) : (
        <ul className={styles.list}>
          {entries.map((account) => (
            <li key={account} className={styles.item}>
              <span className={styles.account}>{account}</span>
              <button
                type="button"
                onClick={() => unblockSender(account)}
                className={styles.unblockBtn}
              >
                解除
              </button>
            </li>
          ))}
        </ul>
      )}
      <form onSubmit={handleAdd} className={styles.form}>
        <label className={styles.field}>
          <span className={styles.label}>ブロックする SS58 アドレス</span>
          <input
            type="text"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            placeholder="5..."
            className={styles.input}
          />
        </label>
        <button type="submit" disabled={!input.trim()} className={styles.addBtn}>
          追加
        </button>
      </form>
    </section>
  );
}
