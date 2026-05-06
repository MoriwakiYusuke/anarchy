'use client'

import { useState, useEffect, useCallback } from 'react'
import { useStorage, type HybridMetadata, type StorageSigner } from '@/hooks/useStorage'
import { useLocale } from '@/i18n/context'
import { decodePostContent, mediaToDataUrl } from '@/lib/postCodec'
import type { MediaItem as PostMediaItem } from '@/lib/postCodec'
import { CopyIcon, CheckIcon, ReplyIcon } from '@/components/Icons'
import { ImageModal } from '@/components/ImageModal'
import { ReactionButton } from '@/components/ReactionButton'
import { PostForm } from '@/components/PostForm'
import type { PolkadotSigner } from 'polkadot-api/signer'
import styles from './Timeline.module.css'

/**
 * アドレスを短縮表示する
 */
function shortenAddress(addr: string): string {
  if (addr.length <= 16) return addr
  return `${addr.slice(0, 8)}...${addr.slice(-6)}`
}

export interface ContentRef {
  root: number[]       // [u8; 32]
  k: number
  n: number
  total_size: number
  // Hybrid metadata fields (now stored on-chain)
  ciphertext_len: number
  shard_size: number
  compressed: boolean
}

/** ネスト返信 1 件分のデータ。Timeline が root ごとに集めて渡す */
export interface NestedReplyData {
  id: number
  author: string
  contentHash: string
  createdAt: number
  parentId: number | null
  contentRef?: ContentRef
  nickname?: string
  likes?: number
  bads?: number
}

interface Props {
  postId: number
  author: string
  contentHash: string  // hex string
  createdAt: number
  parentId: number | null
  /** Distributed-storage reference for the post body (ContentRefs storage). */
  contentRef?: ContentRef
  /** Optional nickname for the author */
  nickname?: string
  /** PAPI client */
  client?: unknown
  /** PAPI unsafe API */
  unsafeApi?: unknown
  /** User's account address */
  account?: string | null
  /** Polkadot signer */
  signer?: PolkadotSigner | null
  /** Storage signer for inline reply form upload */
  storageSigner?: StorageSigner | null
  /** Reaction counts */
  likes?: number
  bads?: number
  /** 同スレッドの返信一覧 (root ポスト時のみ指定)。block 昇順でソート済みを想定 */
  replies?: NestedReplyData[]
  /** ネスト表示モード — Reply ボタンと返信展開トグルを抑制し、インデント付きで描画する */
  isNested?: boolean
  /** 返信が投稿されたときに親 (Timeline) へ refresh を要求するコールバック */
  onReplyPosted?: () => void
}

export function PostItem({
  postId,
  author,
  createdAt,
  parentId,
  contentRef,
  nickname,
  client,
  unsafeApi,
  account,
  signer,
  storageSigner,
  likes,
  bads,
  replies,
  isNested = false,
  onReplyPosted,
}: Props) {
  const { t } = useLocale()
  const { recoverContent, isReady } = useStorage()
  const [content, setContent] = useState<string | null>(null)
  const [decodedMedia, setDecodedMedia] = useState<PostMediaItem[]>([])
  const [isLoading, setIsLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (!contentRef || !isReady) return
    const fetchContent = async () => {
      setIsLoading(true)
      setError(null)
      try {
        // Handle PAPI Binary type - may have asBytes() method
        const rootData = contentRef.root as unknown
        let merkleRoot: Uint8Array
        if (rootData && typeof rootData === 'object' && 'asBytes' in rootData && typeof (rootData as { asBytes: () => Uint8Array }).asBytes === 'function') {
          merkleRoot = (rootData as { asBytes: () => Uint8Array }).asBytes()
        } else if (Array.isArray(rootData)) {
          merkleRoot = new Uint8Array(rootData)
        } else {
          merkleRoot = new Uint8Array(contentRef.root)
        }

        if (merkleRoot.length !== 32) {
          throw new Error(`Invalid merkle root length: ${merkleRoot.length}`)
        }

        const metadata: HybridMetadata = {
          originalLen: contentRef.total_size,
          ciphertextLen: contentRef.ciphertext_len,
          shardSize: contentRef.shard_size,
          compressed: contentRef.compressed,
          threshold: contentRef.k,
          totalShards: contentRef.n,
        }

        const result = await recoverContent(merkleRoot, metadata)
        const decoded = decodePostContent(result.data)
        setContent(decoded.text)
        setDecodedMedia(decoded.media)
      } catch (err) {
        console.error(`[PostItem] Failed to recover content for post ${postId}:`, err)
        setError(err instanceof Error ? err.message : String(err))
      } finally {
        setIsLoading(false)
      }
    }
    void fetchContent()
  }, [contentRef, isReady, recoverContent, postId])

  // Determine what to display
  let displayContent: React.ReactNode
  if (isLoading) {
    displayContent = <span className={styles.contentLoading}>{t('content.loading')}</span>
  } else if (error) {
    displayContent = <span className={styles.error}>{t('content.error', { error })}</span>
  } else if (content) {
    displayContent = content
  } else if (!contentRef) {
    displayContent = '(コンテンツなし)'
  } else {
    displayContent = <span className={styles.contentLoading}>読み込み中...</span>
  }

  const [copied, setCopied] = useState(false)
  const [modalImage, setModalImage] = useState<{ src: string; filename: string } | null>(null)
  const [expanded, setExpanded] = useState(false)
  const [replyFormOpen, setReplyFormOpen] = useState(false)
  const [repliesExpanded, setRepliesExpanded] = useState(false)

  const replyCount = replies?.length ?? 0
  const canReply = !isNested && Boolean(unsafeApi && signer && account)

  // テキストが長いかどうかを判定（200文字 or 5行以上）
  const isLongContent = typeof content === 'string' && (content.length > 200 || content.split('\n').length > 5)

  const handleCopyAddress = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(author)
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    } catch (err) {
      console.error('Failed to copy address:', err)
    }
  }, [author])

  const articleClassName = isNested ? `${styles.post} ${styles.nestedPost}` : styles.post

  return (
    <div className={isNested ? styles.threadItem : styles.threadRoot}>
    <article className={articleClassName}>
      <header className={styles.postHeader}>
        <span className={styles.author}>
          {nickname && <span className={styles.nickname}>{nickname}</span>}
          <span className={styles.addressRow}>
            <span className={styles.address}>{shortenAddress(author)}</span>
            <button
              className={styles.copyButton}
              onClick={handleCopyAddress}
              title={t('address.clickToCopy')}
              type="button"
            >
              {copied ? <CheckIcon size={12} /> : <CopyIcon size={12} />}
            </button>
          </span>
        </span>
        <span className={styles.block}>
          Block #{createdAt}
        </span>
      </header>
      <div className={styles.content}>
        <p className={`${styles.text} ${isLongContent && !expanded ? styles.textCollapsed : ''}`}>
          {displayContent}
        </p>
        {isLongContent && (
          <button
            type="button"
            className={styles.expandButton}
            onClick={() => setExpanded(!expanded)}
          >
            {expanded ? t('post.collapse') : t('post.expand')}
          </button>
        )}
        {decodedMedia.length > 0 && (
          <div className={styles.mediaGrid}>
            {decodedMedia.map((item, idx) => {
              const dataUrl = mediaToDataUrl(item)
              // Use filename from codec if available, otherwise generate one
              const filename = item.filename || `media-${postId}-${idx + 1}.${item.type.split('/')[1] || 'bin'}`
              const isVideo = item.type.startsWith('video/')
              const isImage = item.type.startsWith('image/')
              
              // General file (not image or video)
              if (!isVideo && !isImage) {
                return (
                  <div key={idx} className={styles.fileItem}>
                    <span className={styles.fileIcon}>📄</span>
                    <span className={styles.fileName}>{filename}</span>
                    <a
                      href={dataUrl}
                      download={filename}
                      className={styles.fileDownloadBtn}
                      title="ダウンロード"
                    >
                      ↓
                    </a>
                  </div>
                )
              }
              
              return (
                <div key={idx} className={styles.mediaWrapper}>
                  {isVideo ? (
                    <video
                      src={dataUrl}
                      className={styles.mediaImage}
                      controls
                      preload="metadata"
                    />
                  ) : (
                    <img
                      src={dataUrl}
                      alt={filename}
                      className={styles.mediaImage}
                      onClick={() => setModalImage({ src: dataUrl, filename })}
                    />
                  )}
                  <div className={styles.mediaActions}>
                    <a
                      href={dataUrl}
                      download={filename}
                      className={styles.mediaActionBtn}
                      title="ダウンロード"
                    >
                      ↓
                    </a>
                  </div>
                </div>
              )
            })}
          </div>
        )}
      </div>
      <footer className={styles.postFooter}>
        <div className={styles.postMeta}>
          <span className={styles.postId}>
            Post #{postId}
          </span>
          {parentId !== null && (
            <span className={styles.reply}>
              <ReplyIcon size={12} /> Reply to #{parentId}
            </span>
          )}
          {!isNested && replyCount > 0 && (
            <button
              type="button"
              className={styles.replyCountBtn}
              onClick={() => setRepliesExpanded((v) => !v)}
              aria-expanded={repliesExpanded}
            >
              <ReplyIcon size={12} />{' '}
              {repliesExpanded
                ? t('post.hideReplies')
                : replyCount === 1
                  ? t('post.viewReply')
                  : t('post.viewReplies', { count: replyCount })}
            </button>
          )}
        </div>
        <div className={styles.postActions}>
          {canReply && (
            <button
              type="button"
              className={styles.replyBtn}
              onClick={() => setReplyFormOpen((v) => !v)}
              aria-expanded={replyFormOpen}
            >
              <ReplyIcon size={12} /> {t('post.reply')}
            </button>
          )}
          <ReactionButton
            postId={postId}
            client={client}
            unsafeApi={unsafeApi}
            account={account}
            signer={signer}
            likes={likes}
            bads={bads}
          />
        </div>
      </footer>

      {/* Image modal */}
      {modalImage && (
        <ImageModal
          src={modalImage.src}
          downloadFilename={modalImage.filename}
          onClose={() => setModalImage(null)}
        />
      )}
    </article>

    {!isNested && replyFormOpen && unsafeApi != null && signer != null && (
      <div className={styles.replyFormWrapper}>
        <PostForm
          unsafeApi={unsafeApi}
          signer={signer}
          storageSigner={storageSigner ?? null}
          parentId={postId}
          onCancel={() => setReplyFormOpen(false)}
          onPostSuccess={() => {
            setReplyFormOpen(false)
            setRepliesExpanded(true)
            onReplyPosted?.()
          }}
        />
      </div>
    )}

    {!isNested && repliesExpanded && replies && replies.length > 0 && (
      <div className={styles.replyThread}>
        {replies.map((r) => (
          <PostItem
            key={r.id}
            postId={r.id}
            author={r.author}
            contentHash={r.contentHash}
            createdAt={r.createdAt}
            parentId={r.parentId}
            contentRef={r.contentRef}
            nickname={r.nickname}
            client={client}
            unsafeApi={unsafeApi}
            account={account}
            signer={signer}
            storageSigner={storageSigner}
            likes={r.likes}
            bads={r.bads}
            isNested
          />
        ))}
      </div>
    )}
    </div>
  )
}
