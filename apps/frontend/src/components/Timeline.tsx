'use client'

import { useState, useEffect } from 'react'
import { PolkadotClient } from 'polkadot-api'
import type { PolkadotSigner } from 'polkadot-api/signer'
import { useLocale } from '@/i18n'
import { debugError } from '@/lib/debugLog'
import { PostItem } from './PostItem'
import styles from './Timeline.module.css'

interface ContentRef {
  root: number[]
  k: number
  n: number
  total_size: number
  ciphertext_len: number
  shard_size: number
  compressed: boolean
}

interface ReactionStats {
  likes: number
  boosts: number
  bads: number
}

interface Post {
  id: number
  author: string
  contentHash: string
  createdAt: number
  parentId: number | null
  contentRef?: ContentRef
  nickname?: string
  reactionStats?: ReactionStats
}

interface Props {
  client: PolkadotClient | null
  unsafeApi: any
  account: string | null
  signer: PolkadotSigner | null
  refreshTrigger?: number
}

export function Timeline({ client, unsafeApi, account, signer, refreshTrigger }: Props) {
  const { t } = useLocale()
  const [posts, setPosts] = useState<Post[]>([])
  const [isLoading, setIsLoading] = useState(true)

  useEffect(() => {
    if (!unsafeApi) return

    const fetchPosts = async () => {
      try {
        // Check if Post pallet exists
        if (!unsafeApi.query.Post) {
          setIsLoading(false)
          return
        }

        // getEntriesを使用して全投稿メタデータを取得
        const postEntries = await unsafeApi.query.Post.Posts.getEntries()

        // ContentRefs を取得（分散ストレージ参照）
        const contentRefMap = new Map<number, ContentRef>()
        try {
          if (unsafeApi.query.Post.ContentRefs) {
            const refEntries = await unsafeApi.query.Post.ContentRefs.getEntries()
            for (const entry of refEntries) {
              const postId = Number(entry.keyArgs[0])
              const ref = entry.value
              if (ref) {
                // Binary型からバイト配列を取得
                let rootBytes: number[] = []
                const root = ref.root
                if (typeof root?.asBytes === 'function') {
                  rootBytes = Array.from(root.asBytes())
                } else if (root instanceof Uint8Array) {
                  rootBytes = Array.from(root)
                } else if (Array.isArray(root)) {
                  rootBytes = root
                }
                
                // 32バイトの配列のみ有効
                if (rootBytes.length === 32) {
                  contentRefMap.set(postId, {
                    root: rootBytes,
                    k: Number(ref.k || 3),
                    n: Number(ref.n || 5),
                    // On-chain PostContent uses 'size' field, not 'total_size'
                    total_size: Number(ref.size || 0),
                    // Hybrid metadata fields from on-chain storage
                    ciphertext_len: Number(ref.ciphertext_len || 0),
                    shard_size: Number(ref.shard_size || 0),
                    compressed: Boolean(ref.compressed),
                  })
                }
              }
            }
          }
        } catch {
          // ContentRefs storage not available (V2)
        }

        // 著者のニックネームを取得
        const nicknameMap = new Map<string, string>()
        try {
          if (unsafeApi.query.Nickname?.Nicknames?.getValue) {
            // 全著者アドレスを収集
            const authors = new Set<string>()
            for (const entry of postEntries) {
              const author = entry.value?.author
              if (author && typeof author === 'string') {
                authors.add(author)
              }
            }
            // 各著者のニックネームを取得
            for (const author of authors) {
              try {
                const result = await unsafeApi.query.Nickname.Nicknames.getValue(author)
                if (result) {
                  let bytes: Uint8Array
                  if (typeof result?.asBytes === 'function') {
                    bytes = result.asBytes()
                  } else if (result instanceof Uint8Array) {
                    bytes = result
                  } else if (Array.isArray(result)) {
                    bytes = new Uint8Array(result)
                  } else {
                    bytes = new Uint8Array(result)
                  }
                  const decoded = new TextDecoder().decode(bytes)
                  if (decoded) {
                    nicknameMap.set(author, decoded)
                  }
                }
              } catch {
                // Individual nickname fetch failed
              }
            }
          }
        } catch {
          // Nickname pallet not available
        }
        
        const fetchedPosts: Post[] = postEntries.map((entry: any) => {
          const postId = Number(entry.keyArgs[0])
          const post = entry.value
          const contentRef = contentRefMap.get(postId)
          const author = post.author || 'unknown'
          return {
            id: postId,
            author,
            contentHash: post.content_hash?.asHex?.() || '',
            createdAt: Number(post.created_at || 0),
            parentId: post.parent_id !== undefined ? Number(post.parent_id) : null,
            contentRef,
            nickname: nicknameMap.get(author),
            reactionStats: undefined,
          }
        })

        // 最新順（作成ブロック番号の降順）でソート
        fetchedPosts.sort((a, b) => b.createdAt - a.createdAt)

        // 表示する投稿を50件に制限
        const displayPosts = fetchedPosts.slice(0, 50)

        // リアクション統計を取得（表示する投稿のみ）
        try {
          if (unsafeApi.query.Reaction?.ReactionStatsStorage?.getValue) {
            const statsPromises = displayPosts.map(async (post) => {
              try {
                const stats = await unsafeApi.query.Reaction.ReactionStatsStorage.getValue(post.id)
                if (stats) {
                  return {
                    postId: post.id,
                    stats: {
                      likes: Number(stats.likes || 0),
                      boosts: Number(stats.boosts || 0),
                      bads: Number(stats.bads || 0),
                    }
                  }
                }
              } catch {
                // Individual stats fetch failed
              }
              return null
            })
            const statsResults = await Promise.all(statsPromises)
            for (const result of statsResults) {
              if (result) {
                const post = displayPosts.find(p => p.id === result.postId)
                if (post) {
                  post.reactionStats = result.stats
                }
              }
            }
          }
        } catch {
          // Reaction pallet not available
        }

        setPosts(displayPosts)
      } catch (err) {
        debugError('[Timeline] Failed to fetch posts:', err)
      } finally {
        setIsLoading(false)
      }
    }

    fetchPosts()

    // Note: PAPI event subscription is different, skipping for now
    // TODO: Add event subscription for new posts
  }, [unsafeApi, refreshTrigger])

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
          nickname={post.nickname || 'Anarchy'}
          contentHash={post.contentHash}
          createdAt={post.createdAt}
          parentId={post.parentId}
          contentRef={post.contentRef}
          client={client}
          unsafeApi={unsafeApi}
          account={account}
          signer={signer}
          likes={post.reactionStats?.likes}
          boosts={post.reactionStats?.boosts}
          bads={post.reactionStats?.bads}
        />
      ))}
    </div>
  )
}

