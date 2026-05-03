'use client';

/**
 * AccountContext — connected wallet (account / signer / mainRawSigner) を
 * 全アプリから読めるようにする React Context。
 *
 * **Seed 取り扱い (CLAUDE.md Security Principle #2)**:
 * `setAccount(seed)` の `seed` 文字列は AccountProvider 内で
 * `signer` (polkadot-api PAPI) と `mainRawSigner` (raw sr25519 keyring pair)
 * の両方を導出した直後に **React state から破棄** する。これにより:
 *   - JS string は immutable で memset できないため、heap 上に
 *     V8 GC まで残る分は不可避だが、参照を握っている state を
 *     早期に外すことで GC が回収する確率を上げる
 *   - DmModal 等のコンシューマが accountSeed を再取得する経路を排除
 *     (= 各箇所で keyring.addFromUri(seed) を呼ばない)
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

export interface AccountContextValue {
  /** 接続中のアカウント SS58 アドレス。未接続なら null。 */
  account: string | null;
  /** 接続中アカウント由来の polkadot-api signer。 */
  signer: PolkadotSigner | null;
  /** 接続中アカウント由来の raw sr25519 signer (DM `inner_signed_hash` 署名用)。 */
  mainRawSigner: StorageSigner | null;
  /** 接続。WalletConnect が呼び出す。`seed` は処理直後に破棄される。 */
  setAccount: (account: string | null, accountSeed: string | null) => void;
}

const AccountContext = createContext<AccountContextValue | null>(null);

export function AccountProvider({ children }: PropsWithChildren): JSX.Element {
  const { createSigner } = useApi();
  const [account, setAccountState] = useState<string | null>(null);
  const [signer, setSigner] = useState<PolkadotSigner | null>(null);
  const [mainRawSigner, setMainRawSigner] = useState<StorageSigner | null>(null);
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

  const setAccount = useCallback(
    (acct: string | null, seed: string | null) => {
      setAccountState(acct);
      if (!seed || !acct) {
        setSigner(null);
        setMainRawSigner(null);
        return;
      }
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

  const value = useMemo(
    () => ({ account, signer, mainRawSigner, setAccount }),
    [account, signer, mainRawSigner, setAccount],
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
