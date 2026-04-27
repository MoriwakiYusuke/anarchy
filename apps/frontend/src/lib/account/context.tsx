'use client';

/**
 * AccountContext — connected wallet (account / accountSeed / signer) を全アプリ
 * から読めるようにする React Context。
 *
 * これまで `account` / `accountSeed` は `app/page.tsx` の useState に閉じており、
 * `/dm/*` 以下のページから見えなかったため、DM 系は dev 便宜で
 * `createSigner('//Alice')` をハードコードしていた。Context 化して全ページで
 * 接続中アカウントを参照できるようにする。
 *
 * 加えて、アカウント変更 (null → A → B → null など) に追従して
 * `stealthKeyManager` と `useDmStore` を破棄する責務もここで負う:
 *  - stealth 鍵は session-only で **アカウントに紐付くべき秘密**
 *  - DM ストア (会話 / sentReceipts / blockList) も同様
 *
 * これらはモジュール singleton なので、アカウント切替時に明示クリアしないと
 * 「前アカウントの DM 状態が新アカウントに漏れる」バグになる。
 */

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type PropsWithChildren,
} from 'react';
import type { PolkadotSigner } from 'polkadot-api/signer';
import { useApi } from '@/hooks/useApi';
import { stealthKeyManager } from '@/lib/stealth/keyManager';
import { useDmStore } from '@/lib/dm/store';

export interface AccountContextValue {
  /** 接続中のアカウント SS58 アドレス。未接続なら null。 */
  account: string | null;
  /** 接続中のアカウント seed phrase / derive path。未接続なら null。 */
  accountSeed: string | null;
  /** 接続中アカウント由来の polkadot-api signer。アカウント or seed に追従して再生成。 */
  signer: PolkadotSigner | null;
  /** 接続。WalletConnect が呼び出す。 */
  setAccount: (account: string | null, accountSeed: string | null) => void;
}

const AccountContext = createContext<AccountContextValue | null>(null);

export function AccountProvider({ children }: PropsWithChildren): JSX.Element {
  const { createSigner } = useApi();
  const [account, setAccountState] = useState<string | null>(null);
  const [accountSeed, setAccountSeedState] = useState<string | null>(null);
  const [signer, setSigner] = useState<PolkadotSigner | null>(null);
  const previousAccountRef = useRef<string | null>(null);

  // accountSeed が変わったら signer を作り直す。
  useEffect(() => {
    if (!accountSeed) {
      setSigner(null);
      return;
    }
    let cancelled = false;
    void createSigner(accountSeed).then((next) => {
      if (!cancelled) setSigner(next);
    });
    return () => {
      cancelled = true;
    };
  }, [accountSeed, createSigner]);

  // account が変化したら DM 関連 state を破棄する。
  // null → null は無視、初回マウント (前回 null + 初期値 null) も無視。
  useEffect(() => {
    const prev = previousAccountRef.current;
    if (prev === account) return;
    if (prev !== null) {
      // 切替・解除のいずれもこの分岐 (前のアカウントの遺物を消す)。
      stealthKeyManager.destroy();
      useDmStore.getState().resetForAccountChange();
    }
    previousAccountRef.current = account;
  }, [account]);

  const setAccount = useCallback((acct: string | null, seed: string | null) => {
    setAccountState(acct);
    setAccountSeedState(seed);
  }, []);

  const value = useMemo(
    () => ({ account, accountSeed, signer, setAccount }),
    [account, accountSeed, signer, setAccount],
  );

  return <AccountContext.Provider value={value}>{children}</AccountContext.Provider>;
}

export function useAccount(): AccountContextValue {
  const ctx = useContext(AccountContext);
  if (!ctx) {
    throw new Error('useAccount must be used inside <AccountProvider>');
  }
  return ctx;
}
