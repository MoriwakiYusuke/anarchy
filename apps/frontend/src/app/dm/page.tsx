/**
 * Direct Messages ページ (T049 → T065 改稿)。
 *
 * 役割:
 * - 鍵が stealth key manager に無い = <MissingBackupNotice /> (FR-023)。
 *   "インポート" は /dm/settings への誘導、"新規発行" はそのページで DmKeyManager を使う。
 * - 鍵がある: <ConversationList /> + "新規 DM" リンク。ループスキャン開始。
 *
 * 設定系 (鍵公開 / block リスト) は /dm/settings (T067) に分離、会話本体は
 * /dm/[conversationId] (T066) に遷移する。本ページはインボックスのトップレベルのみ。
 */

'use client';

import { useEffect, useRef, useState } from 'react';
import Link from 'next/link';
import { useRouter } from 'next/navigation';
import { useSmoldot } from '@/hooks/useSmoldot';
import { useAccount } from '@/lib/account/context';
import { stealthKeyManager } from '@/lib/stealth/keyManager';
import { getDmMetaAddressFromStealth } from '@/lib/dm/keyManager';
import { initSs58Toolkit, type ScanContext } from '@/lib/dm/scanner';
import type { SendDmContext, StorageSigner } from '@/lib/dm/sender';
import { sendDmReceipt } from '@/lib/dm/receipt';
import { startDmScanLoop, type DmScanLoopHandle } from '@/lib/dm/worker';
import { useDmStore } from '@/lib/dm/store';
import { ConversationList } from '@/components/dm/ConversationList';
import { MissingBackupNotice } from '@/components/dm/MissingBackupNotice';
import type { AccountId } from '@/lib/dm/types';
import styles from './page.module.css';

export default function DmPage(): JSX.Element {
  const { unsafeApi } = useSmoldot();
  const { account, accountSeed, signer } = useAccount();
  const router = useRouter();
  const [mainRawSigner, setMainRawSigner] = useState<StorageSigner | null>(null);
  const [keyLoaded, setKeyLoaded] = useState(false);
  const [newRecipient, setNewRecipient] = useState('');

  const lastScannedBlock = useDmStore((s: { lastScannedBlock: bigint }) => s.lastScannedBlock);
  const isScanning = useDmStore((s: { isScanning: boolean }) => s.isScanning);
  const loopRef = useRef<DmScanLoopHandle | null>(null);

  // inner_signed_hash (W6) は raw sr25519 で署名する必要がある (PolkadotSigner.signBytes は
  // <Bytes> wrap してしまい受信側 dm_decrypt_scan が拒否する)。
  // accountSeed (= 接続中アカウントの seed phrase / derive path) から keyring pair を作る。
  useEffect(() => {
    if (!accountSeed) {
      setMainRawSigner(null);
      return;
    }
    let cancelled = false;
    void (async () => {
      const { cryptoWaitReady } = await import('@polkadot/util-crypto');
      await cryptoWaitReady();
      const { Keyring } = await import('@polkadot/keyring');
      const keyring = new Keyring({ type: 'sr25519' });
      const pair = keyring.addFromUri(accountSeed);
      if (cancelled) return;
      setMainRawSigner({
        publicKey: pair.publicKey,
        sign: (msg: Uint8Array) => pair.sign(msg),
      });
    })();
    return () => {
      cancelled = true;
    };
  }, [accountSeed]);

  // stealth 鍵のロード状態を監視。session memory なので beforeunload 時に消える。
  useEffect(() => {
    const check = (): void =>
      setKeyLoaded(stealthKeyManager.getMetaAddress() !== null);
    check();
    const id = window.setInterval(check, 1_000);
    return () => window.clearInterval(id);
  }, []);

  useEffect(() => {
    void initSs58Toolkit();
  }, []);

  // 受信ループ: 鍵 + api + signer + mainRawSigner が揃ったら起動。
  // ストレージアクセスは chain-node 経由 (CLAUDE.md Security Principle #5)。
  // (hydrate + persistence subscription は /dm/layout.tsx が面倒を見る。)
  useEffect(() => {
    if (!keyLoaded || !unsafeApi || !signer || !mainRawSigner) return;
    const sendCtx: SendDmContext = {
      api: unsafeApi,
      mainSigner: signer,
      mainAccountPublicKey: new Uint8Array(signer.publicKey),
      mainRawSigner,
    };
    const handle = startDmScanLoop({
      buildContext: (): ScanContext | null => {
        const meta = getDmMetaAddressFromStealth();
        const scanPriv = stealthKeyManager.getViewKey();
        if (!meta || !scanPriv) return null;
        return {
          api: unsafeApi,
          ownScanPriv: new Uint8Array(scanPriv),
          ownSpendPub: meta.spendPub,
          ownMainAccount: '' as AccountId,
          lastScannedBlock: useDmStore.getState().lastScannedBlock,
        };
      },
      onNewIncoming: (msg) => {
        void sendDmReceipt(
          { counterparty: msg.counterparty, refMessageId: msg.messageId, kind: 'delivered' },
          sendCtx,
        ).catch((err) => console.error('[dm-receipt][delivered]', err));
      },
    });
    loopRef.current = handle;
    return () => {
      handle.stop();
      loopRef.current = null;
    };
  }, [keyLoaded, unsafeApi, signer, mainRawSigner]);

  return (
    <main className={styles.main}>
      <header className={styles.header}>
        <h1 className={styles.title}>
          <span className={styles.accent}>D</span>irect Messages
        </h1>
        <nav aria-label="DM navigation" className={styles.nav}>
          <Link href="/" className={styles.navLink}>
            ← Home
          </Link>
          <Link href="/dm/settings" className={styles.navLink}>
            設定
          </Link>
        </nav>
      </header>

      {!account ? (
        <div className={styles.status}>
          ウォレットを接続してください (ホーム画面の WalletConnect)。
        </div>
      ) : !keyLoaded ? (
        <MissingBackupNotice onOpenSettings={() => router.push('/dm/settings')} />
      ) : (
        <>
          <div className={styles.status}>
            <span className={isScanning ? styles.scanning : styles.idle}>
              {isScanning ? '● スキャン中' : '○ 待機中'}
            </span>
            <span className={styles.blockNumber}>
              直近ブロック #{lastScannedBlock.toString()}
            </span>
          </div>
          <section className={styles.composeSection}>
            <label className={styles.composeLabel} htmlFor="dm-new-recipient">
              新しい DM
            </label>
            <form
              className={styles.composeRow}
              onSubmit={(e) => {
                e.preventDefault();
                const trimmed = newRecipient.trim();
                if (!trimmed) return;
                router.push(`/dm/${encodeURIComponent(trimmed)}`);
              }}
            >
              <input
                id="dm-new-recipient"
                type="text"
                className={styles.composeInput}
                placeholder="相手の SS58 アドレス (5...)"
                value={newRecipient}
                onChange={(e) => setNewRecipient(e.target.value)}
              />
              <button
                type="submit"
                className={styles.composeBtn}
                disabled={!newRecipient.trim()}
              >
                開く
              </button>
            </form>
          </section>
          <div className={styles.listWrapper}>
            <ConversationList
              onSelect={(counterparty) =>
                router.push(`/dm/${encodeURIComponent(counterparty)}`)
              }
            />
          </div>
        </>
      )}
    </main>
  );
}
