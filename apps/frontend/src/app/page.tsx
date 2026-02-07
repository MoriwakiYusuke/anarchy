'use client'

import { useState, useMemo, useCallback } from 'react'
import { useApi } from '@/hooks/useApi'
import { Timeline } from '@/components/Timeline'
import { WebAuthnGate } from '@/components/WebAuthnGate'
import { PasskeyRegister } from '@/components/PasskeyRegister'
import { PasskeySignPost } from '@/components/PasskeySignPost'
import { DeviceSettings } from '@/components/DeviceSettings'
import { useWebAuthnContext, WebAuthnProvider } from '@/contexts/WebAuthnContext'
import styles from './page.module.css'

/**
 * 投稿セクション
 * - パスキーがある場合: 投稿フォームを表示
 * - パスキーがない場合: パスキー追加ボタンを表示
 */
function PostSection({ 
  api,
  signer,
  onPostSuccess 
}: { 
  api: any | null
  signer: any | null
  onPostSuccess: () => void 
}) {
  const { identity, persistedCredentials, loginWithPasskey } = useWebAuthnContext()
  const [showRegister, setShowRegister] = useState(false)
  const [isLoggingIn, setIsLoggingIn] = useState(false)
  const [loginError, setLoginError] = useState<string | null>(null)

  // パスキーがある（LocalStorageに保存されている）場合
  const hasStoredCredentials = persistedCredentials.length > 0

  // パスキーでログイン（既存credentialがある場合）
  const handleLogin = async () => {
    try {
      setIsLoggingIn(true)
      setLoginError(null)
      const result = await loginWithPasskey()
      if (!result.success) {
        setLoginError(result.error?.message || 'ログインに失敗しました')
      }
    } catch (err) {
      console.error('Login failed:', err)
      setLoginError(err instanceof Error ? err.message : 'ログインに失敗しました')
    } finally {
      setIsLoggingIn(false)
    }
  }

  return (
    <WebAuthnGate>
      {identity ? (
        // パスキーがある: 投稿フォームを表示
        <div className={styles.passkeyContent}>
          <PasskeySignPost
            api={api}
            signer={signer}
            identityId={identity.identityId}
            passkeyId={identity.passkeyId}
            credentialId={identity.credentialId}
            onSuccess={onPostSuccess}
          />
          <DeviceSettings 
            title="登録済みデバイス"
            className={styles.deviceSettings}
          />
        </div>
      ) : showRegister ? (
        // パスキー登録画面
        <div className={styles.authSection}>
          <PasskeyRegister api={api} signer={signer} />
          <button
            className={styles.linkButton}
            onClick={() => setShowRegister(false)}
          >
            キャンセル
          </button>
        </div>
      ) : hasStoredCredentials ? (
        // 保存済みcredentialがある: ログインを促す
        <div className={styles.noPasskeySection}>
          <div className={styles.noPasskeyIcon}>🔑</div>
          <h3>パスキーでログイン</h3>
          <p className={styles.noPasskeyDescription}>
            登録済みのパスキーで認証してください。
          </p>
          {loginError && (
            <p className={styles.errorMessage}>{loginError}</p>
          )}
          <button
            className={styles.primaryButton}
            onClick={handleLogin}
            disabled={isLoggingIn}
          >
            {isLoggingIn ? '認証中...' : 'パスキーでログイン'}
          </button>
          <button
            className={styles.linkButton}
            onClick={() => setShowRegister(true)}
          >
            別のパスキーを追加
          </button>
        </div>
      ) : (
        // パスキーがない: 追加を促す
        <div className={styles.noPasskeySection}>
          <div className={styles.noPasskeyIcon}>🔑</div>
          <h3>投稿するにはパスキーが必要です</h3>
          <p className={styles.noPasskeyDescription}>
            パスキーを登録すると、生体認証で安全に投稿できます。
          </p>
          <button
            className={styles.primaryButton}
            onClick={() => setShowRegister(true)}
          >
            パスキーを追加
          </button>
        </div>
      )}
    </WebAuthnGate>
  )
}

export default function Home() {
  const { client, unsafeApi, isConnected, error, createSigner } = useApi()
  const [refreshTrigger, setRefreshTrigger] = useState(0)

  // パスキー用のsigner（fee payerとして使用）
  const passkeySigner = useMemo(() => {
    return createSigner('//Alice')
  }, [createSigner])

  // 投稿成功時にデータを更新
  const handlePostSuccess = useCallback(() => {
    setRefreshTrigger(prev => prev + 1)
  }, [])

  return (
    <WebAuthnProvider api={unsafeApi} signer={passkeySigner}>
      <main className={styles.main}>
        <header className={styles.header}>
          <h1 className={styles.title}>
            <span className={styles.accent}>A</span>narchy
          </h1>
          <p className={styles.subtitle}>支配なき秩序</p>
          <div className={styles.status}>
            {isConnected ? (
              <span className={styles.connected}>● 接続済み</span>
            ) : (
              <span className={styles.disconnected}>○ 未接続</span>
            )}
          </div>
        </header>

        <div className={styles.container}>
          <section className={styles.content}>
            <PostSection 
              api={unsafeApi} 
              signer={passkeySigner} 
              onPostSuccess={handlePostSuccess} 
            />
            <Timeline client={client} unsafeApi={unsafeApi} refreshTrigger={refreshTrigger} />
          </section>
        </div>

        {error && (
          <div className={styles.error}>
            エラー: {error}
          </div>
        )}
      </main>
    </WebAuthnProvider>
  )
}
