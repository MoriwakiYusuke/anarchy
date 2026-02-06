'use client'

import { useState } from 'react'
import { ApiPromise } from '@polkadot/api'
import { Keyring } from '@polkadot/keyring'
import styles from './PostForm.module.css'

interface Props {
  api: ApiPromise | null
  account: string
}

export function PostForm({ api, account }: Props) {
  const [content, setContent] = useState('')
  const [isSubmitting, setIsSubmitting] = useState(false)
  const [status, setStatus] = useState<string | null>(null)

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!api || !content.trim()) return

    setIsSubmitting(true)
    setStatus('投稿を送信中...')

    try {
      // 開発用: シードから署名者を取得
      const keyring = new Keyring({ type: 'sr25519' })
      // accountはアドレスなので、対応するシードを見つける必要がある
      // 簡略化のためAliceを使用
      const signer = keyring.addFromUri('//Alice')

      // Post palletのcreate_postを呼び出し
      const tx = api.tx.post.createPost(content, null)

      await tx.signAndSend(signer, ({ status, events }) => {
        if (status.isInBlock) {
          setStatus(`ブロック ${status.asInBlock} に含まれました`)
        } else if (status.isFinalized) {
          setStatus('投稿が完了しました！')
          setContent('')
          setTimeout(() => setStatus(null), 3000)
        }
      })
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
          disabled={isSubmitting || !content.trim() || !api}
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
