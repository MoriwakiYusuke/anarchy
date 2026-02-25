'use client'

/**
 * NicknameSettings Component
 * 
 * T-041: Nickname registration settings UI
 * Allows users to set, update, or clear their on-chain nickname
 */

import React, { useState, useEffect, useCallback, useMemo } from 'react'
import type { PolkadotSigner, PolkadotClient } from 'polkadot-api'
import { useNickname } from '@/hooks/useNickname'
import { useLocale } from '@/i18n'
import styles from './NicknameSettings.module.css'

/** Maximum nickname length in bytes */
const MAX_NICKNAME_BYTES = 128
/** Threshold for warning styling (90% of max) */
const WARNING_THRESHOLD = Math.floor(MAX_NICKNAME_BYTES * 0.9)

interface NicknameSettingsProps {
  /** PAPI client instance */
  client: PolkadotClient | unknown
  /** Unsafe API for queries and transactions */
  unsafeApi: unknown
  /** Current account ID */
  accountId: string
  /** Signer for transactions */
  signer: PolkadotSigner | unknown
  /** Optional callback when nickname changes */
  onNicknameChange?: (nickname: string | null) => void
}

/**
 * Calculate byte length of a UTF-8 string
 */
function getByteLength(str: string): number {
  return new TextEncoder().encode(str).length
}

export default function NicknameSettings({
  client,
  unsafeApi,
  accountId,
  signer,
  onNicknameChange,
}: NicknameSettingsProps): React.ReactElement {
  const { t } = useLocale()
  const {
    nickname,
    error,
    state,
    setNickname,
    clearNickname,
  } = useNickname({
    unsafeApi,
    client,
    accountId,
    signer,
    onSuccess: () => {
      onNicknameChange?.(inputValue || null)
    },
  })

  const [inputValue, setInputValue] = useState('')
  const [wasCleared, setWasCleared] = useState(false)

  // Sync input with current nickname on load
  useEffect(() => {
    if (nickname !== null && nickname !== undefined) {
      setInputValue(nickname)
    }
  }, [nickname])

  // Track if last action was clear
  useEffect(() => {
    if (state.status === 'success' && !nickname) {
      setWasCleared(true)
    } else {
      setWasCleared(false)
    }
  }, [state.status, nickname])

  const byteCount = useMemo(() => getByteLength(inputValue), [inputValue])
  const isOverLimit = byteCount > MAX_NICKNAME_BYTES
  const isNearLimit = byteCount >= WARNING_THRESHOLD && !isOverLimit
  const isPending = state.status === 'pending'
  const isError = state.status === 'error'
  const isSuccess = state.status === 'success'
  const hasNickname = !!nickname

  const handleInputChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    setInputValue(e.target.value)
  }, [])

  const handleSubmit = useCallback(async (e: React.FormEvent) => {
    e.preventDefault()
    if (inputValue.trim() && !isPending && !isOverLimit) {
      await setNickname(inputValue.trim())
    }
  }, [inputValue, isPending, isOverLimit, setNickname])

  const handleClear = useCallback(async () => {
    if (!isPending) {
      await clearNickname()
      setInputValue('')
    }
  }, [isPending, clearNickname])

  const getButtonText = (): string => {
    if (isPending) return t('nickname.setting')
    return t('nickname.set')
  }

  const getStatusMessage = (): string | null => {
    if (isSuccess && wasCleared) return t('nickname.cleared')
    if (isSuccess && nickname) return t('nickname.success')
    return null
  }

  return (
    <div className={styles.container}>
      <h3 className={styles.title}>{t('nickname.title')}</h3>
      
      <form onSubmit={handleSubmit} className={styles.form}>
        <div className={styles.inputWrapper}>
          <input
            type="text"
            value={inputValue}
            onChange={handleInputChange}
            placeholder={t('nickname.placeholder')}
            disabled={isPending}
            className={`${styles.input} ${isError ? styles.inputError : ''}`}
            aria-label={t('nickname.title')}
          />
          
          <span 
            className={`${styles.counter} ${isNearLimit ? styles.warning : ''} ${isOverLimit ? styles.error : ''}`}
          >
            {byteCount}/{MAX_NICKNAME_BYTES}
          </span>
        </div>

        <div className={styles.actions}>
          <button
            type="submit"
            disabled={isPending || isOverLimit || !inputValue.trim()}
            className={styles.setButton}
          >
            {getButtonText()}
          </button>

          {hasNickname && (
            <button
              type="button"
              onClick={handleClear}
              disabled={isPending}
              className={styles.clearButton}
            >
              {t('nickname.clear')}
            </button>
          )}
        </div>
      </form>

      {/* Status messages */}
      {isError && error && (
        <p className={`${styles.message} ${styles.error}`}>
          {t(error)}
        </p>
      )}

      {isSuccess && (
        <p className={`${styles.message} ${styles.success}`}>
          {getStatusMessage()}
        </p>
      )}
    </div>
  )
}
