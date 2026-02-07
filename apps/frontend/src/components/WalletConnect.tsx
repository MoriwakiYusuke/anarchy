'use client'

import { useState } from 'react'
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

type AuthMode = 'dev' | 'seedphrase'

export function WalletConnect({ account, setAccount, setAccountSeed, unsafeApi, signer, accountSeed, refreshTrigger }: Props) {
  const [selectedAccount, setSelectedAccount] = useState<string>('')
  const [authMode, setAuthMode] = useState<AuthMode>('dev')
  const [seedPhraseInput, setSeedPhraseInput] = useState<string>('')
  const [seedPhraseError, setSeedPhraseError] = useState<string | null>(null)
  const [generatedPhrase, setGeneratedPhrase] = useState<string | null>(null)
  const [showCopied, setShowCopied] = useState(false)
  const [isMinting, setIsMinting] = useState(false)
  const [mintStatus, setMintStatus] = useState<string | null>(null)
  const { balance, isLoading: balanceLoading, refetch: refetchBalance } = useMoralBalance(unsafeApi, account, refreshTrigger)

  const isAlice = accountSeed === ALICE_SEED

  // 開発モード: テストアカウントで接続
  const handleConnectDev = async () => {
    if (!selectedAccount) return

    const { cryptoWaitReady } = await import('@polkadot/util-crypto')
    await cryptoWaitReady()
    
    const { Keyring } = await import('@polkadot/keyring')
    const keyring = new Keyring({ type: 'sr25519' })
    const pair = keyring.addFromUri(selectedAccount)
    setAccount(pair.address)
    setAccountSeed(selectedAccount)
  }

  // シードフレーズモード: 入力したシードフレーズで接続
  const handleConnectSeedPhrase = async () => {
    if (!seedPhraseInput.trim()) {
      setSeedPhraseError('シードフレーズを入力してください')
      return
    }

    const { cryptoWaitReady, mnemonicValidate } = await import('@polkadot/util-crypto')
    await cryptoWaitReady()

    const trimmed = seedPhraseInput.trim()
    if (!mnemonicValidate(trimmed)) {
      setSeedPhraseError('無効なシードフレーズです。12または24単語の正しいニーモニックを入力してください。')
      return
    }

    const { Keyring } = await import('@polkadot/keyring')
    const keyring = new Keyring({ type: 'sr25519' })
    const pair = keyring.addFromUri(trimmed)
    setAccount(pair.address)
    setAccountSeed(trimmed)
    setSeedPhraseError(null)
    // セキュリティ: 接続後に入力欄をクリア
    setSeedPhraseInput('')
  }

  // 新しいシードフレーズを生成
  const handleGenerateSeedPhrase = async () => {
    const { cryptoWaitReady, mnemonicGenerate } = await import('@polkadot/util-crypto')
    await cryptoWaitReady()
    
    const newMnemonic = mnemonicGenerate(12)
    setGeneratedPhrase(newMnemonic)
    setSeedPhraseInput(newMnemonic)
    setSeedPhraseError(null)
  }

  // シードフレーズをクリップボードにコピー
  const handleCopySeedPhrase = async () => {
    if (!generatedPhrase) return
    await navigator.clipboard.writeText(generatedPhrase)
    setShowCopied(true)
    setTimeout(() => setShowCopied(false), 2000)
  }

  const handleDisconnect = () => {
    setAccount(null)
    setAccountSeed(null)
    setSelectedAccount('')
    setSeedPhraseInput('')
    setGeneratedPhrase(null)
    setSeedPhraseError(null)
  }

  // 開発用: DevMintは廃止（$moral = ネイティブトークンになった）
  // 代わりに node scripts/mint-moral.mjs <address> <amount> を使用
  const handleDevMint = async () => {
    if (!unsafeApi || !account || !signer || !isAlice) return
    
    setIsMinting(true)
    setMintStatus('[$moral = ネイティブトークン] mint-moral.mjs スクリプトを使用してください')
    setTimeout(() => {
      setMintStatus(null)
      setIsMinting(false)
    }, 3000)
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
          {/* モード切替タブ */}
          <div className={styles.modeTabs}>
            <button
              className={`${styles.modeTab} ${authMode === 'seedphrase' ? styles.active : ''}`}
              onClick={() => setAuthMode('seedphrase')}
            >
              シードフレーズ
            </button>
            <button
              className={`${styles.modeTab} ${authMode === 'dev' ? styles.active : ''}`}
              onClick={() => setAuthMode('dev')}
            >
              開発用
            </button>
          </div>

          {authMode === 'seedphrase' ? (
            <div className={styles.seedPhraseSection}>
              <p className={styles.hint}>
                シードフレーズ（12または24単語）を入力するか、新規生成してください
              </p>
              
              <textarea
                className={styles.seedPhraseInput}
                value={seedPhraseInput}
                onChange={(e) => {
                  setSeedPhraseInput(e.target.value)
                  setSeedPhraseError(null)
                }}
                placeholder="word1 word2 word3 ... word12"
                rows={3}
              />
              
              {seedPhraseError && (
                <div className={styles.seedPhraseError}>
                  {seedPhraseError}
                </div>
              )}
              
              {generatedPhrase && (
                <div className={styles.generatedPhrase}>
                  <p className={styles.warning}>
                    ⚠️ このシードフレーズを安全な場所に保存してください。紛失すると資産にアクセスできなくなります。
                  </p>
                  <button 
                    className={styles.copyBtn}
                    onClick={handleCopySeedPhrase}
                  >
                    {showCopied ? '✅ コピーしました' : '📋 コピー'}
                  </button>
                </div>
              )}
              
              <div className={styles.seedPhraseButtons}>
                <button 
                  className={styles.generateBtn}
                  onClick={handleGenerateSeedPhrase}
                >
                  新規生成
                </button>
                <button 
                  className={styles.connectBtn}
                  onClick={handleConnectSeedPhrase}
                  disabled={!seedPhraseInput.trim()}
                >
                  接続
                </button>
              </div>
            </div>
          ) : (
            <div className={styles.devSection}>
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
                onClick={handleConnectDev}
                disabled={!selectedAccount}
              >
                接続
              </button>
            </div>
          )}
        </div>
      )}

      <div className={styles.info}>
        <p className={styles.note}>
          ※ シードフレーズはブラウザのメモリ内のみに保持され、ページを閉じると消去されます
        </p>
      </div>
    </div>
  )
}
