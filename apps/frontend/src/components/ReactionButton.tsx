/**
 * ReactionButton Component
 * 
 * UI component for submitting PoW-verified reactions (Like/Boost/Bad) to posts.
 * Displays mining progress and handles the complete reaction workflow.
 * 
 * Feature: 017-reaction-mining
 */

'use client'

import React, { useCallback, useState } from 'react'
import { useReactionMining, ReactionType, MiningStatus } from '@/hooks/useReactionMining'
import type { ReactionResult } from '@/services/reactionService'
import type { PolkadotSigner } from 'polkadot-api/signer'

/** ReactionButton props */
export interface ReactionButtonProps {
  /** Target post ID */
  postId: bigint
  /** Current like count (from chain) */
  likes?: number
  /** Current boost count (from chain) */
  boosts?: number
  /** Current bad count (from chain) */
  bads?: number
  /** Whether user has already reacted */
  hasReacted?: boolean
  /** PAPI client */
  client: any
  /** PAPI unsafe API */
  unsafeApi: any
  /** User's account address */
  account: string | null
  /** Polkadot signer */
  signer: PolkadotSigner | null
  /** Callback after successful reaction */
  onReactionSuccess?: (type: ReactionType, reward?: bigint) => void
  /** Additional CSS class */
  className?: string
}

/** Reaction option button */
interface ReactionOptionProps {
  type: ReactionType
  count: number
  icon: string
  label: string
  selected: boolean
  disabled: boolean
  onClick: () => void
}

const ReactionOption: React.FC<ReactionOptionProps> = ({
  type: _type,
  count,
  icon,
  label,
  selected,
  disabled,
  onClick,
}) => (
  <button
    onClick={onClick}
    disabled={disabled}
    className={`
      flex items-center gap-1 px-3 py-1.5 rounded-full text-sm
      transition-all duration-200
      ${selected 
        ? 'bg-primary-500 text-white' 
        : 'bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300'
      }
      ${disabled 
        ? 'opacity-50 cursor-not-allowed' 
        : 'hover:bg-primary-100 dark:hover:bg-gray-700 cursor-pointer'
      }
    `}
    aria-label={`${label} (${count})`}
  >
    <span role="img" aria-hidden="true">{icon}</span>
    <span>{count}</span>
  </button>
)

/** Mining progress overlay */
interface MiningOverlayProps {
  status: MiningStatus
  progress: {
    hashRate: number
    elapsedMs: number
    difficulty: number
  } | null
  error: string | null
  onCancel: () => void
  onResume: () => void
}

const MiningOverlay: React.FC<MiningOverlayProps> = ({
  status,
  progress,
  error,
  onCancel,
  onResume,
}) => {
  if (status === 'idle' || status === 'success') {
    return null
  }
  
  const formatTime = (ms: number): string => {
    const seconds = Math.floor(ms / 1000)
    const minutes = Math.floor(seconds / 60)
    if (minutes > 0) {
      return `${minutes}m ${seconds % 60}s`
    }
    return `${seconds}s`
  }
  
  const formatHashRate = (rate: number): string => {
    if (rate >= 1000000) {
      return `${(rate / 1000000).toFixed(1)}M H/s`
    }
    if (rate >= 1000) {
      return `${(rate / 1000).toFixed(1)}K H/s`
    }
    return `${rate} H/s`
  }
  
  return (
    <div className="mt-2 p-3 rounded-lg bg-gray-50 dark:bg-gray-900 border border-gray-200 dark:border-gray-700">
      {status === 'mining' && progress && (
        <div className="space-y-2">
          <div className="flex items-center gap-2">
            <div className="animate-spin w-4 h-4 border-2 border-primary-500 border-t-transparent rounded-full" />
            <span className="text-sm font-medium text-gray-700 dark:text-gray-300">
              Mining PoW...
            </span>
          </div>
          <div className="text-xs text-gray-500 dark:text-gray-400 space-y-1">
            <div className="flex justify-between">
              <span>Hash Rate:</span>
              <span className="font-mono">{formatHashRate(progress.hashRate)}</span>
            </div>
            <div className="flex justify-between">
              <span>Time:</span>
              <span className="font-mono">{formatTime(progress.elapsedMs)}</span>
            </div>
            <div className="flex justify-between">
              <span>Difficulty:</span>
              <span className="font-mono">{progress.difficulty} bits</span>
            </div>
          </div>
          <button
            onClick={onCancel}
            className="w-full mt-2 px-3 py-1 text-xs text-red-600 dark:text-red-400 border border-red-300 dark:border-red-700 rounded hover:bg-red-50 dark:hover:bg-red-900/20 transition-colors"
          >
            Cancel
          </button>
        </div>
      )}
      
      {status === 'paused' && (
        <div className="space-y-2">
          <div className="flex items-center gap-2 text-yellow-600 dark:text-yellow-400">
            <span>⏸</span>
            <span className="text-sm font-medium">Mining Paused</span>
          </div>
          <p className="text-xs text-gray-500 dark:text-gray-400">
            Mining pauses when this tab is in the background.
          </p>
          <button
            onClick={onResume}
            className="w-full px-3 py-1 text-xs text-primary-600 dark:text-primary-400 border border-primary-300 dark:border-primary-700 rounded hover:bg-primary-50 dark:hover:bg-primary-900/20 transition-colors"
          >
            Resume Mining
          </button>
        </div>
      )}
      
      {status === 'submitting' && (
        <div className="flex items-center gap-2">
          <div className="animate-pulse w-4 h-4 bg-primary-500 rounded-full" />
          <span className="text-sm font-medium text-gray-700 dark:text-gray-300">
            Submitting transaction...
          </span>
        </div>
      )}
      
      {status === 'error' && error && (
        <div className="space-y-2">
          <div className="flex items-center gap-2 text-red-600 dark:text-red-400">
            <span>❌</span>
            <span className="text-sm font-medium">Error</span>
          </div>
          <p className="text-xs text-red-500 dark:text-red-400">{error}</p>
          <button
            onClick={onCancel}
            className="w-full px-3 py-1 text-xs text-gray-600 dark:text-gray-400 border border-gray-300 dark:border-gray-700 rounded hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors"
          >
            Dismiss
          </button>
        </div>
      )}
    </div>
  )
}

/**
 * ReactionButton component
 * 
 * Displays Like/Boost/Bad buttons and handles PoW mining workflow.
 */
export const ReactionButton: React.FC<ReactionButtonProps> = ({
  postId,
  likes = 0,
  boosts = 0,
  bads = 0,
  hasReacted = false,
  client,
  unsafeApi,
  account,
  signer,
  onReactionSuccess,
  className = '',
}) => {
  const [selectedType, setSelectedType] = useState<ReactionType | null>(null)
  
  const {
    status,
    error,
    progress,
    result,
    startMining,
    cancel,
    resume,
  } = useReactionMining({
    client,
    unsafeApi,
    account,
    signer,
    onSuccess: useCallback((res: ReactionResult) => {
      if (selectedType) {
        onReactionSuccess?.(selectedType, res.reward)
      }
      setSelectedType(null)
    }, [selectedType, onReactionSuccess]),
  })
  
  const handleReactionClick = useCallback((type: ReactionType) => {
    if (hasReacted || !account || !signer) {
      return
    }
    
    setSelectedType(type)
    startMining(postId, type)
  }, [hasReacted, account, signer, postId, startMining])
  
  const isDisabled = hasReacted || !account || !signer || status !== 'idle'
  
  return (
    <div className={`reaction-button ${className}`}>
      <div className="flex items-center gap-2">
        <ReactionOption
          type={ReactionType.Like}
          count={likes + (result && selectedType === ReactionType.Like ? 1 : 0)}
          icon="👍"
          label="Like"
          selected={selectedType === ReactionType.Like && status === 'success'}
          disabled={isDisabled}
          onClick={() => handleReactionClick(ReactionType.Like)}
        />
        <ReactionOption
          type={ReactionType.Boost}
          count={boosts + (result && selectedType === ReactionType.Boost ? 1 : 0)}
          icon="🚀"
          label="Boost"
          selected={selectedType === ReactionType.Boost && status === 'success'}
          disabled={isDisabled}
          onClick={() => handleReactionClick(ReactionType.Boost)}
        />
        <ReactionOption
          type={ReactionType.Bad}
          count={bads + (result && selectedType === ReactionType.Bad ? 1 : 0)}
          icon="👎"
          label="Bad"
          selected={selectedType === ReactionType.Bad && status === 'success'}
          disabled={isDisabled}
          onClick={() => handleReactionClick(ReactionType.Bad)}
        />
      </div>
      
      <MiningOverlay
        status={status}
        progress={progress}
        error={error?.message || null}
        onCancel={cancel}
        onResume={resume}
      />
      
      {status === 'success' && result?.reward && (
        <div className="mt-2 text-xs text-green-600 dark:text-green-400 flex items-center gap-1">
          <span>✓</span>
          <span>Author received {Number(result.reward) / 1e12} MORAL reward!</span>
        </div>
      )}
    </div>
  )
}

export default ReactionButton
