/**
 * /dm/[conversationId] ページ (T066)。
 *
 * 役割: 選択済み counterparty の ConversationView を表示し、下部に MessageComposer を
 * 固定配置して返信可能にする。ルーティングは /dm から遷移する想定。
 */

'use client';

import { useEffect, useMemo, useState } from 'react';
import { useParams, useRouter } from 'next/navigation';
import Link from 'next/link';
import { useSmoldot } from '@/hooks/useSmoldot';
import { useAccount } from '@/lib/account/context';
import { stealthKeyManager } from '@/lib/stealth/keyManager';
import { ConversationView } from '@/components/dm/ConversationView';
import type { SendDmContext, StorageSigner } from '@/lib/dm/sender';
import type { AccountId } from '@/lib/dm/types';
import styles from './page.module.css';

export default function ConversationPage(): JSX.Element {
  const params = useParams<{ conversationId: string }>();
  const router = useRouter();
  const conversationId = useMemo<AccountId>(
    () => decodeURIComponent(params?.conversationId ?? '') as AccountId,
    [params],
  );

  const { unsafeApi } = useSmoldot();
  const { accountSeed, signer } = useAccount();
  const [mainRawSigner, setMainRawSigner] = useState<StorageSigner | null>(null);

  // inner_signed_hash (W6) 用の raw sr25519 signer は accountSeed から都度作る。
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

  const sendCtx: SendDmContext | null = useMemo(() => {
    if (!unsafeApi || !signer) return null;
    return {
      api: unsafeApi,
      mainSigner: signer,
      mainAccountPublicKey: new Uint8Array(signer.publicKey),
      mainRawSigner: mainRawSigner ?? undefined,
    };
  }, [unsafeApi, signer, mainRawSigner]);

  const keyLoaded = stealthKeyManager.getMetaAddress() !== null;
  useEffect(() => {
    if (!keyLoaded) router.replace('/dm');
  }, [keyLoaded, router]);
  if (!keyLoaded) {
    return (
      <main className={styles.main}>
        <p className={styles.loading}>リダイレクト中…</p>
      </main>
    );
  }

  return (
    <main className={styles.main}>
      <header className={styles.header}>
        <Link href="/dm" className={styles.backLink}>
          ← インボックスへ戻る
        </Link>
      </header>

      {sendCtx ? (
        <ConversationView conversationId={conversationId} context={sendCtx} />
      ) : (
        <>
          <ConversationView conversationId={conversationId} />
          <p className={styles.loading}>接続中…</p>
        </>
      )}
    </main>
  );
}
