/**
 * PoW Faucet Web Worker
 * 
 * メインスレッドをブロックせずにPoW計算を実行
 * 進捗報告とキャンセル機能をサポート
 */

import { blake2b } from 'blakejs'

// メッセージタイプ定義
export interface MineRequest {
  type: 'start'
  challenge: Uint8Array
  difficulty: number
  startNonce?: bigint
}

export interface MineProgress {
  type: 'progress'
  nonce: bigint
  hashRate: number  // hashes per second
  elapsed: number   // milliseconds
}

export interface MineSolution {
  type: 'solution'
  nonce: bigint
  elapsed: number   // milliseconds
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
  const { type, challenge, difficulty, startNonce = BigInt(0) } = event.data
  
  if (type !== 'start') return
  
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
        const response: MineSolution = {
          type: 'solution',
          nonce,
          elapsed,
        }
        ctx.postMessage(response)
        return
      }
      
      nonce++
      
      // 進捗報告
      if (Number(nonce - lastProgressNonce) >= PROGRESS_INTERVAL) {
        const now = performance.now()
        const elapsed = now - startTime
        const intervalTime = now - lastProgressTime
        const hashRate = (PROGRESS_INTERVAL / intervalTime) * 1000
        
        const progress: MineProgress = {
          type: 'progress',
          nonce,
          hashRate,
          elapsed,
        }
        ctx.postMessage(progress)
        
        lastProgressNonce = nonce
        lastProgressTime = now
      }
    }
  } catch (error) {
    const errorResponse: MineError = {
      type: 'error',
      message: error instanceof Error ? error.message : 'Unknown error',
    }
    ctx.postMessage(errorResponse)
  }
}

// Worker初期化完了通知
ctx.postMessage({ type: 'ready' })
