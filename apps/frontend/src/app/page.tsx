'use client'

import { useState, useEffect, useCallback } from 'react'
import { useApi } from '@/hooks/useApi'
import { useLocale } from '@/i18n'
import { PostForm } from '@/components/PostForm'
import { Timeline } from '@/components/Timeline'
import { WalletConnect } from '@/components/WalletConnect'
import { LanguageSwitcher } from '@/components/LanguageSwitcher'
import styles from './page.module.css'
import type { PolkadotSigner } from 'polkadot-api/signer'

export default function Home() {
  const { client, unsafeApi, connectionState, error, createSigner } = useApi()
  const { t } = useLocale()
  const [account, setAccount] = useState<string | null>(null)
  const [accountSeed, setAccountSeed] = useState<string | null>(null)
  const [refreshTrigger, setRefreshTrigger] = useState(0)
  const [signer, setSigner] = useState<PolkadotSigner | null>(null)

  // Create signer when accountSeed changes (async)
  useEffect(() => {
    if (!accountSeed) {
      setSigner(null)
      return
    }
    let cancelled = false
    createSigner(accountSeed).then((newSigner) => {
      if (!cancelled) {
        setSigner(newSigner)
      }
    })
    return () => {
      cancelled = true
    }
  }, [accountSeed, createSigner])

  // 投稿成功時にデータを更新
  const handlePostSuccess = useCallback(() => {
    setRefreshTrigger(prev => prev + 1)
  }, [])

  // Connection status display helper
  const renderConnectionStatus = () => {
    switch (connectionState.status) {
      case 'connected':
        return <span className={styles.connected}>● {t('app.connected')}</span>
      case 'syncing':
        return <span className={styles.syncing}>◐ {t('app.syncing')}</span>
      case 'initializing':
        return <span className={styles.disconnected}>○ {t('app.connecting')}</span>
      case 'error':
        return <span className={styles.disconnected}>○ {t('app.disconnected')}</span>
      default:
        return <span className={styles.disconnected}>○ {t('app.disconnected')}</span>
    }
  }

  return (
    <main className={styles.main}>
      <div className={styles.headerBg}>
        <header className={styles.header}>
          <div className={styles.headerTop}>
            <LanguageSwitcher variant="compact" />
          </div>
          <h1 className={styles.title}>
            <span className={styles.accent}>A</span>narchy
          </h1>
          <p className={styles.subtitle}>{t('app.subtitle')}</p>
          <div className={styles.status}>
            {renderConnectionStatus()}
          </div>
        </header>
      </div>

      <div className={styles.container}>
        <aside className={styles.sidebar}>
          <WalletConnect 
            account={account} 
            setAccount={setAccount}
            setAccountSeed={setAccountSeed}
            client={client}
            unsafeApi={unsafeApi}
            signer={signer}
            accountSeed={accountSeed}
            refreshTrigger={refreshTrigger}
          />
        </aside>

        <section className={styles.content}>
          {account && signer && (
            <PostForm 
              unsafeApi={unsafeApi} 
              signer={signer}
              derivePath={accountSeed || '//Alice'}
              onPostSuccess={handlePostSuccess}
            />
          )}
          <Timeline client={client} unsafeApi={unsafeApi} refreshTrigger={refreshTrigger} />
        </section>
      </div>

      {error && (
        <div className={styles.error}>
          エラー: {error}
        </div>
      )}
    </main>
  )
}
