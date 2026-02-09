'use client'

import { useState, useEffect } from 'react'
import { PolkadotClient } from 'polkadot-api'
import { useLocale } from '@/i18n'
import { PostItem } from './PostItem'
import styles from './Timeline.module.css'

interface ContentRef {
  root: number[]
  k: number
  n: number
  total_size: number
}

interface Post {
  id: number
  author: string
  content: string
  contentHash: string
  createdAt: number
  parentId: number | null
  contentRef?: ContentRef
}

interface Props {
  client: PolkadotClient | null
  unsafeApi: any
  refreshTrigger?: number
}

export function Timeline({ client, unsafeApi, refreshTrigger }: Props) {
  const { t } = useLocale()
  const [posts, setPosts] = useState<Post[]>([])
  const [isLoading, setIsLoading] = useState(true)

  useEffect(() => {
    if (!unsafeApi) return

    const fetchPosts = async () => {
      try {
        // Check if Post pallet exists
        if (!unsafeApi.query.Post) {
          console.log('Post pallet not found')
          setIsLoading(false)
          return
        }

        // getEntriesを使用して全投稿メタデータを取得
        const postEntries = await unsafeApi.query.Post.Posts.getEntries()
        
        // V1: コンテンツ本文を取得（旧形式）
        const contentMap = new Map<number, string>()
        try {
          if (unsafeApi.query.Post.Contents) {
            const contentEntries = await unsafeApi.query.Post.Contents.getEntries()
            for (const entry of contentEntries) {
              const postId = Number(entry.keyArgs[0])
              // PAPI v1.x: valueは直接Uint8ArrayまたはBoundedVec
              // asBytes()メソッドがある場合はそれを使い、なければ直接使用
              let bytes: Uint8Array | number[] | undefined
              const value = entry.value
              if (typeof value?.asBytes === 'function') {
                bytes = value.asBytes()
              } else if (value instanceof Uint8Array) {
                bytes = value
              } else if (Array.isArray(value)) {
                bytes = value
              } else if (value && typeof value === 'object') {
                // BoundedVecの場合、内部配列を取得
                bytes = value.value || value
              }
              if (bytes) {
                try {
                  const text = new TextDecoder().decode(new Uint8Array(bytes))
                  contentMap.set(postId, text)
                } catch {
                  contentMap.set(postId, '(デコードエラー)')
                }
              }
            }
          }
        } catch (err) {
          console.log('Contents storage not available (V1):', err)
        }

        // V2: ContentRefs を取得（分散ストレージ参照）
        const contentRefMap = new Map<number, ContentRef>()
        try {
          if (unsafeApi.query.Post.ContentRefs) {
            const refEntries = await unsafeApi.query.Post.ContentRefs.getEntries()
            for (const entry of refEntries) {
              const postId = Number(entry.keyArgs[0])
              const ref = entry.value
              if (ref) {
                contentRefMap.set(postId, {
                  root: Array.from(ref.root || []),
                  k: Number(ref.k || 3),
                  n: Number(ref.n || 5),
                  total_size: Number(ref.total_size || 0),
                })
              }
            }
          }
        } catch (err) {
          console.log('ContentRefs storage not available (V2):', err)
        }
        
        const fetchedPosts: Post[] = postEntries.map((entry: any) => {
          const postId = Number(entry.keyArgs[0])
          const post = entry.value
          const contentRef = contentRefMap.get(postId)
          return {
            id: postId,
            author: post.author || 'unknown',
            // V2投稿はcontentRefがあるのでcontentは空でOK
            content: contentMap.get(postId) || '',
            contentHash: post.content_hash?.asHex?.() || '',
            createdAt: Number(post.created_at || 0),
            parentId: post.parent_id !== undefined ? Number(post.parent_id) : null,
            contentRef,
          }
        })

        // 最新順（作成ブロック番号の降順）でソート
        fetchedPosts.sort((a, b) => b.createdAt - a.createdAt)

        setPosts(fetchedPosts.slice(0, 50))
      } catch (err) {
        console.error('Failed to fetch posts:', err)
      } finally {
        setIsLoading(false)
      }
    }

    fetchPosts()

    // Note: PAPI event subscription is different, skipping for now
    // TODO: Add event subscription for new posts
  }, [unsafeApi, refreshTrigger])

  const shortenAddress = (addr: string) => {
    if (addr.startsWith('0x')) addr = addr.slice(2)
    return `${addr.slice(0, 6)}...${addr.slice(-4)}`
  }

  if (isLoading) {
    return (
      <div className={styles.loading}>
        {t('timeline.loading')}
      </div>
    )
  }

  if (posts.length === 0) {
    return (
      <div className={styles.empty}>
        <p>{t('timeline.empty')}</p>
      </div>
    )
  }

  return (
    <div className={styles.timeline}>
      {posts.map((post) => (
        <PostItem
          key={post.id}
          postId={post.id}
          author={post.author}
          contentHash={post.contentHash}
          createdAt={post.createdAt}
          parentId={post.parentId}
          inlineContent={post.content || undefined}
          contentRef={post.contentRef}
          shortenAddress={shortenAddress}
        />
      ))}
    </div>
  )
}

