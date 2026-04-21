'use client';

/**
 * useNicknameOf — read-only nickname lookup for display purposes.
 *
 * Unlike `useNickname` (which also handles set/clear for the current user),
 * this hook is optimized for rendering many addresses at once (e.g. DM
 * conversation list). It uses a module-level cache so repeated mounts of the
 * same accountId do not re-hit the chain, and deduplicates concurrent
 * requests for the same account.
 *
 * Cache is cleared on full page reload. Memory only — no IDB persistence
 * because nicknames can change server-side and we want to re-fetch on next
 * session.
 */

import { useEffect, useState } from 'react';
import { useSmoldot } from './useSmoldot';

type NicknameApi = {
  query: {
    Nickname: {
      Nicknames: {
        getValue: (accountId: string) => Promise<unknown>;
      };
    };
  };
};

const cache = new Map<string, string | null>();
const inflight = new Map<string, Promise<string | null>>();

function toNickname(result: unknown): string | null {
  if (!result) return null;
  let bytes: Uint8Array;
  if (typeof result === 'object' && result !== null && 'asBytes' in result) {
    const asBytes = (result as { asBytes?: () => Uint8Array }).asBytes;
    if (typeof asBytes === 'function') bytes = asBytes.call(result);
    else return null;
  } else if (result instanceof Uint8Array) {
    bytes = result;
  } else if (Array.isArray(result)) {
    bytes = new Uint8Array(result as number[]);
  } else {
    return null;
  }
  if (bytes.length === 0) return null;
  return new TextDecoder().decode(bytes);
}

async function fetchNickname(api: NicknameApi, accountId: string): Promise<string | null> {
  const existing = inflight.get(accountId);
  if (existing) return existing;
  const p = (async () => {
    try {
      const raw = await api.query.Nickname.Nicknames.getValue(accountId);
      const nick = toNickname(raw);
      cache.set(accountId, nick);
      return nick;
    } catch (err) {
      console.warn('[useNicknameOf] query failed', accountId, err);
      return null;
    } finally {
      inflight.delete(accountId);
    }
  })();
  inflight.set(accountId, p);
  return p;
}

/**
 * Returns the nickname for `accountId`, or null if unset / still loading.
 * Safe to call with null/empty accountId (returns null without hitting chain).
 */
export function useNicknameOf(accountId: string | null | undefined): string | null {
  const { unsafeApi } = useSmoldot();
  const [nickname, setNickname] = useState<string | null>(() =>
    accountId ? (cache.get(accountId) ?? null) : null,
  );

  useEffect(() => {
    if (!accountId || !unsafeApi) return;
    if (cache.has(accountId)) {
      setNickname(cache.get(accountId) ?? null);
      return;
    }
    let alive = true;
    void fetchNickname(unsafeApi as NicknameApi, accountId).then((n) => {
      if (alive) setNickname(n);
    });
    return () => {
      alive = false;
    };
  }, [accountId, unsafeApi]);

  return nickname;
}
