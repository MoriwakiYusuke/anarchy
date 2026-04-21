/**
 * <MissingBackupNotice /> — DM 鍵未ロード時の案内 (T062 / FR-023)。
 *
 * 仕様: contracts/frontend-ui.md §2.6。
 *  - onOpenSettings で /dm/settings に誘導。新規発行・インポート両方を同画面に集約する。
 *  - 警告ではなく情報メッセージのトーン。
 */

'use client';

import styles from './MissingBackupNotice.module.css';

export interface MissingBackupNoticeProps {
  onOpenSettings?: () => void;
}

export function MissingBackupNotice({ onOpenSettings }: MissingBackupNoticeProps): JSX.Element {
  return (
    <section className={styles.notice} role="status">
      <h3 className={styles.title}>DM 鍵が読み込まれていません</h3>
      <p className={styles.description}>
        このブラウザでは DM を復号する鍵がまだ読み込まれていません。
        設定画面で新規発行するか、バックアップファイルをインポートしてください。
      </p>
      <div className={styles.actions}>
        {onOpenSettings ? (
          <button type="button" onClick={onOpenSettings} className={styles.primaryBtn}>
            DM 鍵設定を開く
          </button>
        ) : null}
      </div>
    </section>
  );
}
