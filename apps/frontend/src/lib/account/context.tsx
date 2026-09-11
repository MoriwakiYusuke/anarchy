'use client';

/**
 * AccountContext — connected wallet (account / signer / mainRawSigner) を
 * 全アプリから読めるようにする React Context。
 *
 * **Seed 取り扱い (CLAUDE.md Security Principle #2、2026-09-12 改訂)**:
 * `setAccount(seed)` の `seed` は signer 2 種 (`signer` = PAPI, `mainRawSigner` = raw
 * sr25519) を導出した後 React state には載せない。ただしリロード後に接続状態を
 * 復帰させるため、`lib/account/sessionStore` (IndexedDB) に account + seed を
 * **平文で保存** する。切断 (`setAccount(null, null)`) で必ず消す。
 * 復帰中は `isRestoring` が true になり、WalletConnect は接続フォームを出さない。
 *
 * アカウント変更 (null → A → B → null) に追従して
 * `stealthKeyManager` と `useDmStore` を破棄する責務もここで負う。
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
import type { StorageSigner } from '@/lib/dm/sender';
import { clearSession, loadSession, saveSession } from '@/lib/account/sessionStore';

export interface AccountContextValue {
  /** 接続中のアカウント SS58 アドレス。未接続なら null。 */
  account: string | null;
  /** 接続中アカウント由来の polkadot-api signer。 */
  signer: PolkadotSigner | null;
  /** 接続中アカウント由来の raw sr25519 signer (DM `inner_signed_hash` 署名用)。 */
  mainRawSigner: StorageSigner | null;
  /** 接続。WalletConnect が呼び出す。seed 付きなら sessionStore に保存、null なら削除。 */
  setAccount: (account: string | null, accountSeed: string | null) => void;
  /** マウント直後、sessionStore からの復帰が終わるまで true。 */
  isRestoring: boolean;
}

const AccountContext = createContext<AccountContextValue | null>(null);

export function AccountProvider({ children }: PropsWithChildren): JSX.Element {
  const { createSigner } = useApi();
  const [account, setAccountState] = useState<string | null>(null);
  const [signer, setSigner] = useState<PolkadotSigner | null>(null);
  const [mainRawSigner, setMainRawSigner] = useState<StorageSigner | null>(null);
  const [isRestoring, setIsRestoring] = useState(true);
  const previousAccountRef = useRef<string | null>(null);

  // account が変化したら DM 関連 state を破棄する。
  // null → null は無視、初回マウント (前回 null + 初期値 null) も無視。
  useEffect(() => {
    const prev = previousAccountRef.current;
    if (prev === account) return;
    if (prev !== null) {
      stealthKeyManager.destroy();
      useDmStore.getState().resetForAccountChange();
    }
    previousAccountRef.current = account;
  }, [account]);

  // signer 導出本体。persist=true で sessionStore へ保存 (復帰時は保存し直さない)。
  const applyAccount = useCallback(
    (acct: string | null, seed: string | null, persist: boolean) => {
      setAccountState(acct);
      if (!seed || !acct) {
        setSigner(null);
        setMainRawSigner(null);
        void clearSession();
        return;
      }
      if (persist) void saveSession({ account: acct, seed });
      // seed → signer 2 種を導出。完了後、closure の `seed` は GC 対象。
      // setSeed state には載せないので React Devtools / time-travel debug にも残らない。
      void (async () => {
        const [paiPolkaSigner, raw] = await Promise.all([
          createSigner(seed),
          (async (): Promise<StorageSigner> => {
            const { cryptoWaitReady } = await import('@polkadot/util-crypto');
            await cryptoWaitReady();
            const { Keyring } = await import('@polkadot/keyring');
            const keyring = new Keyring({ type: 'sr25519' });
            const pair = keyring.addFromUri(seed);
            return {
              publicKey: pair.publicKey,
              sign: (msg: Uint8Array) => pair.sign(msg),
            };
          })(),
        ]);
        // `seed` 文字列は両 signer に取り込まれた時点でこの closure を抜けると
        // 参照を失う。signer / pair は内部的に raw bytes に展開済み。
        setSigner(paiPolkaSigner);
        setMainRawSigner(raw);
      })();
    },
    [createSigner],
  );

  const setAccount = useCallback(
    (acct: string | null, seed: string | null) => applyAccount(acct, seed, true),
    [applyAccount],
  );

  // マウント時に前回のログイン状態を復帰する
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const saved = await loadSession();
        if (!cancelled && saved) applyAccount(saved.account, saved.seed, false);
      } finally {
        if (!cancelled) setIsRestoring(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [applyAccount]);

  const value = useMemo(
    () => ({ account, signer, mainRawSigner, setAccount, isRestoring }),
    [account, signer, mainRawSigner, setAccount, isRestoring],
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
