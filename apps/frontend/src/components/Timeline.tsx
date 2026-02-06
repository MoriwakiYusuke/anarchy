'use client'

import { useState, useEffect } from 'react'
import { ApiPromise } from '@polkadot/api'
import styles from './Timeline.module.css'

interface Post {
  id: number
  author: string
  contentHash: string
  createdAt: number
  parentId: number | null
}

interface Props {
  api: ApiPromise | null
}

export function Timeline({ api }: Props) {
  const [posts, setPosts] = useState<Post[]>([])
  const [isLoading, setIsLoading] = useState(true)

  useEffect(() => {
    if (!api) return

    const fetchPosts = async () => {
      try {
        // 投稿数を取得
        const nextId = await api.query.post.nextPostId()
        const totalPosts = (nextId as any).toNumber()

        // 全投稿を取得（最新順）
        const fetchedPosts: Post[] = []
        for (let i = totalPosts - 1; i >= 0 && fetchedPosts.length < 50; i--) {
          const post = await api.query.post.posts(i)
          if (post.isSome) {
            const p = post.unwrap()
            fetchedPosts.push({
              id: i,
              author: (p as any).author.toString(),
              contentHash: (p as any).contentHash.toHex(),
              createdAt: (p as any).createdAt.toNumber(),
              parentId: (p as any).parentId.isSome 
                ? (p as any).parentId.unwrap().toNumber() 
                : null,
            })
          }
        }

        setPosts(fetchedPosts)
      } catch (err) {
        console.error('投稿の取得に失敗:', err)
      } finally {
        setIsLoading(false)
      }
    }

    fetchPosts()

    // イベントをサブスクライブして新規投稿を検知
    let unsubscribe: () => void

    const subscribe = async () => {
      unsubscribe = await api.query.system.events((events: any) => {
        events.forEach((record: any) => {
          const { event } = record
          if (event.section === 'post' && event.method === 'PostCreated') {
            // 新規投稿があったら再取得
            fetchPosts()
          }
        })
      })
    }

    subscribe()

    return () => {
      if (unsubscribe) unsubscribe()
    }
  }, [api])

  const shortenAddress = (addr: string) => {
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
            <code className={styles.hash}>
              Content Hash: {post.contentHash.slice(0, 18)}...
            </code>
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
