/**
 * <MissingBackupNotice /> — DM 鍵未ロード時の案内 (T062 / FR-023)。
 *
 * 仕様: contracts/frontend-ui.md §2.6。
 *  - onOpenSettings で /dm/settings に誘導。新規発行・インポート両方を同画面に集約する。
 *  - 警告ではなく情報メッセージのトーン。
 */

'use client';

import { useLocale } from '@/i18n';
import styles from './MissingBackupNotice.module.css';

export interface MissingBackupNoticeProps {
  onOpenSettings?: () => void;
}

export function MissingBackupNotice({ onOpenSettings }: MissingBackupNoticeProps): JSX.Element {
  const { t } = useLocale();
  return (
    <section className={styles.notice} role="status">
      <h3 className={styles.title}>{t('dm.missingKey.title')}</h3>
      <p className={styles.description}>{t('dm.missingKey.description')}</p>
      <div className={styles.actions}>
        {onOpenSettings ? (
          <button type="button" onClick={onOpenSettings} className={styles.primaryBtn}>
            {t('dm.missingKey.openSettings')}
          </button>
        ) : null}
      </div>
    </section>
  );
}
