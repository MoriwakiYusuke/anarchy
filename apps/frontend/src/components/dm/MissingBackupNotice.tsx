/**
 * <MissingBackupNotice /> — DM 鍵未ロード時の案内 (T062 / FR-023)。
 *
 * 仕様: contracts/frontend-ui.md §2.6。
 *  - props.onPublishNew / onImport をクリックで親側ハンドラに委譲。
 *  - 警告ではなく情報メッセージのトーン。
 */

'use client';

export interface MissingBackupNoticeProps {
  onPublishNew?: () => void;
  onImport?: () => void;
}

export function MissingBackupNotice({
  onPublishNew,
  onImport,
}: MissingBackupNoticeProps): JSX.Element {
  return (
    <section className="dm-missing-backup-notice" role="status">
      <h3>DM 鍵が読み込まれていません</h3>
      <p>
        このブラウザでは DM を復号する鍵がまだ読み込まれていません。
        既にバックアップファイルを持っている場合はインポート、
        新規利用なら DM 鍵を発行してください。
      </p>
      <div className="dm-missing-backup-notice__actions">
        {onImport ? (
          <button type="button" onClick={onImport}>
            バックアップをインポート
          </button>
        ) : null}
        {onPublishNew ? (
          <button type="button" onClick={onPublishNew}>
            DM 鍵を発行
          </button>
        ) : null}
      </div>
    </section>
  );
}
