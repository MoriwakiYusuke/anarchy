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
import { useApi } from '@/hooks/useApi';
import { stealthKeyManager } from '@/lib/stealth/keyManager';
import { getDmMetaAddressFromStealth } from '@/lib/dm/keyManager';
import { initSs58Toolkit, type ScanContext } from '@/lib/dm/scanner';
import { startDmScanLoop, type DmScanLoopHandle } from '@/lib/dm/worker';
import { useDmStore } from '@/lib/dm/store';
import {
  hydrateDmStoreFromIndexedDb,
  startDmPersistenceSubscription,
} from '@/lib/dm/persistence';
import { ConversationList } from '@/components/dm/ConversationList';
import { MissingBackupNotice } from '@/components/dm/MissingBackupNotice';
import type { AccountId } from '@/lib/dm/types';
import type { PolkadotSigner } from 'polkadot-api/signer';

export default function DmPage(): JSX.Element {
  const { unsafeApi } = useSmoldot();
  const { createSigner } = useApi();
  const router = useRouter();
  const [signer, setSigner] = useState<PolkadotSigner | null>(null);
  const [keyLoaded, setKeyLoaded] = useState(false);

  const lastScannedBlock = useDmStore((s: { lastScannedBlock: bigint }) => s.lastScannedBlock);
  const isScanning = useDmStore((s: { isScanning: boolean }) => s.isScanning);
  const loopRef = useRef<DmScanLoopHandle | null>(null);

  // 開発便宜: //Alice をデフォルト signer に。本番は WalletConnect 等から。
  useEffect(() => {
    void (async () => {
      const s: PolkadotSigner | null = await createSigner('//Alice');
      if (s) setSigner(s);
    })();
  }, [createSigner]);

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

  // IDB から store を復元 + persistence subscription。
  useEffect(() => {
    void hydrateDmStoreFromIndexedDb();
    const stop = startDmPersistenceSubscription();
    return () => stop();
  }, []);

  // 受信ループ: 鍵 + api + signer の 3 点が揃ったら起動。
  useEffect(() => {
    if (!keyLoaded || !unsafeApi || !signer) return;
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
    });
    loopRef.current = handle;
    return () => {
      handle.stop();
      loopRef.current = null;
    };
  }, [keyLoaded, unsafeApi, signer]);

  return (
    <main>
      <header>
        <h1>Direct Messages</h1>
        <nav aria-label="DM navigation">
          <Link href="/dm/settings">設定</Link>
        </nav>
      </header>

      {!keyLoaded ? (
        <MissingBackupNotice
          onImport={() => router.push('/dm/settings')}
          onPublishNew={() => router.push('/dm/settings')}
        />
      ) : (
        <>
          <p>
            スキャン状態: {isScanning ? '実行中' : '待機中'} / 直近ブロック:{' '}
            {lastScannedBlock.toString()}
          </p>
          <ConversationList
            onSelect={(counterparty) =>
              router.push(`/dm/${encodeURIComponent(counterparty)}`)
            }
          />
        </>
      )}
    </main>
  );
}
