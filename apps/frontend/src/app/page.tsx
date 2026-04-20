'use client'

import { useState, useEffect, useCallback, useRef } from 'react'
import Link from 'next/link'
import { useApi } from '@/hooks/useApi'
import { useLocale } from '@/i18n'
import { PostForm } from '@/components/PostForm'
import { Timeline } from '@/components/Timeline'
import { WalletConnect } from '@/components/WalletConnect'
import { TransferForm } from '@/components/TransferForm'
import { LanguageSwitcher } from '@/components/LanguageSwitcher'
import NicknameSettings from '@/components/NicknameSettings'
import { ConnectedDot, SyncingDot, DisconnectedDot } from '@/components/Icons'
import { useMoralBalance } from '@/hooks/useMoralBalance'
import styles from './page.module.css'
import type { PolkadotSigner } from 'polkadot-api/signer'

export default function Home() {
  const { client, unsafeApi, connectionState, error, createSigner } = useApi()
  const { t } = useLocale()
  const [account, setAccount] = useState<string | null>(null)
  const [accountSeed, setAccountSeed] = useState<string | null>(null)
  const [refreshTrigger, setRefreshTrigger] = useState(0)
  const [signer, setSigner] = useState<PolkadotSigner | null>(null)
  const [isTransferOpen, setIsTransferOpen] = useState(false)
  const refetchBalanceRef = useRef<(() => void) | null>(null)
  const { balance } = useMoralBalance(unsafeApi, account, refreshTrigger)

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
        return <span className={styles.connected}><ConnectedDot size={8} /> {t('app.connected')}</span>
      case 'syncing':
        return <span className={styles.syncing}><SyncingDot size={8} /> {t('app.syncing')}</span>
      case 'initializing':
        return <span className={styles.disconnected}><DisconnectedDot size={8} /> {t('app.connecting')}</span>
      case 'error':
        return <span className={styles.disconnected}><DisconnectedDot size={8} /> {t('app.disconnected')}</span>
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
          {account && signer && client && unsafeApi && (
            <NicknameSettings
              client={client}
              unsafeApi={unsafeApi}
              accountId={account}
              signer={signer}
            />
          )}
          <WalletConnect 
            account={account} 
            setAccount={setAccount}
            setAccountSeed={setAccountSeed}
            client={client}
            unsafeApi={unsafeApi}
            signer={signer}
            accountSeed={accountSeed}
            refreshTrigger={refreshTrigger}
            onBalanceChange={(refetch) => { refetchBalanceRef.current = refetch }}
          />
          {account && signer && (
            <div className={styles.collapsibleSection}>
              <button
                className={styles.collapsibleHeader}
                onClick={() => setIsTransferOpen(!isTransferOpen)}
                aria-expanded={isTransferOpen}
              >
                <span>{t('transfer.title')}</span>
                <span className={styles.collapseIcon}>{isTransferOpen ? '▲' : '▼'}</span>
              </button>
              {isTransferOpen && (
                <div className={styles.collapsibleContent}>
                  <TransferForm
                    client={client}
                    unsafeApi={unsafeApi}
                    senderAddress={account}
                    balance={balance ?? BigInt(0)}
                    signer={signer}
                    blockNumber={connectionState.blockNumber}
                    onSuccess={() => refetchBalanceRef.current?.()}
                  />
                </div>
              )}
            </div>
          )}
          {account && signer && (
            <div className={styles.collapsibleSection}>
              <Link href="/dm" className={styles.collapsibleHeader}>
                <span>{t('nav.dm')}</span>
                <span className={styles.collapseIcon}>→</span>
              </Link>
            </div>
          )}
        </aside>

        <section className={styles.content}>
          {account && signer && (
            <div className={styles.postFormWrapper}>
              <PostForm 
                unsafeApi={unsafeApi} 
                signer={signer}
                derivePath={accountSeed || '//Alice'}
                onPostSuccess={handlePostSuccess}
              />
            </div>
          )}
          <div className={styles.timelineWrapper}>
            <Timeline client={client} unsafeApi={unsafeApi} account={account} signer={signer} refreshTrigger={refreshTrigger} />
          </div>
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
