'use client'

import { useState } from 'react'
import { Keyring } from '@polkadot/keyring'
import styles from './WalletConnect.module.css'

interface Props {
  account: string | null
  setAccount: (account: string | null) => void
}

// 開発用: テストアカウント
const TEST_ACCOUNTS = [
  { name: 'Alice', seed: '//Alice' },
  { name: 'Bob', seed: '//Bob' },
  { name: 'Charlie', seed: '//Charlie' },
]

export function WalletConnect({ account, setAccount }: Props) {
  const [selectedAccount, setSelectedAccount] = useState<string>('')

  const handleConnect = () => {
    if (!selectedAccount) return

    const keyring = new Keyring({ type: 'sr25519' })
    const pair = keyring.addFromUri(selectedAccount)
    setAccount(pair.address)
  }

  const handleDisconnect = () => {
    setAccount(null)
    setSelectedAccount('')
  }

  const shortenAddress = (addr: string) => {
    return `${addr.slice(0, 6)}...${addr.slice(-4)}`
  }

  return (
    <div className={styles.container}>
      <h3 className={styles.title}>ウォレット</h3>

      {account ? (
        <div className={styles.connected}>
          <div className={styles.address}>
            <span className={styles.label}>接続中</span>
            <code>{shortenAddress(account)}</code>
          </div>
          <button 
            className={styles.disconnectBtn}
            onClick={handleDisconnect}
          >
            切断
          </button>
        </div>
      ) : (
        <div className={styles.connect}>
          <p className={styles.hint}>
            開発用テストアカウント
          </p>
          <select 
            className={styles.select}
            value={selectedAccount}
            onChange={(e) => setSelectedAccount(e.target.value)}
          >
            <option value="">選択してください</option>
            {TEST_ACCOUNTS.map((acc) => (
              <option key={acc.seed} value={acc.seed}>
                {acc.name}
              </option>
            ))}
          </select>
          <button 
            className={styles.connectBtn}
            onClick={handleConnect}
            disabled={!selectedAccount}
          >
            接続
          </button>
        </div>
      )}

      <div className={styles.info}>
        <p className={styles.note}>
          ※ 本番環境ではWebAuthn（パスキー）による認証に置き換わります
        </p>
      </div>
    </div>
  )
}
