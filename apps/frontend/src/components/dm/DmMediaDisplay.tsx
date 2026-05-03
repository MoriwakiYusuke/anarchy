'use client';

/**
 * DmMediaDisplay — DM 受信側で添付メディアを表示するコンポーネント。
 *
 * fragment 取得 + AES-GCM 復号した結果は `lib/dm/media.ts` のグローバルキャッシュ
 * (`getCachedDmMediaUrl`) で共有する。これにより:
 *   - re-render での useEffect 再起動による無駄な再 fetch / URL revoke を回避
 *     (revoke すると `<img src=blob:>` も `<a href=blob: download>` も壊れる)
 *   - Lightbox 開閉や conversation スクロールでも blob URL が安定
 *   - 同一 root+key を複数 bubble が参照しても 1 回しか復号しない
 *
 * 画像クリックは in-modal Lightbox。動画は inline `<video controls>`。
 * その他のファイルは `<a download>` リンク。
 */

import { useCallback, useEffect, useState } from 'react';
import Lightbox from '@/components/Lightbox';
import type { DmMediaRef } from '@/lib/dm/types';
import { getCachedDmMediaUrl } from '@/lib/dm/media';
import { useLocale } from '@/i18n';
import styles from './DmMediaDisplay.module.css';

export interface DmMediaDisplayProps {
  media: DmMediaRef[];
  /** Chain-node JSON-RPC HTTP endpoint。省略時は env / dev fallback。 */
  chainRpcEndpoint?: string;
}

interface ItemState {
  status: 'loading' | 'ready' | 'error';
  url?: string;
  error?: string;
}

const LOADING_STATE: ItemState = { status: 'loading' };

export function DmMediaDisplay({
  media,
  chainRpcEndpoint,
}: DmMediaDisplayProps): JSX.Element | null {
  const [lightboxIndex, setLightboxIndex] = useState<number | null>(null);

  const imageRefs = media.filter((ref) => ref.mime.startsWith('image/'));

  const handleImageClick = useCallback(
    (mediaIdx: number) => {
      const ref = media[mediaIdx];
      if (!ref) return;
      const li = imageRefs.findIndex((r) => r === ref);
      if (li >= 0) setLightboxIndex(li);
    },
    [media, imageRefs],
  );

  const handleLightboxClose = useCallback(() => setLightboxIndex(null), []);
  const handleLightboxPrev = useCallback(
    () =>
      setLightboxIndex((prev) =>
        prev === null
          ? null
          : prev > 0
            ? prev - 1
            : imageRefs.length - 1,
      ),
    [imageRefs.length],
  );
  const handleLightboxNext = useCallback(
    () =>
      setLightboxIndex((prev) =>
        prev === null
          ? null
          : prev < imageRefs.length - 1
            ? prev + 1
            : 0,
      ),
    [imageRefs.length],
  );

  if (media.length === 0) return null;

  return (
    <>
      <ul className={styles.grid}>
        {media.map((ref, idx) => (
          <DmMediaItem
            key={`${ref.root}-${idx}`}
            mediaRef={ref}
            chainRpcEndpoint={chainRpcEndpoint}
            onImageClick={() => handleImageClick(idx)}
          />
        ))}
      </ul>
      {lightboxIndex !== null && imageRefs.length > 0 && (
        <LightboxBridge
          imageRefs={imageRefs}
          chainRpcEndpoint={chainRpcEndpoint}
          currentIndex={lightboxIndex}
          onClose={handleLightboxClose}
          onPrev={handleLightboxPrev}
          onNext={handleLightboxNext}
        />
      )}
    </>
  );
}

/**
 * Lightbox に渡す URL リストはキャッシュから引く。currentIndex の画像が未だ
 * fetch 中の場合 (キャッシュ未登録) は loading 文言を出す。
 */
function LightboxBridge({
  imageRefs,
  chainRpcEndpoint,
  currentIndex,
  onClose,
  onPrev,
  onNext,
}: {
  imageRefs: DmMediaRef[];
  chainRpcEndpoint?: string;
  currentIndex: number;
  onClose: () => void;
  onPrev: () => void;
  onNext: () => void;
}): JSX.Element {
  const [urls, setUrls] = useState<Array<string | null>>(() =>
    imageRefs.map(() => null),
  );

  useEffect(() => {
    let cancelled = false;
    Promise.all(
      imageRefs.map((ref) =>
        getCachedDmMediaUrl(ref, { chainRpcEndpoint }).catch(() => null),
      ),
    ).then((resolved) => {
      if (!cancelled) setUrls(resolved);
    });
    return () => {
      cancelled = true;
    };
  }, [imageRefs, chainRpcEndpoint]);

  // **少なくとも currentIndex の画像が ready なら開く**。すべてが ready に
  // なるのを待つと、1 枚でも fetch 失敗 (404 / GC) があったとき lightbox が
  // 永久に開かない問題が起きる。失敗した枚は alt 表示で代替する。
  const currentReady = urls[currentIndex] !== null;
  if (!currentReady) return <></>;

  return (
    <Lightbox
      images={imageRefs.map((ref, i) => ({
        // 失敗 (null) は空文字列にフォールバック → Lightbox 側で alt が出る。
        src: urls[i] ?? '',
        width: ref.width,
        height: ref.height,
        alt: ref.mime,
      }))}
      currentIndex={currentIndex}
      onClose={onClose}
      onPrev={onPrev}
      onNext={onNext}
    />
  );
}

interface DmMediaItemProps {
  mediaRef: DmMediaRef;
  chainRpcEndpoint?: string;
  onImageClick: () => void;
}

function DmMediaItem({
  mediaRef,
  chainRpcEndpoint,
  onImageClick,
}: DmMediaItemProps): JSX.Element {
  const { t } = useLocale();
  const [item, setItem] = useState<ItemState>(LOADING_STATE);

  // 依存は ref の identity ではなく `root + key` 文字列でキー化する。
  // ConversationView 側で再 render が起きても同一 key なら effect は再起動しない。
  const refKey = `${mediaRef.root}|${mediaRef.key}`;

  useEffect(() => {
    let cancelled = false;
    setItem(LOADING_STATE);
    getCachedDmMediaUrl(mediaRef, { chainRpcEndpoint })
      .then((url) => {
        if (!cancelled) setItem({ status: 'ready', url });
      })
      .catch((err) => {
        if (!cancelled) {
          setItem({
            status: 'error',
            error: err instanceof Error ? err.message : String(err),
          });
        }
      });
    return () => {
      cancelled = true;
    };
    // mediaRef は object identity が変わってもキャッシュキーが同じなら同じ URL
    // を返すので問題ないが、念のため key で deps を絞る。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [refKey, chainRpcEndpoint]);

  if (item.status === 'loading') {
    return (
      <li className={`${styles.item} ${styles.itemLoading}`}>
        <span className={styles.placeholder}>{t('dm.media.loading')}</span>
      </li>
    );
  }
  if (item.status === 'error' || !item.url) {
    return (
      <li className={`${styles.item} ${styles.itemError}`}>
        <span className={styles.placeholder}>{t('dm.media.fetchFailed')}</span>
      </li>
    );
  }

  if (mediaRef.mime.startsWith('image/')) {
    return (
      <li className={styles.item}>
        <button
          type="button"
          onClick={onImageClick}
          className={styles.imageButton}
          aria-label={t('dm.media.openImage')}
        >
          <img src={item.url} alt={mediaRef.mime} className={styles.image} />
        </button>
      </li>
    );
  }
  if (mediaRef.mime.startsWith('video/')) {
    return (
      <li className={styles.item}>
        <video
          src={item.url}
          controls
          preload="metadata"
          className={styles.video}
        />
      </li>
    );
  }
  // それ以外は download リンク。`download` 属性で blob を消費するので revoke 前に
  // ブラウザがストリームを保持する。
  return (
    <li className={`${styles.item} ${styles.itemFile}`}>
      <a href={item.url} download className={styles.fileLink}>
        {t('dm.media.downloadFile', { mime: mediaRef.mime, size: mediaRef.size })}
      </a>
    </li>
  );
}

export default DmMediaDisplay;
