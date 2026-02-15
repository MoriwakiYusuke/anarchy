'use client'

import { useState, useRef, useEffect, useMemo } from 'react'
import { PolkadotSigner } from 'polkadot-api/signer'
import { Binary } from 'polkadot-api'
import { usePostCost, calculatePostCost } from '@/hooks/usePostCost'
import { useStorage, createStorageSigner, StorageSigner } from '@/hooks/useStorage'
import { useLocale } from '@/i18n'
import styles from './PostForm.module.css'

interface Props {
  unsafeApi: any
  signer: PolkadotSigner | null
  derivePath: string
  onPostSuccess?: () => void
}

// エラーコードから翻訳キーへのマッピング
const ERROR_KEY_MAP: Record<string, string> = {
  // Post pallet errors
  'ContentTooLong': 'error.contentTooLong',
  'TooManyPosts': 'error.tooManyPosts',
  'ParentPostNotFound': 'error.parentPostNotFound',
  'InsufficientMoralBalance': 'error.insufficientMoralBalance',
  // Moral pallet errors
  'InsufficientBalance': 'error.insufficientBalance',
  'Overflow': 'error.overflow',
  'SelfTransfer': 'error.selfTransfer',
  // System errors
  'ExhaustsResources': 'error.exhaustsResources',
  'InvalidTransaction': 'error.invalidTransaction',
  'BadOrigin': 'error.badOrigin',
  // Invalid transaction errors (Substrate system level)
  'Payment': 'error.payment',
  'Invalid': 'error.invalidTransaction',
}

// parseError用の柔軟な型（動的エラーキー対応）
type TranslateFunc = (key: string, params?: Record<string, string | number>) => string

// エラーオブジェクトから読みやすいメッセージを抽出
function parseError(error: any, t: TranslateFunc): string {
  // Invalid Transaction エラー形式 { type: "Invalid", value: { type: "Payment" } }
  if (error?.type === 'Invalid' && error?.value?.type) {
    const key = error.value.type
    if (ERROR_KEY_MAP[key]) {
      return t(ERROR_KEY_MAP[key])
    }
    return t('error.invalidTransaction')
  }

  // PAPI のディスパッチエラー形式
  if (error?.type === 'Module') {
    const moduleName = error.value?.type || ''
    const errorName = error.value?.value?.type || ''
    const key = errorName || moduleName
    if (ERROR_KEY_MAP[key]) {
      return t(ERROR_KEY_MAP[key])
    }
    return t('error.moduleError', { module: moduleName, error: errorName })
  }
  
  // エラーオブジェクトを探索
  if (typeof error === 'object' && error !== null) {
    // dispatchError を持つ場合
    if (error.dispatchError) {
      return parseError(error.dispatchError, t)
    }
    
    // type と value を持つ場合（PAPI形式）
    if (error.type && error.value) {
      const key = error.value?.type || error.type
      if (ERROR_KEY_MAP[key]) {
        return t(ERROR_KEY_MAP[key])
      }
      return `${error.type}: ${JSON.stringify(error.value)}`
    }
    
    // message を持つ標準Errorの場合
    if (error.message) {
      return error.message
    }
  }
  
  // 文字列の場合
  if (typeof error === 'string') {
    if (ERROR_KEY_MAP[error]) {
      return t(ERROR_KEY_MAP[error])
    }
    return error
  }
  
  return t('error.unknown')
}

export function PostForm({ unsafeApi, signer, derivePath, onPostSuccess }: Props) {
  const { t } = useLocale()
  const [content, setContent] = useState('')
  const [isSubmitting, setIsSubmitting] = useState(false)
  const [status, setStatus] = useState<{ type: 'info' | 'success' | 'error'; message: string } | null>(null)
  
  // Storage認証用のsigner
  const [storageSigner, setStorageSigner] = useState<StorageSigner | null>(null)
  
  // derivePathが変わったらstorageSignerを再作成
  useEffect(() => {
    if (derivePath) {
      createStorageSigner(derivePath).then(setStorageSigner).catch(console.error)
    }
  }, [derivePath])
  
  // signerオプションをメモ化
  const storageOptions = useMemo(() => 
    storageSigner ? { signer: storageSigner } : undefined
  , [storageSigner])
  
  // V2: Storage Hook (現在は常に使用)
  const { uploadContent, progress: uploadProgress, error: uploadError, isProcessing, isReady: storageReady } = useStorage(storageOptions)

  // ブロックチェーンからコスト設定を動的に取得
  const costConfig = usePostCost(unsafeApi)

  // バイト数とコストをリアルタイム計算
  const contentBytes = new TextEncoder().encode(content)
  const byteCount = contentBytes.length
  const estimatedCost = calculatePostCost(byteCount, costConfig)

  // SSS固定パラメータ
  const SSS_K = 3  // 復元に必要な断片数
  const SSS_N = 5  // 総断片数

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!unsafeApi || !signer || !content.trim() || !storageReady) return

    setIsSubmitting(true)
    setStatus({ type: 'info', message: t('post.uploading') })

    try {
      // SSS分割 → Storage Node アップロード → create_post
      const contentBytes = new TextEncoder().encode(content)
      
      // 1. SSS分割してアップロード
      setStatus({ type: 'info', message: t('post.splitting') })
      const uploadResult = await uploadContent(new Uint8Array(contentBytes))
      
      // 2. create_postを呼び出し（MerkleRootをチェーンに記録）
      setStatus({ type: 'info', message: t('post.recording') })
      
      if (!unsafeApi.tx.Post?.create_post) {
        throw new Error('create_post not found in Post pallet')
      }
      
      const tx = unsafeApi.tx.Post.create_post({
        merkle_root: Binary.fromBytes(uploadResult.merkleRoot),
        k: SSS_K,
        n: SSS_N,
        total_size: BigInt(uploadResult.totalSize),
        parent_id: undefined
      })

      const result = await tx.signAndSubmit(signer)
      
      console.log('Transaction result:', result)
      
      if (result.ok) {
        setStatus({ 
          type: 'success', 
          message: t('post.success', { block: result.block.number.toString() })
        })
        setContent('')
        onPostSuccess?.()
        setTimeout(() => setStatus(null), 3000)
      } else {
        const errorMessage = parseError(result.dispatchError, t as TranslateFunc)
        console.error('Transaction failed:', result.dispatchError)
        setStatus({ type: 'error', message: errorMessage })
      }
    } catch (err: any) {
      console.error('Transaction error:', err)
      // エラーオブジェクトの構造を確認してparseErrorを使う
      let errorMessage: string
      if (err?.type) {
        // PAPI形式のエラーオブジェクト { type: "Invalid", value: { type: "Payment" } }
        errorMessage = parseError(err, t as TranslateFunc)
      } else if (err instanceof Error) {
        // Errorオブジェクトの場合、messageがJSON形式かもしれない
        try {
          const parsed = JSON.parse(err.message)
          errorMessage = parseError(parsed, t as TranslateFunc)
        } catch {
          errorMessage = err.message
        }
      } else {
        errorMessage = parseError(err, t as TranslateFunc)
      }
      setStatus({ type: 'error', message: errorMessage })
    } finally {
      setIsSubmitting(false)
    }
  }

  return (
    <form className={styles.form} onSubmit={handleSubmit}>
      <textarea
        className={styles.textarea}
        placeholder={t('post.placeholder')}
        value={content}
        onChange={(e) => setContent(e.target.value)}
        maxLength={10000}
        rows={4}
      />
      <div className={styles.footer}>
        <span className={styles.charCount}>
          {byteCount.toLocaleString()} bytes
        </span>
        <span className={styles.cost}>
          {costConfig.isLoading ? (
            t('common.loading')
          ) : (
            <>
              {t('post.cost', { cost: estimatedCost.toFixed(1) })}
              {!costConfig.isFromChain && <span title={t('post.defaultCostNote')}> *</span>}
            </>
          )}
        </span>
        <button 
          className={styles.submitBtn}
          type="submit"
          disabled={isSubmitting || !content.trim() || !unsafeApi || !signer || costConfig.isLoading || !storageReady}
        >
          {isSubmitting ? t('post.submitting') : t('post.submit')}
        </button>
      </div>
      {/* プログレスバー */}
      {isProcessing && (
        <div className={styles.progressContainer}>
          <div 
            className={styles.progressBar} 
            style={{ width: `${uploadProgress}%` }}
          />
          <span className={styles.progressText}>{uploadProgress}%</span>
        </div>
      )}
      {status && (
        <div className={`${styles.status} ${styles[status.type]}`}>
          {status.message}
        </div>
      )}
      {/* uploadErrorはstatusがエラーでない場合のみ表示（2重表示防止） */}
      {uploadError && status?.type !== 'error' && (
        <div className={`${styles.status} ${styles.error}`}>
          {uploadError}
        </div>
      )}
    </form>
  )
}
