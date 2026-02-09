'use client'

import { useState, useEffect } from 'react'
import { useStorage } from '@/hooks/useStorage'
import styles from './Timeline.module.css'

interface ContentRef {
  root: number[]       // [u8; 32]
  k: number
  n: number
  total_size: number
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
          const merkleRoot = new Uint8Array(contentRef.root)
          const result = await recoverContent(merkleRoot, contentRef.k, contentRef.n)
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
    displayContent = <span className={styles.loading}>コンテンツを復元中...</span>
  } else if (error) {
    displayContent = <span className={styles.error}>復元エラー: {error}</span>
  } else if (content) {
    displayContent = content
  } else if (!inlineContent && !contentRef) {
    displayContent = '(コンテンツなし)'
  } else {
    displayContent = <span className={styles.loading}>読み込み中...</span>
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
        {contentRef && (
          <span className={styles.v2Badge} title={`k=${contentRef.k}, n=${contentRef.n}`}>
            V2
          </span>
        )}
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
