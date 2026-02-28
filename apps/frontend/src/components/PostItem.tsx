'use client'

import { useState, useEffect, useCallback } from 'react'
import { useStorage, type HybridMetadata } from '@/hooks/useStorage'
import { useLocale } from '@/i18n/context'
import { decodePostContent, mediaToDataUrl } from '@/lib/postCodec'
import type { MediaItem as PostMediaItem } from '@/lib/postCodec'
import { CopyIcon, CheckIcon, ReplyIcon } from '@/components/Icons'
import { ImageModal } from '@/components/ImageModal'
import { ReactionButton } from '@/components/ReactionButton'
import type { PolkadotSigner } from 'polkadot-api/signer'
import styles from './Timeline.module.css'

/**
 * アドレスを短縮表示する
 */
function shortenAddress(addr: string): string {
  if (addr.length <= 16) return addr
  return `${addr.slice(0, 8)}...${addr.slice(-6)}`
}

interface ContentRef {
  root: number[]       // [u8; 32]
  k: number
  n: number
  total_size: number
  // Hybrid metadata fields (now stored on-chain)
  ciphertext_len: number
  shard_size: number
  compressed: boolean
}

interface Props {
  postId: number
  author: string
  contentHash: string  // hex string
  createdAt: number
  parentId: number | null
  /** V1: inline content from Contents storage */
  inlineContent?: string
  /** V2: content reference from ContentRefs storage */  
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
  /** Reaction counts */
  likes?: number
  boosts?: number
  bads?: number
}

/**
 * 投稿アイテム - V1 (inline) と V2 (distributed storage) 両方に対応
 */
export function PostItem({
  postId,
  author,
  createdAt,
  parentId,
  inlineContent,
  contentRef,
  nickname,
  client,
  unsafeApi,
  account,
  signer,
  likes,
  boosts,
  bads,
}: Props) {
  const { t } = useLocale()
  const { recoverContent, isReady } = useStorage()
  const [content, setContent] = useState<string | null>(null)
  const [decodedMedia, setDecodedMedia] = useState<PostMediaItem[]>([])
  const [isLoading, setIsLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    // V1: inline content available
    if (inlineContent) {
      setContent(inlineContent)
      return
    }

    // V2: need to recover from distributed storage
    if (contentRef && isReady) {
      const fetchV2Content = async () => {
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
          
          // Construct HybridMetadata from ContentRef
          // All metadata fields are now stored on-chain
          const metadata: HybridMetadata = {
            originalLen: contentRef.total_size,
            ciphertextLen: contentRef.ciphertext_len,
            shardSize: contentRef.shard_size,
            compressed: contentRef.compressed,
            threshold: contentRef.k,
            totalShards: contentRef.n,
          }
          
          const result = await recoverContent(merkleRoot, metadata)
          
          // Decode binary content (text + media)
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
      fetchV2Content()
    }
  }, [inlineContent, contentRef, isReady, recoverContent, postId])

  // Determine what to display
  let displayContent: React.ReactNode
  if (isLoading) {
    displayContent = <span className={styles.contentLoading}>{t('content.loading')}</span>
  } else if (error) {
    displayContent = <span className={styles.error}>{t('content.error', { error })}</span>
  } else if (content) {
    displayContent = content
  } else if (!inlineContent && !contentRef) {
    displayContent = '(コンテンツなし)'
  } else {
    displayContent = <span className={styles.contentLoading}>読み込み中...</span>
  }

  const [copied, setCopied] = useState(false)
  const [modalImage, setModalImage] = useState<{ src: string; filename: string } | null>(null)
  const [expanded, setExpanded] = useState(false)

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

  return (
    <article className={styles.post}>
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
        </div>
        <ReactionButton
          postId={postId}
          client={client}
          unsafeApi={unsafeApi}
          account={account}
          signer={signer}
          likes={likes}
          boosts={boosts}
          bads={bads}
        />
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
  )
}
