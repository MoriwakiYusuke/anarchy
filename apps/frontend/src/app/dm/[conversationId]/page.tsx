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
import { MessageComposer } from '@/components/dm/MessageComposer';
import { hydrateDmStoreFromIndexedDb } from '@/lib/dm/persistence';
import type { SendDmContext } from '@/lib/dm/sender';
import type { AccountId } from '@/lib/dm/types';
import type { PolkadotSigner } from 'polkadot-api/signer';

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

  useEffect(() => {
    void (async () => {
      const s: PolkadotSigner | null = await createSigner('//Alice');
      if (s) setSigner(s);
    })();
  }, [createSigner]);

  useEffect(() => {
    void hydrateDmStoreFromIndexedDb();
  }, []);

  const sendCtx: SendDmContext | null = useMemo(() => {
    if (!unsafeApi || !signer) return null;
    return {
      api: unsafeApi,
      mainSigner: signer,
      mainAccountPublicKey: new Uint8Array(signer.publicKey),
      storageEndpoint: STORAGE_ENDPOINT,
    };
  }, [unsafeApi, signer]);

  const keyLoaded = stealthKeyManager.getMetaAddress() !== null;
  if (!keyLoaded) {
    router.replace('/dm');
    return <main><p>リダイレクト中…</p></main>;
  }

  return (
    <main className="dm-conversation-page">
      <header>
        <Link href="/dm">← インボックスへ戻る</Link>
      </header>

      <ConversationView conversationId={conversationId} />

      {sendCtx ? (
        <div
          className="dm-conversation-page__composer"
          style={{ position: 'sticky', bottom: 0 }}
        >
          <MessageComposer counterparty={conversationId} context={sendCtx} />
        </div>
      ) : (
        <p>接続中…</p>
      )}
    </main>
  );
}
