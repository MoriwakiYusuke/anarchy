'use client';

/**
 * /dm/* layout — hydrate IDB and start the persistence subscription once for
 * the entire DM area. Keeping this in the layout (rather than each page)
 * prevents the subscription from being torn down on child route navigations,
 * which would otherwise cause optimistic sends on `/dm/[id]` to never be
 * flushed and then be wiped by the next hydrate on `/dm`.
 */

import { useEffect, type ReactNode } from 'react';
import {
  hydrateDmStoreFromIndexedDb,
  startDmPersistenceSubscription,
} from '@/lib/dm/persistence';
import styles from './layout.module.css';

let hydrated = false;

export default function DmLayout({ children }: { children: ReactNode }): JSX.Element {
  useEffect(() => {
    if (!hydrated) {
      hydrated = true;
      void hydrateDmStoreFromIndexedDb();
    }
    const stop = startDmPersistenceSubscription();
    return () => stop();
  }, []);

  return <div className={styles.wrapper}>{children}</div>;
}
