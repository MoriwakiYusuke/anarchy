'use client'

import { useState, useRef, useEffect, useMemo, useCallback } from 'react'
import { PolkadotSigner } from 'polkadot-api/signer'
import { Binary } from 'polkadot-api'
import { usePostCost, calculatePostCost } from '@/hooks/usePostCost'
import { useStorage, type StorageSigner } from '@/hooks/useStorage'
import { useLocale } from '@/i18n'
import MediaUpload from '@/components/MediaUpload'
import type { MediaFile } from '@/types/media'
import { encodePostContent, loadMediaFile, isSupportedMediaType } from '@/lib/postCodec'
import type { MediaItem, MediaType as PostMediaType } from '@/lib/postCodec'
import styles from './PostForm.module.css'

interface Props {
  unsafeApi: any
  signer: PolkadotSigner | null
  /** Raw sr25519 signer for storage upload auth. AccountContext.mainRawSigner を渡す。 */
  storageSigner: StorageSigner | null
  onPostSuccess?: () => void
  /** 返信時の親 post_id。設定されると Replying to ヘッダーが出てキャンセル UI が有効になる */
  parentId?: number
  /** インライン返信フォームをキャンセルするコールバック */
  onCancel?: () => void
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

export function PostForm({ unsafeApi, signer, storageSigner, onPostSuccess, parentId, onCancel }: Props) {
  const { t } = useLocale()
  const [content, setContent] = useState('')
  const [isSubmitting, setIsSubmitting] = useState(false)
  const [status, setStatus] = useState<{ type: 'info' | 'success' | 'error'; message: string } | null>(null)

  // Media files (not yet uploaded)
  const [mediaFiles, setMediaFiles] = useState<MediaFile[]>([])

  // Reset key for MediaUpload component (increment to force remount)
  const [mediaResetKey, setMediaResetKey] = useState(0)

  // signerオプションをメモ化
  const storageOptions = useMemo(() =>
    storageSigner ? { signer: storageSigner } : undefined
  , [storageSigner])
  
  // V2: Storage Hook (現在は常に使用)
  const { uploadContent, progress: uploadProgress, error: uploadError, isProcessing, isReady: storageReady } = useStorage(storageOptions)

  // ブロックチェーンからコスト設定を動的に取得
  const costConfig = usePostCost(unsafeApi)

  // バイト数とコストをリアルタイム計算（テキスト＋画像サイズ）
  const textBytes = new TextEncoder().encode(content)
  const mediaByteSize = mediaFiles.reduce((sum, f) => sum + f.size, 0)
  const byteCount = textBytes.length + mediaByteSize
  const estimatedCost = calculatePostCost(byteCount, costConfig)

  // SSS固定パラメータ
  const SSS_K = 3  // 復元に必要な断片数
  const SSS_N = 5  // 総断片数

  // Handle media files change
  const handleMediaFilesChange = useCallback((files: MediaFile[]) => {
    setMediaFiles(files)
  }, [])

  // Handle media upload error
  const handleMediaUploadError = useCallback((error: string) => {
    setStatus({ type: 'error', message: error })
  }, [])

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!unsafeApi || !signer || !content.trim() || !storageReady) return

    setIsSubmitting(true)
    setStatus({ type: 'info', message: t('post.uploading') })

    try {
      // 1. 画像ファイルを読み込み、MediaItemに変換
      const mediaItems: MediaItem[] = []
      for (const mediaFile of mediaFiles) {
        if (!isSupportedMediaType(mediaFile.file.type)) {
          throw new Error(`Unsupported media type: ${mediaFile.file.type}`)
        }
        const item = await loadMediaFile(mediaFile.file)
        mediaItems.push(item)
      }
      
      // 2. テキスト+画像をバイナリにエンコード
      const postContent = { text: content, media: mediaItems }
      const encodedContent = encodePostContent(postContent)
      
      // 3. SSS分割してアップロード
      setStatus({ type: 'info', message: t('post.splitting') })
      const uploadResult = await uploadContent(encodedContent)
      
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
        ciphertext_len: BigInt(uploadResult.metadata.ciphertextLen),
        shard_size: uploadResult.metadata.shardSize,
        compressed: uploadResult.metadata.compressed,
        parent_id: parentId !== undefined ? BigInt(parentId) : undefined,
      })

      const result = await tx.signAndSubmit(signer)
      
      if (result.ok) {
        setStatus({ 
          type: 'success', 
          message: t('post.success', { block: result.block.number.toString() })
        })
        setContent('')
        setMediaFiles([]) // Clear media state
        setMediaResetKey(prev => prev + 1) // Force MediaUpload component to reset
        onPostSuccess?.()
        setTimeout(() => setStatus(null), 3000)
      } else {
        const errorMessage = parseError(result.dispatchError, t as TranslateFunc)
        setStatus({ type: 'error', message: errorMessage })
      }
    } catch (err: any) {
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
      {parentId !== undefined && (
        <div className={styles.replyHeader}>
          <span className={styles.replyHeaderLabel}>
            {t('post.replyTo', { id: parentId })}
          </span>
          {onCancel && (
            <button
              type="button"
              className={styles.replyCancelBtn}
              onClick={onCancel}
              disabled={isSubmitting}
            >
              {t('post.cancelReply')}
            </button>
          )}
        </div>
      )}
      <textarea
        className={styles.textarea}
        placeholder={t('post.placeholder')}
        value={content}
        onChange={(e) => setContent(e.target.value)}
        rows={4}
      />
      
      {/* Media Upload Section */}
      <MediaUpload
        key={mediaResetKey}
        disabled={isSubmitting}
        includeVideo={true}
        onFilesChange={handleMediaFilesChange}
        onError={handleMediaUploadError}
        className={styles.mediaUpload}
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
