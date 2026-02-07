'use client'

import { useState, useCallback } from 'react'
import { useWebAuthnContext } from '../contexts/WebAuthnContext'
import { RegisterResult } from '../types/webauthn'
import styles from './PasskeyRegister.module.css'

export interface PasskeyRegisterProps {
  /** PAPI API instance */
  api: any | null
  /** Signer for transactions */
  signer: any | null
  /** Default device name */
  defaultDeviceName?: string
  /** Callback on successful registration */
  onSuccess?: (result: RegisterResult) => void
  /** Callback on registration error */
  onError?: (error: any) => void
  /** Custom button text */
  buttonText?: string
  /** Whether to show device name input */
  showDeviceNameInput?: boolean
}

/**
 * Format registration status for display
 */
function getStatusMessage(status: string): string {
  switch (status) {
    case 'authenticating':
      return 'パスキーを作成中...'
    case 'extracting':
      return '公開鍵を取得中...'
    case 'submitting':
      return 'ブロックチェーンに登録中...'
    case 'confirming':
      return 'トランザクションを確認中...'
    case 'success':
      return '登録完了！'
    case 'error':
      return 'エラーが発生しました'
    default:
      return ''
  }
}

/**
 * PasskeyRegister - Component for registering a new passkey identity
 * 
 * Provides:
 * - Registration button with loading states
 * - Optional device name input
 * - Success/error feedback
 * - Retry functionality
 * 
 * @example
 * ```tsx
 * function RegistrationPage() {
 *   const { api, signer } = useApi()
 *   
 *   return (
 *     <WebAuthnGate>
 *       <PasskeyRegister 
 *         api={api}
 *         signer={signer}
 *         onSuccess={(result) => {
 *           console.log('Registered:', result.identityId)
 *         }}
 *       />
 *     </WebAuthnGate>
 *   )
 * }
 * ```
 */
export function PasskeyRegister({
  api,
  signer,
  defaultDeviceName = '',
  onSuccess,
  onError,
  buttonText = 'パスキーで登録',
  showDeviceNameInput = true,
}: PasskeyRegisterProps) {
  const [deviceName, setDeviceName] = useState(defaultDeviceName)

  // Use context instead of standalone hook - this ensures identity state is shared
  const { registrationStatus: status, registerPasskey, reset, error } = useWebAuthnContext()

  const handleRegister = useCallback(async () => {
    const result = await registerPasskey(deviceName || undefined)
    if (result.success) {
      onSuccess?.(result)
    } else if (result.error) {
      onError?.(result.error)
    }
  }, [registerPasskey, deviceName, onSuccess, onError])

  const handleReset = useCallback(() => {
    reset()
  }, [reset])

  const isProcessing = status !== 'idle' && status !== 'success' && status !== 'error'

  // Error state
  if (status === 'error' && error) {
    // Extract original error message if available
    const originalErrorMessage = error.originalError instanceof Error 
      ? error.originalError.message 
      : null

    return (
      <div className={styles.container}>
        <div className={styles.error}>
          <div className={styles.errorIcon}>✕</div>
          <h3>登録に失敗しました</h3>
          <p className={styles.errorMessage}>{error.message}</p>
          {originalErrorMessage && (
            <p className={styles.errorDetail}>
              詳細: {originalErrorMessage}
            </p>
          )}
          <div className={styles.buttonGroup}>
            <button
              className={styles.primaryButton}
              onClick={handleReset}
            >
              やり直す
            </button>
          </div>
        </div>
      </div>
    )
  }

  // Idle and processing states (success state is handled by parent showing PasskeySignPost)
  return (
    <div className={styles.container}>
      <div className={styles.form}>
        <div className={styles.header}>
          <div className={styles.passkeyIcon}>🔑</div>
          <h3>パスキーで登録</h3>
          <p className={styles.description}>
            パスキーを使って、パスワード不要で安全にAnarchyにアクセスできます。
          </p>
        </div>

        {showDeviceNameInput && (
          <div className={styles.inputGroup}>
            <label htmlFor="device-name" className={styles.label}>
              デバイス名（任意）
            </label>
            <input
              id="device-name"
              type="text"
              className={styles.input}
              value={deviceName}
              onChange={(e) => setDeviceName(e.target.value)}
              placeholder="例: MacBook Pro, iPhone"
              maxLength={64}
              disabled={isProcessing}
            />
            <span className={styles.hint}>
              複数デバイス管理時に識別しやすくなります
            </span>
          </div>
        )}

        <button
          className={styles.primaryButton}
          onClick={handleRegister}
          disabled={isProcessing || !api || !signer}
        >
          {isProcessing ? (
            <>
              <span className={styles.spinner} />
              {getStatusMessage(status)}
            </>
          ) : (
            buttonText
          )}
        </button>

        {(!api || !signer) && (
          <p className={styles.warning}>
            ブロックチェーンに接続中です...
          </p>
        )}

        <p className={styles.securityNote}>
          🛡️ パスキーはこのデバイスに安全に保存されます。
          <br />
          秘密鍵がサーバーに送信されることはありません。
        </p>
      </div>
    </div>
  )
}

export default PasskeyRegister
