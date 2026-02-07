'use client'

import { useState, useCallback } from 'react'

// 型定義のみインポート（実行時コードなし）
// KeyringPairの型は動的インポート後に取得

export interface UseSeedPhraseResult {
  /** 現在のシードフレーズ（入力または生成） */
  seedPhrase: string
  /** シードフレーズから導出されたAccountId */
  accountId: string | null
  /** シードフレーズが有効かどうか */
  isValid: boolean
  /** エラーメッセージ */
  error: string | null
  /** キーペア（署名用、メモリ内のみ） */
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  keyPair: any | null
  /** シードフレーズを設定 */
  setSeedPhrase: (phrase: string) => Promise<void>
  /** 新しいシードフレーズを生成 */
  generateNewSeedPhrase: () => Promise<string>
  /** クリア（切断） */
  clear: () => void
}

/**
 * シードフレーズ管理フック
 * 
 * - 12/24ワードのBIP39ニーモニックをサポート
 * - 開発用パス（//Alice等）もサポート
 * - キーペアはメモリ内のみ（永続化なし）
 */
export function useSeedPhrase(): UseSeedPhraseResult {
  const [seedPhrase, setSeedPhraseState] = useState<string>('')
  const [accountId, setAccountId] = useState<string | null>(null)
  const [isValid, setIsValid] = useState(false)
  const [error, setError] = useState<string | null>(null)
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const [keyPair, setKeyPair] = useState<any | null>(null)

  /**
   * シードフレーズを設定し、AccountIdを導出
   */
  const setSeedPhrase = useCallback(async (phrase: string) => {
    setSeedPhraseState(phrase)
    
    if (!phrase.trim()) {
      setAccountId(null)
      setIsValid(false)
      setError(null)
      setKeyPair(null)
      return
    }

    try {
      // WASM暗号モジュールの初期化を待つ（動的インポート）
      const { cryptoWaitReady, mnemonicValidate } = await import('@polkadot/util-crypto')
      await cryptoWaitReady()

      const trimmedPhrase = phrase.trim()
      
      // 開発用パス（//Alice等）または有効なニーモニックかチェック
      const isDevPath = trimmedPhrase.startsWith('//')
      const isValidMnemonic = mnemonicValidate(trimmedPhrase)

      if (!isDevPath && !isValidMnemonic) {
        setAccountId(null)
        setIsValid(false)
        setError('無効なシードフレーズです。12または24単語のニーモニックを入力してください。')
        setKeyPair(null)
        return
      }

      // キーペアを生成（動的インポート）
      const { Keyring } = await import('@polkadot/keyring')
      const keyring = new Keyring({ type: 'sr25519' })
      const pair = keyring.addFromUri(trimmedPhrase)
      
      setAccountId(pair.address)
      setIsValid(true)
      setError(null)
      setKeyPair(pair)
    } catch (err) {
      setAccountId(null)
      setIsValid(false)
      setError(err instanceof Error ? err.message : '不明なエラー')
      setKeyPair(null)
    }
  }, [])

  /**
   * 新しいシードフレーズを生成
   */
  const generateNewSeedPhrase = useCallback(async (): Promise<string> => {
    const { cryptoWaitReady, mnemonicGenerate } = await import('@polkadot/util-crypto')
    await cryptoWaitReady()
    
    // 12ワードのニーモニックを生成
    const newMnemonic = mnemonicGenerate(12)
    
    // 生成したニーモニックを設定
    await setSeedPhrase(newMnemonic)
    
    return newMnemonic
  }, [setSeedPhrase])

  /**
   * クリア（切断）
   */
  const clear = useCallback(() => {
    setSeedPhraseState('')
    setAccountId(null)
    setIsValid(false)
    setError(null)
    setKeyPair(null)
  }, [])

  return {
    seedPhrase,
    accountId,
    isValid,
    error,
    keyPair,
    setSeedPhrase,
    generateNewSeedPhrase,
    clear,
  }
}
