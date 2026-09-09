/**
 * PoW Reaction Mining Web Worker
 * 
 * メインスレッドをブロックせずにPoW計算を実行
 * faucet/worker.tsと同じ構造でpure-JS実装
 * 
 * NOTE: BigIntはpostMessageでシリアライズできないため、stringで送受信
 */

import { blake2b } from 'blakejs'

// メッセージタイプ定義（stringで送受信）
export interface MineRequest {
  type: 'start'
  challenge: Uint8Array
  difficulty: number
  // string (旧プロトコル) と bigint (structured clone) の両方を受理する。
  startNonce?: string | bigint
}

export interface MineProgress {
  type: 'progress'
  nonce: string // BigInt as string
  hashRate: number
  elapsed: number
}

export interface MineSolution {
  type: 'solution'
  nonce: string // BigInt as string
  hashRate: number
  elapsed: number
}

export interface MineError {
  type: 'error'
  message: string
}

export type WorkerMessage = MineProgress | MineSolution | MineError

// 進捗報告間隔（ハッシュ回数）
const PROGRESS_INTERVAL = 50000

/**
 * PoWハッシュを計算
 */
function computePoWHash(challenge: Uint8Array, nonce: bigint): Uint8Array {
  const nonceBytes = new Uint8Array(8)
  const view = new DataView(nonceBytes.buffer)
  view.setBigUint64(0, nonce, true)
  
  const input = new Uint8Array(challenge.length + nonceBytes.length)
  input.set(challenge)
  input.set(nonceBytes, challenge.length)
  
  return blake2b(input, undefined, 32)
}

/**
 * 先頭ゼロビット数をカウント
 */
function countLeadingZeroBits(hash: Uint8Array): number {
  let zeros = 0
  
  for (let i = 0; i < hash.length; i++) {
    const byte = hash[i]
    if (byte === 0) {
      zeros += 8
    } else {
      zeros += Math.clz32(byte) - 24
      break
    }
  }
  
  return zeros
}

// Worker context
const ctx: Worker = self as unknown as Worker

ctx.onmessage = (event: MessageEvent<MineRequest>) => {
  const { type, challenge, difficulty, startNonce: startNonceStr } = event.data
  
  if (type !== 'start') return
  
  // Parse startNonce from string (BigInt can't be serialized via postMessage)
  const startNonce = startNonceStr ? BigInt(startNonceStr) : BigInt(0)
  
  const startTime = performance.now()
  let nonce = startNonce
  let lastProgressNonce = nonce
  let lastProgressTime = startTime
  
  try {
    while (true) {
      const hash = computePoWHash(challenge, nonce)
      const leadingZeros = countLeadingZeroBits(hash)
      
      if (leadingZeros >= difficulty) {
        // 解発見
        const elapsed = performance.now() - startTime
        const hashRate = elapsed > 0 ? Number(nonce - startNonce) / (elapsed / 1000) : 0
        const response: MineSolution = {
          type: 'solution',
          nonce: nonce.toString(), // Convert to string for postMessage
          hashRate: Math.round(hashRate),
          elapsed: Math.round(elapsed),
        }
        ctx.postMessage(response)
        return
      }
      
      nonce++
      
      // 進捗報告
      if (Number(nonce - lastProgressNonce) >= PROGRESS_INTERVAL) {
        const now = performance.now()
        const elapsed = now - lastProgressTime
        const hashRate = elapsed > 0 ? PROGRESS_INTERVAL / (elapsed / 1000) : 0
        
        const progress: MineProgress = {
          type: 'progress',
          nonce: nonce.toString(), // Convert to string for postMessage
          hashRate: Math.round(hashRate),
          elapsed: Math.round(now - startTime),
        }
        ctx.postMessage(progress)
        
        lastProgressNonce = nonce
        lastProgressTime = now
      }
    }
  } catch (error) {
    const response: MineError = {
      type: 'error',
      message: error instanceof Error ? error.message : String(error),
    }
    ctx.postMessage(response)
  }
}

// Worker初期化完了通知
ctx.postMessage({ type: 'ready' })
