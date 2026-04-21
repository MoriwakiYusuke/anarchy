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
import { useApi } from '@/hooks/useApi';
import { stealthKeyManager } from '@/lib/stealth/keyManager';
import { ConversationView } from '@/components/dm/ConversationView';
import type { SendDmContext, StorageSigner } from '@/lib/dm/sender';
import type { AccountId } from '@/lib/dm/types';
import type { PolkadotSigner } from 'polkadot-api/signer';
import styles from './page.module.css';

const STORAGE_ENDPOINT = process.env.NEXT_PUBLIC_STORAGE_ENDPOINT ?? 'http://127.0.0.1:3030';

export default function ConversationPage(): JSX.Element {
  const params = useParams<{ conversationId: string }>();
  const router = useRouter();
  const conversationId = useMemo<AccountId>(
    () => decodeURIComponent(params?.conversationId ?? '') as AccountId,
    [params],
  );

  const { unsafeApi } = useSmoldot();
  const { createSigner } = useApi();
  const [signer, setSigner] = useState<PolkadotSigner | null>(null);
  const [storageSigner, setStorageSigner] = useState<StorageSigner | null>(null);

  useEffect(() => {
    void (async () => {
      const s: PolkadotSigner | null = await createSigner('//Alice');
      if (s) setSigner(s);
    })();
  }, [createSigner]);

  // storage-node 認証用の raw sr25519 signer (「//Alice」を dev 用に使用)。
  // PolkadotSigner.signBytes は `<Bytes>` wrap をしてしまうため、@polkadot/keyring
  // の pair.sign をそのまま使う必要がある (contracts: X-Anarchy-Auth 仕様)。
  useEffect(() => {
    void (async () => {
      const { Keyring } = await import('@polkadot/keyring');
      const { DEV_PHRASE } = await import('@polkadot/keyring/defaults');
      const keyring = new Keyring({ type: 'sr25519' });
      const pair = keyring.addFromUri(`${DEV_PHRASE}//Alice`);
      setStorageSigner({
        publicKey: pair.publicKey,
        sign: (msg: Uint8Array) => pair.sign(msg),
      });
    })();
  }, []);

  const sendCtx: SendDmContext | null = useMemo(() => {
    if (!unsafeApi || !signer) return null;
    return {
      api: unsafeApi,
      mainSigner: signer,
      mainAccountPublicKey: new Uint8Array(signer.publicKey),
      // inner_signed_hash 用 raw sr25519 signer。storageSigner と同じ keyring pair を再利用。
      mainRawSigner: storageSigner ?? undefined,
      storageEndpoint: STORAGE_ENDPOINT,
      storageSigner: storageSigner ?? undefined,
    };
  }, [unsafeApi, signer, storageSigner]);

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
