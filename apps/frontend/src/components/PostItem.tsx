'use client'

import { useState, useEffect } from 'react'
import { useStorage, type HybridMetadata } from '@/hooks/useStorage'
import { useLocale } from '@/i18n/context'
import styles from './Timeline.module.css'

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
  shortenAddress: (addr: string) => string
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
  shortenAddress,
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
            console.warn(`[PostItem] Invalid merkle root length: ${merkleRoot.length} (expected 32)`)
            throw new Error(`Invalid merkle root length: ${merkleRoot.length}`)
          }
          
          console.log(`[PostItem] Recovering content for post ${postId}, merkle_root:`, Array.from(merkleRoot).map(b => b.toString(16).padStart(2, '0')).join(''))
          
          // Construct HybridMetadata from ContentRef
          // Note: Posts created before hybrid migration cannot be recovered with this code path
          // Reed-Solomon shard_size = ceil(ciphertext_len / k)
          // AES-GCM overhead: 12 bytes nonce + 16 bytes tag = 28 bytes
          const estimatedCiphertextLen = contentRef.total_size + 28
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

  return (
    <article className={styles.post}>
      <header className={styles.postHeader}>
        <span className={styles.author}>
          {shortenAddress(author)}
        </span>
        <span className={styles.block}>
          Block #{createdAt}
        </span>
      </header>
      <div className={styles.content}>
        <p className={styles.text}>{displayContent}</p>
      </div>
      <footer className={styles.postFooter}>
        <span className={styles.postId}>
          Post #{postId}
        </span>
        {parentId !== null && (
          <span className={styles.reply}>
            ↩ Reply to #{parentId}
          </span>
        )}
      </footer>
    </article>
  )
}
