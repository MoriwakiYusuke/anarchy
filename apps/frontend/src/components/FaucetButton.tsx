'use client'

import { useState, useRef, useCallback } from 'react'
import { PolkadotSigner } from 'polkadot-api/signer'
import { useFaucet, FaucetStatus } from '@/hooks/useFaucet'
import { useLocale } from '@/i18n'
import styles from './FaucetButton.module.css'

// 連打防止のクールダウン時間（ミリ秒）
const CLICK_COOLDOWN_MS = 1000
// 成功後のクールダウン時間（ミリ秒）
const SUCCESS_COOLDOWN_MS = 5000

interface FaucetButtonProps {
  client: any
  unsafeApi: any
  account: string | null
  signer: PolkadotSigner | null
  onSuccess?: () => void
}

/**
 * PoW Faucetボタンコンポーネント
 * 
 * ボタンクリックでPoW計算を開始し、成功時にトランザクションを送信
 */
export function FaucetButton({ client, unsafeApi, account, signer, onSuccess }: FaucetButtonProps) {
  const { t } = useLocale()
  const [cooldown, setCooldown] = useState(false)
  const lastClickRef = useRef<number>(0)
  
  const { status, error, progress, startMining, cancel } = useFaucet({
    client,
    unsafeApi,
    account,
    signer,
    onSuccess: () => {
      // 成功後のクールダウン
      setCooldown(true)
      setTimeout(() => setCooldown(false), SUCCESS_COOLDOWN_MS)
      onSuccess?.()
    },
  })

  const isProcessing = status === 'mining' || status === 'submitting'
  // Disabled when no account/signer, or during cooldown
  const isDisabled = !account || !signer || cooldown

  const handleClick = useCallback(() => {
    // 連打防止: 前回クリックから一定時間経過していなければ無視
    const now = Date.now()
    if (now - lastClickRef.current < CLICK_COOLDOWN_MS) {
      return
    }
    lastClickRef.current = now

    if (isProcessing) {
      cancel()
    } else if (!cooldown) {
      startMining()
    }
  }, [isProcessing, cooldown, cancel, startMining])

  const getButtonText = (): string => {
    switch (status) {
      case 'mining':
        if (progress) {
          const hashRateK = (progress.hashRate / 1000).toFixed(1)
          return `${t('faucet.mining')} (${hashRateK} kH/s)`
        }
        return t('faucet.mining')
      case 'submitting':
        return t('faucet.submitting')
      case 'success':
        return t('faucet.success')
      case 'error':
        return t('faucet.error')
      default:
        return t('faucet.button')
    }
  }

  const getStatusClass = (): string => {
    switch (status) {
      case 'mining':
      case 'submitting':
        return styles.processing
      case 'success':
        return styles.success
      case 'error':
        return styles.error
      default:
        return ''
    }
  }

  const getErrorMessage = (): string | null => {
    if (!error) return null
    
    // エラーコードに基づいてローカライズされたメッセージを返す
    const errorMessages: Record<string, string> = {
      AlreadyClaimed: t('error.alreadyClaimed'),
      ChallengeExpired: t('error.challengeExpired'),
      InvalidProof: t('error.invalidProof'),
      BlockNotFound: t('error.blockNotFound'),
      InsufficientBalance: t('error.insufficientBalance'),
      NetworkError: t('error.faucetNetworkError'),
    }
    
    return errorMessages[error.code] || error.message
  }

  return (
    <div className={styles.container}>
      <button
        className={`${styles.button} ${getStatusClass()}`}
        onClick={handleClick}
        disabled={isDisabled}
        title={isProcessing ? t('faucet.cancel') : undefined}
      >
        {isProcessing && (
          <span className={styles.spinner} />
        )}
        <span className={styles.text}>{getButtonText()}</span>
      </button>
      
      {error && (
        <div className={styles.errorMessage}>
          {getErrorMessage()}
        </div>
      )}
      
      {status === 'mining' && progress && (
        <div className={styles.progressInfo}>
          <span className={styles.elapsed}>
            {(progress.elapsed / 1000).toFixed(1)}s
          </span>
        </div>
      )}
    </div>
  )
}
