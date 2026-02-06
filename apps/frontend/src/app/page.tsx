'use client'

import { useState, useMemo } from 'react'
import { useApi } from '@/hooks/useApi'
import { PostForm } from '@/components/PostForm'
import { Timeline } from '@/components/Timeline'
import { WalletConnect } from '@/components/WalletConnect'
import styles from './page.module.css'

export default function Home() {
  const { client, unsafeApi, isConnected, error, createSigner } = useApi()
  const [account, setAccount] = useState<string | null>(null)
  const [accountSeed, setAccountSeed] = useState<string | null>(null)

  // Create signer when accountSeed changes
  const signer = useMemo(() => {
    if (!accountSeed) return null
    return createSigner(accountSeed)
  }, [accountSeed, createSigner])

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
          />
        </aside>

        <section className={styles.content}>
          {account && signer && (
            <PostForm 
              unsafeApi={unsafeApi} 
              signer={signer}
              derivePath={accountSeed || '//Alice'}
            />
          )}
          <Timeline client={client} unsafeApi={unsafeApi} />
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
