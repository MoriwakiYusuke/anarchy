'use client'

import { useState } from 'react'
import { Binary } from 'polkadot-api'
import { PolkadotSigner } from 'polkadot-api/signer'
import styles from './PostForm.module.css'

interface Props {
  unsafeApi: any
  signer: PolkadotSigner | null
  derivePath: string
}

export function PostForm({ unsafeApi, signer, derivePath }: Props) {
  const [content, setContent] = useState('')
  const [isSubmitting, setIsSubmitting] = useState(false)
  const [status, setStatus] = useState<string | null>(null)

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!unsafeApi || !signer || !content.trim()) return

    setIsSubmitting(true)
    setStatus('投稿を送信中...')

    try {
      // Post palletのcreate_postを呼び出し
      // PAPI uses Binary type for byte arrays
      const contentBytes = Binary.fromText(content)
      
      // Check if post pallet exists
      if (!unsafeApi.tx.Post) {
        throw new Error('Post pallet not found in chain')
      }
      
      const tx = unsafeApi.tx.Post.create_post({
        content: contentBytes,
        parent_id: undefined
      })

      const result = await tx.signAndSubmit(signer)
      
      if (result.ok) {
        setStatus(`投稿が完了しました！ (ブロック #${result.block.number.toString()})`)
        setContent('')
        setTimeout(() => setStatus(null), 3000)
      } else {
        setStatus(`エラー: トランザクションが失敗しました`)
      }
    } catch (err) {
      setStatus(`エラー: ${err instanceof Error ? err.message : '不明なエラー'}`)
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
          {content.length} / 10,000
        </span>
        <button 
          className={styles.submitBtn}
          type="submit"
          disabled={isSubmitting || !content.trim() || !unsafeApi || !signer}
        >
          {isSubmitting ? '送信中...' : '投稿'}
        </button>
      </div>
      {status && (
        <div className={styles.status}>
          {status}
        </div>
      )}
    </form>
  )
}
