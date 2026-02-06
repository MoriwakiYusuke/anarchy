'use client'

import { useState, useEffect } from 'react'
import { useApi } from '@/hooks/useApi'
import { PostForm } from '@/components/PostForm'
import { Timeline } from '@/components/Timeline'
import { WalletConnect } from '@/components/WalletConnect'
import styles from './page.module.css'

export default function Home() {
  const { api, isConnected, error } = useApi()
  const [account, setAccount] = useState<string | null>(null)

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
          />
        </aside>

        <section className={styles.content}>
          {account && <PostForm api={api} account={account} />}
          <Timeline api={api} />
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
