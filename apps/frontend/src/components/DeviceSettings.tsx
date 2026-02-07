'use client'

import { useState, useCallback } from 'react'
import styles from './DeviceSettings.module.css'
import { useWebAuthnContext, StoredCredential } from '../contexts/WebAuthnContext'

export interface DeviceSettingsProps {
  /** Optional title for the section */
  title?: string
  /** Optional class name for custom styling */
  className?: string
  /** Callback when a new passkey is successfully added */
  onPasskeyAdded?: () => void
  /** Callback on error */
  onError?: (error: Error) => void
}

/**
 * DeviceSettings Component
 * 
 * Displays registered passkeys and allows adding new devices.
 * 
 * @example
 * ```tsx
 * <DeviceSettings 
 *   title="デバイス管理"
 *   onPasskeyAdded={() => toast.success('デバイスを追加しました')}
 * />
 * ```
 */
export function DeviceSettings({
  title = 'デバイス管理',
  className,
  onPasskeyAdded,
  onError,
}: DeviceSettingsProps) {
  const {
    identity,
    persistedCredentials,
    registrationStatus,
    addPasskey,
    removeCredential,
    switchCredential,
    error,
  } = useWebAuthnContext()

  const [isAddingDevice, setIsAddingDevice] = useState(false)
  const [deviceName, setDeviceName] = useState('')
  const [showAddForm, setShowAddForm] = useState(false)

  // Handle add passkey
  const handleAddPasskey = useCallback(async () => {
    if (!deviceName.trim()) return
    
    setIsAddingDevice(true)
    try {
      const result = await addPasskey(deviceName.trim())
      if (result.success) {
        setDeviceName('')
        setShowAddForm(false)
        onPasskeyAdded?.()
      } else if (result.error) {
        onError?.(new Error(result.error.message))
      }
    } catch (err) {
      onError?.(err instanceof Error ? err : new Error('Failed to add passkey'))
    } finally {
      setIsAddingDevice(false)
    }
  }, [deviceName, addPasskey, onPasskeyAdded, onError])

  // Handle remove credential (just from local storage, not from chain)
  const handleRemoveCredential = useCallback((credential: StoredCredential) => {
    if (confirm(`「${credential.deviceName || credential.credentialId.slice(0, 8)}」をこのデバイスから削除しますか？\n\n注: ブロックチェーン上のパスキーは削除されません。`)) {
      removeCredential(credential.credentialId)
    }
  }, [removeCredential])

  // Handle switch credential
  const handleSwitchCredential = useCallback((credential: StoredCredential) => {
    switchCredential(credential.credentialId)
  }, [switchCredential])

  // Format date
  const formatDate = (timestamp: number) => {
    return new Date(timestamp).toLocaleDateString('ja-JP', {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
    })
  }

  // Check if user has an identity (required for adding passkeys)
  const canAddPasskey = identity !== null

  // Check how many passkeys are registered
  const credentialCount = persistedCredentials.length

  return (
    <div className={`${styles.container} ${className || ''}`}>
      <h3 className={styles.title}>{title}</h3>
      
      {/* Credential List */}
      <div className={styles.credentialList}>
        {credentialCount === 0 ? (
          <div className={styles.emptyState}>
            <p>登録済みのデバイスはありません</p>
            <p className={styles.emptyHint}>
              パスキーを登録して投稿を始めましょう
            </p>
          </div>
        ) : (
          <>
            <div className={styles.listHeader}>
              <span>登録済みデバイス ({credentialCount})</span>
            </div>
            <ul className={styles.list}>
              {persistedCredentials.map((credential) => {
                const isCurrent = identity?.credentialId === credential.credentialId
                
                return (
                  <li
                    key={credential.credentialId}
                    className={`${styles.credentialItem} ${isCurrent ? styles.current : ''}`}
                  >
                    <div className={styles.credentialInfo}>
                      <div className={styles.deviceIcon}>🔑</div>
                      <div className={styles.deviceDetails}>
                        <span className={styles.deviceName}>
                          {credential.deviceName || `デバイス ${credential.credentialId.slice(0, 8)}...`}
                        </span>
                        <span className={styles.deviceMeta}>
                          登録日: {formatDate(credential.createdAt)}
                          {isCurrent && <span className={styles.currentBadge}>使用中</span>}
                        </span>
                      </div>
                    </div>
                    <div className={styles.credentialActions}>
                      {!isCurrent && (
                        <button
                          className={styles.switchButton}
                          onClick={() => handleSwitchCredential(credential)}
                          title="このデバイスに切り替え"
                        >
                          切替
                        </button>
                      )}
                      <button
                        className={styles.removeButton}
                        onClick={() => handleRemoveCredential(credential)}
                        title="このデバイスから削除"
                      >
                        ✕
                      </button>
                    </div>
                  </li>
                )
              })}
            </ul>
          </>
        )}
      </div>

      {/* Add Device Section */}
      {canAddPasskey && (
        <div className={styles.addSection}>
          {!showAddForm ? (
            <button
              className={styles.addButton}
              onClick={() => setShowAddForm(true)}
              disabled={registrationStatus !== 'idle'}
            >
              <span className={styles.addIcon}>+</span>
              デバイスを追加
            </button>
          ) : (
            <div className={styles.addForm}>
              <input
                type="text"
                value={deviceName}
                onChange={(e) => setDeviceName(e.target.value)}
                placeholder="デバイス名 (例: iPhone, MacBook)"
                className={styles.deviceInput}
                disabled={isAddingDevice}
                maxLength={64}
              />
              <div className={styles.addFormActions}>
                <button
                  className={styles.confirmButton}
                  onClick={handleAddPasskey}
                  disabled={isAddingDevice || !deviceName.trim()}
                >
                  {isAddingDevice ? '追加中...' : '追加'}
                </button>
                <button
                  className={styles.cancelButton}
                  onClick={() => {
                    setShowAddForm(false)
                    setDeviceName('')
                  }}
                  disabled={isAddingDevice}
                >
                  キャンセル
                </button>
              </div>
            </div>
          )}

          {/* Status Messages */}
          {registrationStatus !== 'idle' && registrationStatus !== 'success' && (
            <div className={styles.statusMessage}>
              {registrationStatus === 'authenticating' && (
                <span>🔐 パスキーを確認しています...</span>
              )}
              {registrationStatus === 'extracting' && (
                <span>📦 公開鍵を取得しています...</span>
              )}
              {registrationStatus === 'submitting' && (
                <span>📤 トランザクションを送信中...</span>
              )}
              {registrationStatus === 'confirming' && (
                <span>⏳ 確認を待っています...</span>
              )}
              {registrationStatus === 'error' && error && (
                <span className={styles.errorMessage}>❌ {error.message}</span>
              )}
            </div>
          )}
        </div>
      )}

      {/* Help Text */}
      {!canAddPasskey && (
        <div className={styles.helpText}>
          <p>デバイスを追加するには、まずパスキーで登録してください。</p>
        </div>
      )}

      {/* Info Note */}
      <div className={styles.infoNote}>
        <p>
          💡 ヒント: 複数のデバイスを登録すると、どのデバイスからでも投稿できます。
        </p>
      </div>
    </div>
  )
}

export default DeviceSettings
