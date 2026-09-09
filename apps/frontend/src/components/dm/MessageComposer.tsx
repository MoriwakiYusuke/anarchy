'use client';

/**
 * MessageComposer — DM 送信フォーム + 添付メディア。
 *
 * Flow:
 *   1. ユーザーがテキスト入力 + (任意で) ファイル添付。
 *   2. 送信時:
 *        a. 添付ファイルを `uploadDmMedia` で 1 件ずつ encrypt → fragment → upload
 *           (`storage_uploadFragment`, k-of-n)。失敗したら sendDm 自体を実行せず
 *           `MediaUploadFailed` で abort。
 *        b. `encodeDmContent({ text, media })` で envelope を組む。
 *        c. 既存 `sendDm(...)` に `body` として渡す。
 *
 * Progress:
 *   - 添付フェーズは `attaching:{i}/{total}:{ufrag}/{nfrag}` を表示。
 *   - DM 送信フェーズは sender.ts の 5 段 (`encrypting → uploading → prefunding →
 *     dispatching → done`) を踏襲。
 */

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  estimateDmCostFromInputs,
  formatMoral,
  sendDm,
  type SendDmContext,
  type SendDmProgress,
} from '@/lib/dm/sender';
import { uploadDmMedia } from '@/lib/dm/media';
import { encodeDmContent } from '@/lib/dm/contentCodec';
import { useDmStore } from '@/lib/dm/store';
import { useLocale } from '@/i18n';
import type { TranslationKey } from '@/i18n';
import {
  DmError,
  type AccountId,
  type DmMediaRef,
  type SendDmParams,
} from '@/lib/dm/types';
import styles from './MessageComposer.module.css';

export interface MessageComposerProps {
  counterparty: AccountId;
  context: SendDmContext;
  onSent?: () => void;
}

interface PendingFile {
  id: string;
  file: File;
  /** プレビュー用 object URL (image/video のみ)。それ以外は undefined。 */
  preview?: string;
  /** アップロード進捗 0–100。 */
  progress: number;
  /** kind 表示用。 */
  kind: 'image' | 'video' | 'file';
}

type ComposerState =
  | { kind: 'idle' }
  | { kind: 'attaching'; fileIndex: number; fileTotal: number; uploadedFragments: number; totalFragments: number }
  | { kind: 'sending'; progress: SendDmProgress }
  | { kind: 'error'; banner: ErrorBanner }
  | { kind: 'sent' };

const PROGRESS_KEY: Record<SendDmProgress['kind'], TranslationKey> = {
  encrypting: 'dm.progress.encrypting',
  uploading: 'dm.progress.uploading',
  prefunding: 'dm.progress.prefunding',
  dispatching: 'dm.progress.dispatching',
  done: 'dm.progress.done',
};

interface ErrorBanner {
  key: TranslationKey;
  detail?: string;
  canRetry: boolean;
}

function classifyError(err: unknown): ErrorBanner {
  const msg = err instanceof Error ? err.message : String(err);
  if (msg === DmError.RecipientKeyNotPublished) {
    return { key: 'dm.error.recipientKeyNotPublished', canRetry: false };
  }
  if (msg === DmError.MainAccountInsufficientBalance) {
    return { key: 'dm.error.insufficientBalance', canRetry: false };
  }
  if (msg === DmError.StorageInsufficient) {
    return { key: 'dm.error.storageInsufficient', canRetry: true };
  }
  if (msg.startsWith(DmError.TransactionDropped)) {
    return { key: 'dm.error.transactionDropped', detail: msg, canRetry: true };
  }
  if (msg === DmError.BodyTooLarge) {
    return { key: 'dm.error.bodyTooLarge', canRetry: false };
  }
  if (msg.startsWith(DmError.MediaUploadFailed)) {
    return { key: 'dm.error.mediaUploadFailed', detail: msg, canRetry: true };
  }
  return { key: 'dm.error.generic', detail: msg, canRetry: true };
}

function detectKind(file: File): PendingFile['kind'] {
  if (file.type.startsWith('image/')) return 'image';
  if (file.type.startsWith('video/')) return 'video';
  return 'file';
}

export function MessageComposer({
  counterparty,
  context,
  onSent,
}: MessageComposerProps): JSX.Element {
  const { t } = useLocale();
  const [body, setBody] = useState('');
  const [files, setFiles] = useState<PendingFile[]>([]);
  const [state, setState] = useState<ComposerState>({ kind: 'idle' });
  const addOutgoing = useDmStore(
    (s: { addOutgoing: (m: import('@/lib/dm/types').DmMessageRecord) => void }) =>
      s.addOutgoing,
  );

  // phase-1 で upload 済みのファイルの DmMediaRef キャッシュ (PendingFile.id → ref)。
  // sendDm (extrinsic) が失敗して retry した際に、成功済みの添付を再 fragment /
  // 再 upload (= 再課金) しないため。ファイルを削除したら invalidate し、
  // 送信成功時に全クリアする (id は添付ごとに一意なので追加分は自然に miss する)。
  const uploadedMediaRef = useRef<Map<string, DmMediaRef>>(new Map());

  // unmount 時の blob URL revoke。`useEffect([])` 直接 closure だと
  // 初期 files (空配列) を捕捉してしまい、後から追加した preview がリークする。
  // ref を経由して常に最新の files を見るようにする。
  const filesRef = useRef<PendingFile[]>([]);
  filesRef.current = files;
  useEffect(() => {
    return () => {
      for (const f of filesRef.current) {
        if (f.preview) URL.revokeObjectURL(f.preview);
      }
    };
  }, []);

  const addFiles = useCallback((picked: File[]) => {
    const next: PendingFile[] = picked.map((file) => {
      const kind = detectKind(file);
      const preview = kind === 'image' || kind === 'video'
        ? URL.createObjectURL(file)
        : undefined;
      return {
        id: typeof crypto !== 'undefined' && 'randomUUID' in crypto
          ? crypto.randomUUID()
          : `${Date.now()}-${Math.random()}`,
        file,
        preview,
        progress: 0,
        kind,
      };
    });
    setFiles((prev) => [...prev, ...next]);
  }, []);

  const removeFile = useCallback((id: string) => {
    // 添付リストが変わったらキャッシュも invalidate (再 retry 時に外したファイル
    // の upload 結果を envelope に混入させない)。
    uploadedMediaRef.current.delete(id);
    setFiles((prev) => {
      const target = prev.find((f) => f.id === id);
      if (target?.preview) URL.revokeObjectURL(target.preview);
      return prev.filter((f) => f.id !== id);
    });
  }, []);

  const submit = useCallback(
    async (paramsOverride?: SendDmParams) => {
      const text = body;
      if (!paramsOverride && !text.trim() && files.length === 0) return;

      // ---- Phase 1: upload attachments (新規送信時のみ) ----
      let media: DmMediaRef[] = [];
      if (!paramsOverride && files.length > 0) {
        try {
          for (let i = 0; i < files.length; i += 1) {
            const f = files[i];
            // 前回の試行 (sendDm 失敗 → retry) で upload 済みならスキップして
            // 再アップロード課金を避ける。
            const cached = uploadedMediaRef.current.get(f.id);
            if (cached) {
              media.push(cached);
              continue;
            }
            setState({
              kind: 'attaching',
              fileIndex: i + 1,
              fileTotal: files.length,
              uploadedFragments: 0,
              totalFragments: 0,
            });
            const ref = await uploadDmMedia(
              f.file,
              {
                chainRpcEndpoint: context.chainRpcEndpoint,
                signer: context.mainRawSigner,
                onProgress: (uploaded, total) => {
                  setState({
                    kind: 'attaching',
                    fileIndex: i + 1,
                    fileTotal: files.length,
                    uploadedFragments: uploaded,
                    totalFragments: total,
                  });
                  setFiles((prev) =>
                    prev.map((pf) =>
                      pf.id === f.id
                        ? {
                            ...pf,
                            progress:
                              total > 0 ? Math.round((uploaded / total) * 100) : 0,
                          }
                        : pf,
                    ),
                  );
                },
              },
            );
            uploadedMediaRef.current.set(f.id, ref);
            media.push(ref);
          }
        } catch (err) {
          setState({ kind: 'error', banner: classifyError(err) });
          return;
        }
      }

      // ---- Phase 2: build envelope + send DM ----
      const params: SendDmParams =
        paramsOverride ?? {
          recipientAccountId: counterparty,
          body: encodeDmContent({ text, media }),
        };

      setState({ kind: 'sending', progress: { kind: 'encrypting' } });

      try {
        const result = await sendDm(params, {
          ...context,
          onProgress: (progress) => setState({ kind: 'sending', progress }),
        });

        addOutgoing({
          messageId: result.messageId,
          blockNumber: result.blockNumber,
          direction: 'outgoing',
          counterparty,
          timestampMs: Date.now(),
          body: params.body,
          bodyState: 'plaintext',
          deliveryState: 'sent',
        });

        setState({ kind: 'sent' });
        setBody('');
        // 送信成功時のみ files を消す (失敗 retry のため preview は残す)。
        for (const f of files) {
          if (f.preview) URL.revokeObjectURL(f.preview);
        }
        setFiles([]);
        uploadedMediaRef.current.clear();
        onSent?.();
      } catch (err) {
        setState({ kind: 'error', banner: classifyError(err) });
      }
    },
    [addOutgoing, body, context, counterparty, files, onSent],
  );

  const retry = useCallback(() => {
    if (!body.trim() && files.length === 0) return;
    void submit();
  }, [body, files.length, submit]);

  const isBusy = state.kind === 'sending' || state.kind === 'attaching';
  const canSubmit = !isBusy && (body.trim().length > 0 || files.length > 0);

  // コスト見積 (実際の `encodeDmContent` 結果から body byte 数を求めて bucket
  // を選ぶので、添付の thumbnail data URL も含めて正確に出る)。
  const estimatedCost = useMemo(() => {
    if (!body.trim() && files.length === 0) return null;
    return estimateDmCostFromInputs(
      body,
      files.map((f) => ({
        mime: f.file.type,
        size: f.file.size,
      })),
    );
  }, [body, files]);

  return (
    <div role="region" aria-label="DM compose form" className={styles.region}>
      <textarea
        value={body}
        onChange={(e) => setBody(e.target.value)}
        placeholder={t('dm.compose.bodyPlaceholder')}
        rows={3}
        disabled={isBusy}
        aria-label="dm message body"
        className={styles.textarea}
      />

      {files.length > 0 && (
        <ul
          className={styles.attachments}
          aria-label={t('dm.compose.attachmentsLabel')}
        >
          {files.map((f) => (
            <li key={f.id} className={styles.thumb}>
              {f.kind === 'image' && f.preview ? (
                <img
                  src={f.preview}
                  alt={f.file.name}
                  className={styles.thumbImg}
                />
              ) : f.kind === 'video' && f.preview ? (
                <video src={f.preview} muted className={styles.thumbVideo} />
              ) : (
                <div className={styles.thumbFile} title={f.file.name}>
                  {f.file.name}
                </div>
              )}
              <button
                type="button"
                onClick={() => removeFile(f.id)}
                disabled={isBusy}
                className={styles.thumbRemove}
                aria-label={t('dm.compose.removeAttachment', {
                  name: f.file.name,
                })}
              >
                ×
              </button>
              {isBusy && f.progress > 0 && f.progress < 100 ? (
                <div className={styles.thumbProgress}>{f.progress}%</div>
              ) : null}
            </li>
          ))}
        </ul>
      )}

      <div className={styles.attachRow}>
        {/* ボタン経由の `fileInputRef.current.click()` だと React の onClick 経由で
         *  user-gesture chain が壊れたり、Strict Mode 二重マウント中に stale ref を
         *  指す等の不安定要因がある。`<label>` で input を囲むと、ブラウザが純粋に
         *  ネイティブの click → file picker を起動するためこれらを回避できる。 */}
        <label
          className={`${styles.attachBtn} ${isBusy ? styles.attachBtnDisabled : ''}`}
          aria-disabled={isBusy}
        >
          {t('dm.compose.attachButton')}
          <input
            type="file"
            multiple
            className={styles.hiddenInput}
            onChange={(e) => {
              const picked = Array.from(e.target.files ?? []);
              if (picked.length > 0) addFiles(picked);
              e.target.value = '';
            }}
            disabled={isBusy}
          />
        </label>
        <button
          type="button"
          onClick={() => void submit()}
          disabled={!canSubmit}
          className={styles.sendBtn}
        >
          {isBusy ? t('dm.compose.sending') : t('dm.compose.send')}
        </button>
      </div>

      {state.kind === 'idle' || state.kind === 'sent' || state.kind === 'error' ? (
        estimatedCost === null && (body.trim() || files.length > 0) ? (
          <p role="status" className={styles.costWarn}>
            {t('dm.compose.bodyTooLargeWarn')}
          </p>
        ) : estimatedCost !== null ? (
          <p role="status" aria-live="polite" className={styles.cost}>
            {t('dm.compose.estimatedCost', { amount: formatMoral(estimatedCost) })}
          </p>
        ) : null
      ) : null}

      {state.kind === 'attaching' && (
        <p role="status" aria-live="polite" className={styles.progress}>
          {t('dm.progress.attaching', {
            current: state.fileIndex,
            total: state.fileTotal,
            uploaded: state.uploadedFragments,
            fragments: state.totalFragments,
          })}
        </p>
      )}

      {state.kind === 'sending' && (
        <p role="status" aria-live="polite" className={styles.progress}>
          {state.progress.kind === 'uploading'
            ? t('dm.progress.uploadingProgress', {
                uploaded: state.progress.uploaded,
                total: state.progress.total,
              })
            : t(PROGRESS_KEY[state.progress.kind])}
        </p>
      )}

      {state.kind === 'sent' && (
        <p role="status" aria-live="polite" className={styles.sent}>
          {t('dm.compose.sent')}
        </p>
      )}

      {state.kind === 'error' && (
        <div role="alert" className={styles.error}>
          <p className={styles.errorText}>
            {state.banner.detail
              ? t(state.banner.key, { detail: state.banner.detail })
              : t(state.banner.key)}
          </p>
          {state.banner.canRetry && (
            <button type="button" onClick={retry} className={styles.retryBtn}>
              {t('dm.compose.retry')}
            </button>
          )}
        </div>
      )}
    </div>
  );
}

export default MessageComposer;
