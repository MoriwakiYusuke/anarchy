'use client'

import { useState, useCallback } from 'react'
import { useWebAuthnSigning, UseWebAuthnSigningResult } from '../hooks/useWebAuthnSigning'
import { usePostCost, calculatePostCost, PostCostConfig } from '../hooks/usePostCost'
import {
  SigningStatus,
  PostResult,
  SigningError,
  SIGNING_ERROR_MESSAGES,
} from '../types/webauthn'
import styles from './PasskeySignPost.module.css'

export interface PasskeySignPostProps {
  /** PAPI API instance */
  api: any | null
  /** Polkadot signer */
  signer: any | null
  /** User's identity ID */
  identityId: bigint
  /** User's passkey ID (Blake2-256 hash) */
  passkeyId: Uint8Array
  /** Base64URL-encoded credential ID */
  credentialId: string
  /** Called when post is successful */
  onSuccess?: (result: PostResult) => void
  /** Called on error */
  onError?: (error: SigningError) => void
  /** Placeholder text */
  placeholder?: string
  /** Maximum content length in bytes */
  maxBytes?: number
  /** Parent post ID for replies */
  parentId?: number
}

/** Status message component */
function StatusMessage({ status, error }: { status: SigningStatus; error: SigningError | null }) {
  const getStatusMessage = (): string => {
    switch (status) {
      case 'hashing':
        return '投稿内容を準備中...'
      case 'authenticating':
        return 'パスキーで署名中... デバイスを確認してください'
      case 'submitting':
        return 'ブロックチェーンに送信中...'
      case 'confirming':
        return 'トランザクション確認中...'
      case 'success':
        return '投稿が完了しました！'
      case 'error':
        return error?.message || 'エラーが発生しました'
      default:
        return ''
    }
  }

  if (status === 'idle') return null

  const isError = status === 'error'
  const isSuccess = status === 'success'
  const isProcessing = !isError && !isSuccess

  return (
    <div
      className={`${styles.status} ${isError ? styles.error : ''} ${isSuccess ? styles.success : ''}`}
      role={isError ? 'alert' : 'status'}
      aria-live="polite"
    >
      {isProcessing && <span className={styles.spinner} aria-hidden="true" />}
      {getStatusMessage()}
    </div>
  )
}

/** Cost display component */
function CostDisplay({
  byteCount,
  costConfig,
}: {
  byteCount: number
  costConfig: PostCostConfig
}) {
  const estimatedCost = calculatePostCost(byteCount, costConfig)

  return (
    <div className={styles.costInfo}>
      <span className={styles.byteCount}>{byteCount.toLocaleString()} bytes</span>
      <span className={styles.cost}>
        {costConfig.isLoading ? (
          '読込中...'
        ) : (
          <>
            コスト: {estimatedCost.toFixed(1)} $moral
            {!costConfig.isFromChain && (
              <span title="チェーンから取得できないためデフォルト値を使用" className={styles.fallbackIndicator}>
                {' '}
                *
              </span>
            )}
          </>
        )}
      </span>
    </div>
  )
}

/**
 * PasskeySignPost - WebAuthn署名付き投稿フォーム
 *
 * パスキーでコンテンツに署名してブロックチェーンに投稿するコンポーネント。
 * WYSIWYS (What You Sign Is What You See) パターンを使用。
 *
 * @example
 * ```tsx
 * <PasskeySignPost
 *   api={api}
 *   signer={signer}
 *   identityId={42n}
 *   passkeyId={passkeyId}
 *   credentialId="base64url-cred-id"
 *   onSuccess={(result) => console.log('Posted!', result.postId)}
 * />
 * ```
 */
export function PasskeySignPost({
  api,
  signer,
  identityId,
  passkeyId,
  credentialId,
  onSuccess,
  onError,
  placeholder = '今、何を考えていますか？',
  maxBytes = 10000,
  parentId,
}: PasskeySignPostProps) {
  const [content, setContent] = useState('')
  const [submittedPostId, setSubmittedPostId] = useState<bigint | null>(null)

  // Get cost configuration from chain
  const costConfig = usePostCost(api)

  // WebAuthn signing hook
  const { status, sign, reset, error } = useWebAuthnSigning({
    api,
    signer,
    identityId,
    passkeyId,
    credentialId,
    onSuccess: (result) => {
      if (result.postId) {
        setSubmittedPostId(result.postId)
      }
      onSuccess?.(result)
    },
    onError,
  })

  // Calculate byte count
  const contentBytes = new TextEncoder().encode(content)
  const byteCount = contentBytes.length
  const isOverLimit = byteCount > maxBytes

  // Check if form is ready
  const isReady = api && signer && identityId && passkeyId && credentialId
  const isProcessing = status !== 'idle' && status !== 'success' && status !== 'error'
  const canSubmit = isReady && content.trim().length > 0 && !isOverLimit && !isProcessing

  // Handle form submission
  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault()
      if (!canSubmit) return

      const result = await sign(content.trim(), parentId)
      if (result.success) {
        setContent('')
      }
    },
    [canSubmit, sign, content, parentId]
  )

  // Handle retry after error
  const handleRetry = useCallback(() => {
    reset()
  }, [reset])

  // Handle new post after success
  const handleNewPost = useCallback(() => {
    reset()
    setSubmittedPostId(null)
  }, [reset])

  return (
    <div className={styles.container}>
      {/* Success state */}
      {status === 'success' && submittedPostId && (
        <div className={styles.successContainer}>
          <div className={styles.successIcon} aria-hidden="true">
            ✓
          </div>
          <p className={styles.successMessage}>投稿が完了しました！</p>
          <p className={styles.postIdDisplay}>Post ID: {submittedPostId.toString()}</p>
          <button type="button" className={styles.newPostButton} onClick={handleNewPost}>
            新しい投稿を作成
          </button>
        </div>
      )}

      {/* Error state */}
      {status === 'error' && (
        <div className={styles.errorContainer}>
          <div className={styles.errorIcon} aria-hidden="true">
            ✕
          </div>
          <p className={styles.errorMessage}>{error?.message || 'エラーが発生しました'}</p>
          <button type="button" className={styles.retryButton} onClick={handleRetry}>
            再試行
          </button>
        </div>
      )}

      {/* Form state */}
      {(status === 'idle' || isProcessing) && (
        <form className={styles.form} onSubmit={handleSubmit}>
          <textarea
            className={`${styles.textarea} ${isOverLimit ? styles.overLimit : ''}`}
            placeholder={placeholder}
            value={content}
            onChange={(e) => setContent(e.target.value)}
            disabled={isProcessing}
            rows={4}
            aria-label="投稿内容"
            aria-invalid={isOverLimit}
            aria-describedby={isOverLimit ? 'byte-limit-error' : undefined}
          />

          {isOverLimit && (
            <p id="byte-limit-error" className={styles.limitError} role="alert">
              コンテンツが長すぎます（最大 {maxBytes.toLocaleString()} bytes）
            </p>
          )}

          <CostDisplay byteCount={byteCount} costConfig={costConfig} />

          <StatusMessage status={status} error={error} />

          <div className={styles.footer}>
            <button
              type="submit"
              className={styles.submitButton}
              disabled={!canSubmit}
              aria-busy={isProcessing}
            >
              {isProcessing ? (
                <>
                  <span className={styles.buttonSpinner} aria-hidden="true" />
                  処理中...
                </>
              ) : (
                <>
                  <span className={styles.passkeyIcon} aria-hidden="true">
                    🔐
                  </span>
                  署名して投稿
                </>
              )}
            </button>
          </div>

          <p className={styles.securityNote}>
            <small>パスキーで署名された投稿は、あなたのIDで認証されます。</small>
          </p>
        </form>
      )}

      {/* Not ready state */}
      {!isReady && status === 'idle' && (
        <div className={styles.notReadyMessage}>
          <p>投稿するには、パスキーの登録とウォレット接続が必要です。</p>
        </div>
      )}
    </div>
  )
}

export default PasskeySignPost
