'use client'

import { useState } from 'react'
import { Binary } from 'polkadot-api'
import { PolkadotSigner } from 'polkadot-api/signer'
import { usePostCost, calculatePostCost } from '@/hooks/usePostCost'
import styles from './PostForm.module.css'

interface Props {
  unsafeApi: any
  signer: PolkadotSigner | null
  derivePath: string
  onPostSuccess?: () => void
}

// パレットエラーの日本語マッピング
const ERROR_MESSAGES: Record<string, string> = {
  // Post pallet errors
  'ContentTooLong': 'コンテンツが長すぎます（最大10,000バイト）',
  'TooManyPosts': '投稿数の上限に達しました',
  'ParentPostNotFound': '返信先の投稿が見つかりません',
  'InsufficientMoralBalance': '$moral残高が不足しています',
  // Moral pallet errors
  'InsufficientBalance': '$moral残高が不足しています',
  'Overflow': '数値がオーバーフローしました',
  'SelfTransfer': '自分自身への転送はできません',
  // System errors
  'ExhaustsResources': 'リソースが枯渇しました',
  'InvalidTransaction': '無効なトランザクションです',
  'BadOrigin': '権限がありません',
}

// エラーオブジェクトから読みやすいメッセージを抽出
function parseError(error: any): string {
  // PAPI のディスパッチエラー形式
  if (error?.type === 'Module') {
    const moduleName = error.value?.type || ''
    const errorName = error.value?.value?.type || ''
    const key = errorName || moduleName
    return ERROR_MESSAGES[key] || `モジュールエラー: ${moduleName}::${errorName}`
  }
  
  // エラーオブジェクトを探索
  if (typeof error === 'object' && error !== null) {
    // dispatchError を持つ場合
    if (error.dispatchError) {
      return parseError(error.dispatchError)
    }
    
    // type と value を持つ場合（PAPI形式）
    if (error.type && error.value) {
      const key = error.value?.type || error.type
      if (ERROR_MESSAGES[key]) {
        return ERROR_MESSAGES[key]
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
    return ERROR_MESSAGES[error] || error
  }
  
  return '不明なエラーが発生しました'
}

export function PostForm({ unsafeApi, signer, derivePath, onPostSuccess }: Props) {
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
    setStatus({ type: 'info', message: '投稿を送信中...' })

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
          message: `投稿が完了しました！ (ブロック #${result.block.number.toString()})` 
        })
        setContent('')
        onPostSuccess?.()
        setTimeout(() => setStatus(null), 3000)
      } else {
        // トランザクションが失敗した場合のエラー詳細
        const errorMessage = parseError(result.dispatchError)
        console.error('Transaction failed:', result.dispatchError)
        setStatus({ type: 'error', message: errorMessage })
      }
    } catch (err) {
      console.error('Transaction error:', err)
      const errorMessage = err instanceof Error ? err.message : parseError(err)
      setStatus({ type: 'error', message: errorMessage })
    } finally {
      setIsSubmitting(false)
    }
  }

  return (
    <form className={styles.form} onSubmit={handleSubmit}>
      <textarea
        className={styles.textarea}
        placeholder="今、何を考えていますか？（匿名で投稿されます）"
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
            '読込中...'
          ) : (
            <>
              投稿コスト: {estimatedCost.toFixed(1)} $moral
              {!costConfig.isFromChain && <span title="チェーンから取得できないためデフォルト値を使用"> *</span>}
            </>
          )}
        </span>
        <button 
          className={styles.submitBtn}
          type="submit"
          disabled={isSubmitting || !content.trim() || !unsafeApi || !signer || costConfig.isLoading}
        >
          {isSubmitting ? '送信中...' : '投稿'}
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
