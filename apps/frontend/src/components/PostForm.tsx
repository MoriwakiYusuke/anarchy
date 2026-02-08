'use client'

import { useState } from 'react'
import { Binary } from 'polkadot-api'
import { PolkadotSigner } from 'polkadot-api/signer'
import { usePostCost, calculatePostCost } from '@/hooks/usePostCost'
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

  // ブロックチェーンからコスト設定を動的に取得
  const costConfig = usePostCost(unsafeApi)

  // バイト数とコストをリアルタイム計算
  const contentBytes = new TextEncoder().encode(content)
  const byteCount = contentBytes.length
  const estimatedCost = calculatePostCost(byteCount, costConfig)

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!unsafeApi || !signer || !content.trim()) return

    setIsSubmitting(true)
    setStatus({ type: 'info', message: t('post.sending') })

    try {
      // Post palletのcreate_postを呼び出し
      const contentBytes = Binary.fromText(content)
      
      if (!unsafeApi.tx.Post) {
        throw new Error('Post pallet not found in chain')
      }
      
      const tx = unsafeApi.tx.Post.create_post({
        content: contentBytes,
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
        // トランザクションが失敗した場合のエラー詳細
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
          disabled={isSubmitting || !content.trim() || !unsafeApi || !signer || costConfig.isLoading}
        >
          {isSubmitting ? t('post.submitting') : t('post.submit')}
        </button>
      </div>
      {status && (
        <div className={`${styles.status} ${styles[status.type]}`}>
          {status.message}
        </div>
      )}
    </form>
  )
}
