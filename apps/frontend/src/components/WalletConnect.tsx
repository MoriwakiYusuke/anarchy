'use client'

import { useState } from 'react'
import { Keyring } from '@polkadot/keyring'
import { PolkadotSigner } from 'polkadot-api/signer'
import { useMoralBalance, formatMoralBalance } from '@/hooks/useMoralBalance'
import styles from './WalletConnect.module.css'

interface Props {
  account: string | null
  setAccount: (account: string | null) => void
  setAccountSeed: (seed: string | null) => void
  unsafeApi: any
  signer: PolkadotSigner | null
  accountSeed: string | null
  refreshTrigger?: number
}

// 開発用: テストアカウント
const TEST_ACCOUNTS = [
  { name: 'Alice', seed: '//Alice' },
  { name: 'Bob', seed: '//Bob' },
  { name: 'Charlie', seed: '//Charlie' },
]

// AliceのアドレスプレフィックスまたはSeed
const ALICE_SEED = '//Alice'

export function WalletConnect({ account, setAccount, setAccountSeed, unsafeApi, signer, accountSeed, refreshTrigger }: Props) {
  const [selectedAccount, setSelectedAccount] = useState<string>('')
  const [isMinting, setIsMinting] = useState(false)
  const [mintStatus, setMintStatus] = useState<string | null>(null)
  const { balance, isLoading: balanceLoading, refetch: refetchBalance } = useMoralBalance(unsafeApi, account, refreshTrigger)

  const isAlice = accountSeed === ALICE_SEED

  const handleConnect = async () => {
    if (!selectedAccount) return

    // WASM暗号モジュールの初期化を待つ
    const { cryptoWaitReady } = await import('@polkadot/util-crypto')
    await cryptoWaitReady()
    
    const keyring = new Keyring({ type: 'sr25519' })
    const pair = keyring.addFromUri(selectedAccount)
    setAccount(pair.address)
    setAccountSeed(selectedAccount)
  }

  const handleDisconnect = () => {
    setAccount(null)
    setAccountSeed(null)
    setSelectedAccount('')
  }

  // 開発用: Sudo mintでトークンを自分にmint
  const handleDevMint = async () => {
    if (!unsafeApi || !account || !signer || !isAlice) return
    
    setIsMinting(true)
    setMintStatus('mintトランザクション送信中...')
    
    try {
      const amount = BigInt(10000) * BigInt(1_000_000_000_000) // 10,000 moral
      
      // Sudo権限でMoral.mintを呼び出し
      const mintCall = unsafeApi.tx.Moral.mint({
        to: account,
        amount: amount,
      })

      const sudoTx = unsafeApi.tx.Sudo.sudo({
        call: mintCall.decodedCall,
      })

      const result = await sudoTx.signAndSubmit(signer)

      if (result.ok) {
        setMintStatus(`✅ 10,000 moral をmintしました！`)
        refetchBalance()
        setTimeout(() => setMintStatus(null), 3000)
      } else {
        setMintStatus(`❌ Mint失敗: ${JSON.stringify(result.dispatchError)}`)
      }
    } catch (err) {
      console.error('Mint failed:', err)
      setMintStatus(`❌ エラー: ${err instanceof Error ? err.message : '不明'}`)
    } finally {
      setIsMinting(false)
    }
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
            <span className={styles.label}>接続中 {isAlice && <span className={styles.adminBadge}>Admin</span>}</span>
            <code>{shortenAddress(account)}</code>
          </div>
          
          <div className={styles.balance}>
            <span className={styles.balanceLabel}>$moral残高</span>
            <span className={styles.balanceValue}>
              {balanceLoading ? (
                <span className={styles.loading}>読込中...</span>
              ) : (
                <>
                  {formatMoralBalance(balance)}
                  <button 
                    className={styles.refreshBtn}
                    onClick={refetchBalance}
                    title="残高を更新"
                  >
                    ↻
                  </button>
                </>
              )}
            </span>
          </div>

          {/* 開発用: AliceのみSudo mint可能 */}
          {isAlice && (
            <button 
              className={styles.devMintBtn}
              onClick={handleDevMint}
              disabled={isMinting}
            >
              {isMinting ? 'Mint中...' : '🔧 10,000 moral をmint (Dev)'}
            </button>
          )}
          
          {mintStatus && (
            <div className={styles.mintStatus}>
              {mintStatus}
            </div>
          )}
          
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
