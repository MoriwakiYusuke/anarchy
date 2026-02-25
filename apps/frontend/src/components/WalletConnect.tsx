'use client'

import { useState } from 'react'
import { PolkadotSigner } from 'polkadot-api/signer'
import { useMoralBalance, formatMoralBalance } from '@/hooks/useMoralBalance'
import { useLocale } from '@/i18n'
import { FaucetButton } from './FaucetButton'
import { ConnectedDot, CopyIcon, CheckIcon } from './Icons'
import styles from './WalletConnect.module.css'

interface Props {
  account: string | null
  setAccount: (account: string | null) => void
  setAccountSeed: (seed: string | null) => void
  client: any
  unsafeApi: any
  signer: PolkadotSigner | null
  accountSeed: string | null
  refreshTrigger?: number
  onBalanceChange?: (refetch: () => void) => void
}

// 開発用: テストアカウント
const TEST_ACCOUNTS = [
  { name: 'Alice', seed: '//Alice' },
  { name: 'Bob', seed: '//Bob' },
  { name: 'Charlie', seed: '//Charlie' },
]

type AuthMode = 'dev' | 'seedphrase'

export function WalletConnect({ account, setAccount, setAccountSeed, client, unsafeApi, signer, accountSeed, refreshTrigger, onBalanceChange }: Props) {
  const { t } = useLocale()
  const [selectedAccount, setSelectedAccount] = useState<string>('')
  const [authMode, setAuthMode] = useState<AuthMode>('dev')
  const [seedPhraseInput, setSeedPhraseInput] = useState<string>('')
  const [seedPhraseError, setSeedPhraseError] = useState<string | null>(null)
  const [generatedPhrase, setGeneratedPhrase] = useState<string | null>(null)
  const [showCopied, setShowCopied] = useState(false)
  const [showAddressCopied, setShowAddressCopied] = useState(false)
  const { balance, isLoading: balanceLoading, refetch: refetchBalance } = useMoralBalance(unsafeApi, account, refreshTrigger)

  // Expose refetchBalance to parent
  if (onBalanceChange) {
    onBalanceChange(refetchBalance)
  }

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
      setSeedPhraseError(t('wallet.seedErrorEmpty'))
      return
    }

    const { cryptoWaitReady, mnemonicValidate } = await import('@polkadot/util-crypto')
    await cryptoWaitReady()

    const trimmed = seedPhraseInput.trim()
    if (!mnemonicValidate(trimmed)) {
      setSeedPhraseError(t('wallet.seedErrorInvalid'))
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

  // アドレスをクリップボードにコピー
  const handleCopyAddress = async () => {
    if (!account) return
    await navigator.clipboard.writeText(account)
    setShowAddressCopied(true)
    setTimeout(() => setShowAddressCopied(false), 2000)
  }

  const handleDisconnect = () => {
    setAccount(null)
    setAccountSeed(null)
    setSelectedAccount('')
    setSeedPhraseInput('')
    setGeneratedPhrase(null)
    setSeedPhraseError(null)
  }

  const shortenAddress = (addr: string) => {
    return `${addr.slice(0, 6)}...${addr.slice(-4)}`
  }

  return (
    <div className={styles.container}>
      <h3 className={styles.title}>{t('wallet.title')}</h3>

      {account ? (
        <div className={styles.connected}>
          <div className={styles.address}>
            <span className={styles.label}><ConnectedDot size={8} /> {t('wallet.connected')}</span>
            <div className={styles.addressRow}>
              <code className={styles.fullAddress}>{account}</code>
              <button
                className={styles.copyBtn}
                onClick={handleCopyAddress}
                title={showAddressCopied ? t('wallet.copied') : t('wallet.copy')}
              >
                {showAddressCopied ? (
                  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                    <polyline points="20 6 9 17 4 12" />
                  </svg>
                ) : (
                  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                    <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
                    <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
                  </svg>
                )}
              </button>
            </div>
          </div>
          
          <div className={styles.balance}>
            <span className={styles.balanceLabel}>{t('balance.label')}</span>
            <span className={styles.balanceValue}>
              {formatMoralBalance(balance)}
              <button 
                className={styles.refreshBtn}
                onClick={refetchBalance}
                title={t('wallet.refreshBalance')}
                disabled={balanceLoading}
                style={{ opacity: balanceLoading ? 0.5 : 1 }}
              >
                ↻
              </button>
            </span>
          </div>
          
          <FaucetButton
            client={client}
            unsafeApi={unsafeApi}
            account={account}
            signer={signer}
            onSuccess={refetchBalance}
          />
          
          <button 
            className={styles.disconnectBtn}
            onClick={handleDisconnect}
          >
            {t('wallet.disconnect')}
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
              {t('wallet.seedPhrase')}
            </button>
            <button
              className={`${styles.modeTab} ${authMode === 'dev' ? styles.active : ''}`}
              onClick={() => setAuthMode('dev')}
            >
              {t('wallet.dev')}
            </button>
          </div>

          {authMode === 'seedphrase' ? (
            <div className={styles.seedPhraseSection}>
              <p className={styles.hint}>
                {t('wallet.seedHint')}
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
                    {t('wallet.seedWarning')}
                  </p>
                  <button 
                    className={styles.copyBtn}
                    onClick={handleCopySeedPhrase}
                  >
                    {showCopied ? <><CheckIcon size={14} />{' '}{t('wallet.copied')}</> : <><CopyIcon size={14} />{' '}{t('wallet.copy')}</>}
                  </button>
                </div>
              )}
              
              <div className={styles.seedPhraseButtons}>
                <button 
                  className={styles.generateBtn}
                  onClick={handleGenerateSeedPhrase}
                >
                  {t('wallet.generate')}
                </button>
                <button 
                  className={styles.connectBtn}
                  onClick={handleConnectSeedPhrase}
                  disabled={!seedPhraseInput.trim()}
                >
                  {t('wallet.connect')}
                </button>
              </div>
            </div>
          ) : (
            <div className={styles.devSection}>
              <p className={styles.hint}>
                {t('wallet.devTestAccount')}
              </p>
              <select 
                className={styles.select}
                value={selectedAccount}
                onChange={(e) => setSelectedAccount(e.target.value)}
              >
                <option value="">{t('wallet.selectAccount')}</option>
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
                {t('wallet.connect')}
              </button>
            </div>
          )}
        </div>
      )}

      <div className={styles.info}>
        <p className={styles.note}>
          {t('wallet.seedNote')}
        </p>
      </div>
    </div>
  )
}
