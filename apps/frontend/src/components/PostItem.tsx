'use client'

import { useState, useEffect, useCallback } from 'react'
import { useStorage, type HybridMetadata } from '@/hooks/useStorage'
import { useLocale } from '@/i18n/context'
import MediaDisplay, { type MediaItem } from '@/components/MediaDisplay'
import { CopyIcon, CheckIcon, ReplyIcon } from '@/components/Icons'
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
  // Optional hybrid metadata fields (may not be present in older posts)
  ciphertext_len?: number
  shard_size?: number
  compressed?: boolean
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
  /** Optional media attachments */
  media?: MediaItem[]
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
  media = [],
}: Props) {
  const { t } = useLocale()
  const { recoverContent, isReady } = useStorage()
  const [content, setContent] = useState<string | null>(null)
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
          // Note: Posts created before hybrid migration cannot be recovered with this code path
          // Reed-Solomon shard_size = ceil(ciphertext_len / k)
          // AES-GCM overhead: 12 bytes nonce + 16 bytes auth tag
          // TODO: Export AES_GCM_OVERHEAD from wasm-engine and import here to ensure consistency
          // with backend encryption parameters. If encryption params change, this estimation
          // could become incorrect.
          const AES_GCM_OVERHEAD = 12 + 16 // nonce (12) + auth tag (16) = 28 bytes
          const estimatedCiphertextLen = contentRef.total_size + AES_GCM_OVERHEAD
          const metadata: HybridMetadata = {
            originalLen: contentRef.total_size,
            ciphertextLen: contentRef.ciphertext_len ?? estimatedCiphertextLen,
            // shard_size must use k (threshold), not n (total shards)
            shardSize: contentRef.shard_size ?? Math.ceil((contentRef.ciphertext_len ?? estimatedCiphertextLen) / contentRef.k),
            // compressed flag from ContentRef, default false for older posts (may fail if mismatched)
            compressed: contentRef.compressed ?? false,
            threshold: contentRef.k,
            totalShards: contentRef.n,
          }
          
          const result = await recoverContent(merkleRoot, metadata)
          const text = new TextDecoder().decode(result.data)
          setContent(text)
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
        <p className={styles.text}>{displayContent}</p>
        {media.length > 0 && (
          <MediaDisplay media={media} />
        )}
      </div>
      <footer className={styles.postFooter}>
        <span className={styles.postId}>
          Post #{postId}
        </span>
        {parentId !== null && (
          <span className={styles.reply}>
            <ReplyIcon size={12} /> Reply to #{parentId}
          </span>
        )}
      </footer>
    </article>
  )
}
