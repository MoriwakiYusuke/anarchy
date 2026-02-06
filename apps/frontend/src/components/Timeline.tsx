'use client'

import { useState, useEffect } from 'react'
import { PolkadotClient } from 'polkadot-api'
import styles from './Timeline.module.css'

interface Post {
  id: number
  author: string
  content: string
  contentHash: string
  createdAt: number
  parentId: number | null
}

interface Props {
  client: PolkadotClient | null
  unsafeApi: any
}

export function Timeline({ client, unsafeApi }: Props) {
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
        
        // コンテンツ本文も取得
        const contentEntries = await unsafeApi.query.Post.Contents.getEntries()
        const contentMap = new Map<number, string>()
        for (const entry of contentEntries) {
          const postId = Number(entry.keyArgs[0])
          // BoundedVec<u8, MaxContentLength> をテキストに変換
          const bytes = entry.value?.asBytes?.() || entry.value
          if (bytes) {
            try {
              const text = new TextDecoder().decode(new Uint8Array(bytes))
              contentMap.set(postId, text)
            } catch {
              contentMap.set(postId, '(デコードエラー)')
            }
          }
        }
        
        const fetchedPosts: Post[] = postEntries.map((entry: any) => {
          const postId = Number(entry.keyArgs[0])
          const post = entry.value
          return {
            id: postId,
            author: post.author || 'unknown',
            content: contentMap.get(postId) || '(コンテンツなし)',
            contentHash: post.content_hash?.asHex?.() || '',
            createdAt: Number(post.created_at || 0),
            parentId: post.parent_id !== undefined ? Number(post.parent_id) : null,
          }
        })

        // 最新順（作成ブロック番号の降順）でソート
        fetchedPosts.sort((a, b) => b.createdAt - a.createdAt)

        setPosts(fetchedPosts.slice(0, 50))
      } catch (err) {
        console.error('投稿の取得に失敗:', err)
      } finally {
        setIsLoading(false)
      }
    }

    fetchPosts()

    // Note: PAPI event subscription is different, skipping for now
    // TODO: Add event subscription for new posts
  }, [unsafeApi])

  const shortenAddress = (addr: string) => {
    if (addr.startsWith('0x')) addr = addr.slice(2)
    return `${addr.slice(0, 6)}...${addr.slice(-4)}`
  }

  if (isLoading) {
    return (
      <div className={styles.loading}>
        タイムラインを読み込み中...
      </div>
    )
  }

  if (posts.length === 0) {
    return (
      <div className={styles.empty}>
        <p>まだ投稿がありません</p>
        <p className={styles.hint}>最初の投稿者になりましょう</p>
      </div>
    )
  }

  return (
    <div className={styles.timeline}>
      {posts.map((post) => (
        <article key={post.id} className={styles.post}>
          <header className={styles.postHeader}>
            <span className={styles.author}>
              {shortenAddress(post.author)}
            </span>
            <span className={styles.block}>
              Block #{post.createdAt}
            </span>
          </header>
          <div className={styles.content}>
            <p className={styles.text}>{post.content}</p>
          </div>
          <footer className={styles.postFooter}>
            <span className={styles.postId}>
              Post #{post.id}
            </span>
            {post.parentId !== null && (
              <span className={styles.reply}>
                ↩ Reply to #{post.parentId}
              </span>
            )}
          </footer>
        </article>
      ))}
    </div>
  )
}
