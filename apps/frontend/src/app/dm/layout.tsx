'use client';

/**
 * /dm/* layout — IDB ハイドレート + persistence subscription を **将来の再有効化用に
 * 残しつつ、現状はトグルで無効化**する。
 *
 * 背景: stealth 鍵が session memory only であるのに対し、IDB に保存される
 * 復号済み plaintext bodies は永続化されており「鍵を捨てれば DM も読めなくなる」
 * という直感に反していた。プライバシー一貫性を優先し、現状は IDB を使わず
 * 毎セッションで chain → storage から再 scan / 再復号する設計に切り替える。
 *
 * ただし IDB 永続化の機構自体は将来の再導入 (例えばパスワード暗号化キャッシュ
 * への置換) のため `lib/dm/persistence.ts` ごと無傷で残す。再有効化したい
 * 場合はこの 1 箇所のフラグを `true` に切り替えるだけで戻る。
 */

import { useEffect, type ReactNode } from 'react';
import {
  hydrateDmStoreFromIndexedDb,
  startDmPersistenceSubscription,
} from '@/lib/dm/persistence';
import styles from './layout.module.css';

/**
 * IDB 永続化を有効にするかどうか。`false` のときはハイドレートも subscribe も
 * 行わず、ストアは毎セッション初期化される。`persistence.ts` 自体は残置。
 */
const USE_IDB_PERSISTENCE = false;

let hydrated = false;

export default function DmLayout({ children }: { children: ReactNode }): JSX.Element {
  useEffect(() => {
    if (!USE_IDB_PERSISTENCE) return;
    if (!hydrated) {
      hydrated = true;
      void hydrateDmStoreFromIndexedDb();
    }
    const stop = startDmPersistenceSubscription();
    return () => stop();
  }, []);

  return <div className={styles.wrapper}>{children}</div>;
}
