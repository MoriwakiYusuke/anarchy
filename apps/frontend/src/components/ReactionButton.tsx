/**
 * ReactionButton Component - X-style minimal design with PoW mining
 * 
 * Feature: 017-reaction-mining
 */

'use client'

import React, { useState, useCallback, useEffect, useRef } from 'react'
import { useReactionMining, type MiningError } from '@/hooks/useReactionMining'
import { ReactionType, type ReactionResult } from '@/services/reactionService'
import type { PolkadotSigner } from 'polkadot-api/signer'
import styles from './ReactionButton.module.css'

/** Loading spinner icon */
const SpinnerIcon: React.FC = () => (
  <svg className={styles.spinner} viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" strokeWidth="2">
    <circle cx="12" cy="12" r="10" strokeOpacity="0.25" />
    <path d="M12 2a10 10 0 0 1 10 10" strokeLinecap="round" />
  </svg>
)

/** Like icon (heart outline) */
const HeartIcon: React.FC<{ filled?: boolean }> = ({ filled }) => (
  <svg viewBox="0 0 24 24" width="18" height="18" fill={filled ? 'currentColor' : 'none'} stroke="currentColor" strokeWidth="2">
    <path d="M12 21.35l-1.45-1.32C5.4 15.36 2 12.28 2 8.5 2 5.42 4.42 3 7.5 3c1.74 0 3.41.81 4.5 2.09C13.09 3.81 14.76 3 16.5 3 19.58 3 22 5.42 22 8.5c0 3.78-3.4 6.86-8.55 11.54L12 21.35z" />
  </svg>
)

/** Boost icon (retweet/repost) */
const BoostIcon: React.FC = () => (
  <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M17 1l4 4-4 4" />
    <path d="M3 11V9a4 4 0 014-4h14" />
    <path d="M7 23l-4-4 4-4" />
    <path d="M21 13v2a4 4 0 01-4 4H3" />
  </svg>
)

/** Bad icon (thumbs down) */
const BadIcon: React.FC = () => (
  <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M10 15v4a3 3 0 003 3l4-9V2H5.72a2 2 0 00-2 1.7l-1.38 9a2 2 0 002 2.3zm7-13h2.67A2.31 2.31 0 0122 4v7a2.31 2.31 0 01-2.33 2H17" />
  </svg>
)

export { ReactionType }

export interface ReactionButtonProps {
  /** Target post ID */
  postId: number
  /** Current like count */
  likes?: number
  /** Current boost count */
  boosts?: number
  /** Current bad count */
  bads?: number
  /** PAPI client */
  client?: unknown
  /** PAPI unsafe API */
  unsafeApi?: unknown
  /** User's account address */
  account?: string | null
  /** Polkadot signer */
  signer?: PolkadotSigner | null
  /** Callback after successful reaction */
  onReactionSuccess?: (type: ReactionType, reward?: bigint) => void
}

export const ReactionButton: React.FC<ReactionButtonProps> = ({
  postId,
  likes = 0,
  boosts = 0,
  bads = 0,
  client,
  unsafeApi,
  account = null,
  signer = null,
  onReactionSuccess,
}) => {
  const [selectedType, setSelectedType] = useState<ReactionType | null>(null)
  const [loading, setLoading] = useState<ReactionType | null>(null)
  const [localCounts, setLocalCounts] = useState({ likes, boosts, bads })
  // 投稿ごとに1回のみリアクション可能（Like/Boost/Badのいずれか）
  const [hasReacted, setHasReacted] = useState(false)
  const [reactedType, setReactedType] = useState<ReactionType | null>(null)
  // チェーン側でAlreadyReactedエラーが返ってきた場合
  const [alreadyReactedError, setAlreadyReactedError] = useState(false)
  // クロージャの問題を解決するため、現在処理中のタイプをrefで追跡
  const pendingTypeRef = useRef<ReactionType | null>(null)

  // disconnect時に状態リセット
  useEffect(() => {
    if (!account) {
      setSelectedType(null)
      setLoading(null)
      setHasReacted(false)
      setReactedType(null)
      setAlreadyReactedError(false)
      pendingTypeRef.current = null
    }
  }, [account])

  const {
    status,
    progress,
    startMining,
  } = useReactionMining({
    client,
    unsafeApi,
    account,
    signer,
    onSuccess: useCallback((result: ReactionResult) => {
      const type = pendingTypeRef.current
      if (type === ReactionType.Like) {
        setLocalCounts((c) => ({ ...c, likes: c.likes + 1 }))
      } else if (type === ReactionType.Boost) {
        setLocalCounts((c) => ({ ...c, boosts: c.boosts + 1 }))
      } else if (type === ReactionType.Bad) {
        setLocalCounts((c) => ({ ...c, bads: c.bads + 1 }))
      }
      // 1回でもリアクションしたら全ボタン無効化
      setHasReacted(true)
      setReactedType(type)
      if (type) {
        onReactionSuccess?.(type, result.reward)
      }
      setSelectedType(null)
      setLoading(null)
      pendingTypeRef.current = null
    }, [onReactionSuccess]),
    onError: useCallback((err: MiningError) => {
      console.error('[ReactionButton] onError:', err)
      // チェーン側でAlreadyReactedエラーの場合、全ボタン無効化 + エラー表示
      if (err.code === 'AlreadyReacted') {
        setHasReacted(true)
        setAlreadyReactedError(true)
      }
      setLoading(null)
    }, []),
  })

  const isLoading = (type: ReactionType): boolean => {
    return loading === type
  }

  const handleClick = useCallback((type: ReactionType) => {
    // 既にリアクション済みの場合は全て無視（投稿ごとに1回のみ）
    if (hasReacted) {
      return
    }
    // mining or submitting中は無視（idleまたはerrorからは再試行可能）
    if (status === 'mining' || status === 'submitting') {
      return
    }
    if (!client || !unsafeApi || !account || !signer) {
      return // 接続がない場合は何もしない
    }

    setSelectedType(type)
    setLoading(type)
    pendingTypeRef.current = type
    startMining(BigInt(postId), type)
  }, [hasReacted, status, client, unsafeApi, account, signer, postId, startMining])

  const getButtonContent = (type: ReactionType, count: number, icon: React.ReactNode, activeIcon: React.ReactNode, isActive: boolean) => {
    if (isLoading(type)) {
      return (
        <>
          <SpinnerIcon />
          {count > 0 && <span className={styles.count}>{count}</span>}
        </>
      )
    }
    return (
      <>
        {isActive ? activeIcon : icon}
        {count > 0 && <span className={styles.count}>{count}</span>}
      </>
    )
  }

  const isNotConnected = !client || !unsafeApi || !account || !signer
  // マイニング/送信中は全ボタン無効化（同時押し防止）
  const isProcessing = loading !== null

  return (
    <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center' }}>
      <div className={styles.container}>
        <button
          type="button"
          className={`${styles.btn} ${styles.likeBtn} ${reactedType === ReactionType.Like ? styles.active : ''}`}
          onClick={() => handleClick(ReactionType.Like)}
          disabled={isNotConnected || hasReacted || isProcessing}
          aria-label="Like"
        >
          {getButtonContent(
            ReactionType.Like,
            localCounts.likes,
            <HeartIcon filled={false} />,
            <HeartIcon filled={true} />,
            reactedType === ReactionType.Like
          )}
        </button>

        <button
          type="button"
          className={`${styles.btn} ${styles.boostBtn} ${reactedType === ReactionType.Boost ? styles.active : ''}`}
          onClick={() => handleClick(ReactionType.Boost)}
          disabled={isNotConnected || hasReacted || isProcessing}
          aria-label="Boost"
        >
          {getButtonContent(
            ReactionType.Boost,
            localCounts.boosts,
            <BoostIcon />,
            <BoostIcon />,
            reactedType === ReactionType.Boost
          )}
        </button>

        <button
          type="button"
          className={`${styles.btn} ${styles.badBtn} ${reactedType === ReactionType.Bad ? styles.active : ''}`}
          onClick={() => handleClick(ReactionType.Bad)}
          disabled={isNotConnected || hasReacted || isProcessing}
          aria-label="Bad"
        >
          {getButtonContent(
            ReactionType.Bad,
            localCounts.bads,
            <BadIcon />,
            <BadIcon />,
            reactedType === ReactionType.Bad
          )}
        </button>
      </div>
      {isNotConnected && (
        <span style={{ color: '#888', fontSize: '0.625rem', marginTop: '0.25rem' }}>接続してください</span>
      )}
      {status === 'mining' && progress && (
        <span style={{ color: '#888', fontSize: '0.625rem', marginTop: '0.25rem' }}>
          {(progress.hashRate / 1000).toFixed(1)} kH/s · {(progress.elapsedMs / 1000).toFixed(1)}s
        </span>
      )}
      {status === 'submitting' && (
        <span style={{ color: '#888', fontSize: '0.625rem', marginTop: '0.25rem' }}>送信中...</span>
      )}
      {alreadyReactedError && (
        <span style={{ color: '#f4212e', fontSize: '0.625rem', marginTop: '0.25rem' }}>すでに反応済みです</span>
      )}
    </div>
  )
}

export default ReactionButton
