'use client'

import { useState, useMemo, useCallback } from 'react'
import { useApi } from '@/hooks/useApi'
import { PostForm } from '@/components/PostForm'
import { Timeline } from '@/components/Timeline'
import { WalletConnect } from '@/components/WalletConnect'
import styles from './page.module.css'

export default function Home() {
  const { client, unsafeApi, isConnected, error, createSigner } = useApi()
  const [account, setAccount] = useState<string | null>(null)
  const [accountSeed, setAccountSeed] = useState<string | null>(null)
  const [refreshTrigger, setRefreshTrigger] = useState(0)

  // Create signer when accountSeed changes
  const signer = useMemo(() => {
    if (!accountSeed) return null
    return createSigner(accountSeed)
  }, [accountSeed, createSigner])

  // 投稿成功時にデータを更新
  const handlePostSuccess = useCallback(() => {
    setRefreshTrigger(prev => prev + 1)
  }, [])

  return (
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
        <aside className={styles.sidebar}>
          <WalletConnect 
            account={account} 
            setAccount={setAccount}
            setAccountSeed={setAccountSeed}
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
