'use client'

import { useState, useCallback, useRef } from 'react'
import { useApi } from '@/hooks/useApi'
import { useAccount } from '@/lib/account/context'
import { useLocale } from '@/i18n'
import { PostForm } from '@/components/PostForm'
import { Timeline } from '@/components/Timeline'
import { WalletConnect } from '@/components/WalletConnect'
import { TransferForm } from '@/components/TransferForm'
import { LanguageSwitcher } from '@/components/LanguageSwitcher'
import NicknameSettings from '@/components/NicknameSettings'
import { DmModal } from '@/components/dm/DmModal'
import { ConnectedDot, SyncingDot, DisconnectedDot } from '@/components/Icons'
import { useMoralBalance } from '@/hooks/useMoralBalance'
import styles from './page.module.css'

export default function Home() {
  const { client, unsafeApi, connectionState, error } = useApi()
  const { t } = useLocale()
  const { account, signer, mainRawSigner } = useAccount()
  const [refreshTrigger, setRefreshTrigger] = useState(0)
  const [isTransferOpen, setIsTransferOpen] = useState(false)
  const [isDmOpen, setIsDmOpen] = useState(false)
  const refetchBalanceRef = useRef<(() => void) | null>(null)
  const { balance } = useMoralBalance(unsafeApi, account, refreshTrigger)

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
          <h1 className={styles.title} aria-label="Anarchy">
            <svg
              className={styles.logo}
              xmlns="http://www.w3.org/2000/svg"
              viewBox="20 20 200 200"
              role="img"
              aria-label="A"
            >
              <defs>
                <linearGradient id="anarchyRing" x1="0" y1="0" x2="1" y2="1">
                  <stop offset="0%" stopColor="#cc0000" />
                  <stop offset="100%" stopColor="#660000" />
                </linearGradient>
              </defs>
              <circle cx="120" cy="120" r="92" stroke="url(#anarchyRing)" strokeWidth="6" fill="none" />
              <g stroke="#cc0000" strokeWidth="14" strokeLinecap="square" fill="none">
                <path d="M70 168 L120 60 L170 168" />
                <path d="M88 130 L152 130" />
              </g>
              <path d="M55 178 L185 178" stroke="url(#anarchyRing)" strokeWidth="6" />
            </svg>
            <span className={styles.titleText}>narchy</span>
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
            client={client}
            unsafeApi={unsafeApi}
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
              <button
                type="button"
                className={styles.collapsibleHeader}
                onClick={() => setIsDmOpen(true)}
              >
                <span>{t('nav.dm')}</span>
                <span className={styles.collapseIcon}>→</span>
              </button>
            </div>
          )}
        </aside>

        <DmModal isOpen={isDmOpen} onClose={() => setIsDmOpen(false)} />

        <section className={styles.content}>
          {account && signer && (
            <div className={styles.postFormWrapper}>
              <PostForm
                unsafeApi={unsafeApi}
                signer={signer}
                storageSigner={mainRawSigner}
                onPostSuccess={handlePostSuccess}
              />
            </div>
          )}
          <div className={styles.timelineWrapper}>
            <Timeline
              client={client}
              unsafeApi={unsafeApi}
              account={account}
              signer={signer}
              storageSigner={mainRawSigner}
              refreshTrigger={refreshTrigger}
              onReplyPosted={handlePostSuccess}
            />
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
