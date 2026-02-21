/**
 * WorkerPool Tests
 * 
 * T058: Test worker pool limits worker count
 */

import { WorkerPool, getSharedWorkerPool, resetSharedWorkerPool } from '@/workers/WorkerPool'

// Web Worker モック
class MockWorker {
  onmessage: ((event: MessageEvent) => void) | null = null
  onerror: ((event: ErrorEvent) => void) | null = null
  private terminated = false
  private eventListeners: Map<string, ((event: MessageEvent) => void)[]> = new Map()

  constructor() {
    // Worker初期化完了を模擬（非同期）
    setTimeout(() => {
      if (!this.terminated) {
        // ready イベントを送信
        const readyEvent = { data: { type: 'ready' } } as MessageEvent
        this.eventListeners.get('message')?.forEach(handler => handler(readyEvent))
        this.onmessage?.(readyEvent)
      }
    }, 0)
  }

  addEventListener(type: string, handler: (event: MessageEvent) => void) {
    const handlers = this.eventListeners.get(type) || []
    handlers.push(handler)
    this.eventListeners.set(type, handlers)
  }

  removeEventListener(type: string, handler: (event: MessageEvent) => void) {
    const handlers = this.eventListeners.get(type) || []
    const index = handlers.indexOf(handler)
    if (index !== -1) {
      handlers.splice(index, 1)
      this.eventListeners.set(type, handlers)
    }
  }

  postMessage(data: { id: string; type: string; payload: unknown }) {
    if (this.terminated) return

    // 非同期でレスポンスを返す
    setTimeout(() => {
      if (this.terminated) return

      // タスクタイプに応じたモックレスポンス
      let result: unknown
      switch (data.type) {
        case 'blake2b_hash':
          result = new Uint8Array(32)
          break
        case 'hybrid_split':
          result = {
            shards: [new Uint8Array(10), new Uint8Array(10)],
            shardHashes: [new Uint8Array(32), new Uint8Array(32)],
            originalLen: 100,
            ciphertextLen: 116,
            shardSize: 10,
            compressed: false,
            threshold: 2,
            totalShards: 3,
          }
          break
        default:
          result = null
      }

      const responseEvent = {
        data: {
          id: data.id,
          success: true,
          result,
        },
      } as MessageEvent
      
      this.onmessage?.(responseEvent)
    }, 10)
  }

  terminate() {
    this.terminated = true
  }
}

// グローバルWorkerをモック
const originalWorker = global.Worker
beforeAll(() => {
  // @ts-expect-error - モック
  global.Worker = MockWorker
})

afterAll(() => {
  global.Worker = originalWorker
})

afterEach(() => {
  resetSharedWorkerPool()
})

describe('WorkerPool', () => {
  describe('constructor', () => {
    it('creates workers up to maxSize limit', async () => {
      const pool = new WorkerPool({ size: 20, maxSize: 8 })
      
      // maxSize (8) を超えないことを確認
      expect(pool.size).toBeLessThanOrEqual(8)
      
      pool.terminate()
    })

    it('respects provided size when under maxSize', async () => {
      const pool = new WorkerPool({ size: 4, maxSize: 8 })
      
      expect(pool.size).toBe(4)
      
      pool.terminate()
    })

    it('uses navigator.hardwareConcurrency as default size', async () => {
      // navigator.hardwareConcurrency が定義されている場合、それを使用
      const originalHardwareConcurrency = navigator.hardwareConcurrency
      Object.defineProperty(navigator, 'hardwareConcurrency', {
        value: 6,
        configurable: true,
      })

      const pool = new WorkerPool()
      expect(pool.size).toBe(6)
      
      pool.terminate()

      // 元に戻す
      Object.defineProperty(navigator, 'hardwareConcurrency', {
        value: originalHardwareConcurrency,
        configurable: true,
      })
    })
  })

  describe('test_worker_pool_limits_worker_count', () => {
    it('limits worker count to maxSize even with many concurrent tasks', async () => {
      const pool = new WorkerPool({ size: 2, maxSize: 4 })
      await pool.waitUntilReady()

      // プールサイズが上限内であることを確認
      expect(pool.size).toBeLessThanOrEqual(4)
      expect(pool.size).toBe(2)

      // 複数の並行タスクを実行
      const tasks = Array.from({ length: 10 }, () =>
        pool.execute('blake2b_hash', { data: new Uint8Array([1, 2, 3]) })
      )

      // 全タスクが完了してもworker数は増えない
      await Promise.all(tasks)
      expect(pool.size).toBe(2)

      pool.terminate()
    })
  })

  describe('execute', () => {
    it('distributes tasks across workers in round-robin fashion', async () => {
      const pool = new WorkerPool({ size: 3, maxSize: 8 })
      await pool.waitUntilReady()

      // 9タスクを実行（3ワーカー × 3ラウンド）
      const results = await Promise.all(
        Array.from({ length: 9 }, () =>
          pool.execute('blake2b_hash', { data: new Uint8Array([1, 2, 3]) })
        )
      )

      expect(results).toHaveLength(9)
      
      pool.terminate()
    })

    it('throws error when pool has no workers (after termination)', async () => {
      // ワーカーが終了した状態をテスト
      const pool = new WorkerPool({ size: 2 })
      await pool.waitUntilReady()
      
      // 全ワーカーを終了
      pool.terminate()
      expect(pool.size).toBe(0)

      // 終了後は実行できない
      await expect(pool.execute('test', {})).rejects.toThrow('no workers available')
    })
  })

  describe('waitUntilReady', () => {
    it('resolves when all workers are ready', async () => {
      const pool = new WorkerPool({ size: 3, maxSize: 8 })
      
      expect(pool.isReady).toBe(false)
      
      await pool.waitUntilReady()
      
      expect(pool.isReady).toBe(true)
      expect(pool.readyCount).toBe(3)
      
      pool.terminate()
    })
  })

  describe('terminate', () => {
    it('terminates all workers and clears state', async () => {
      const pool = new WorkerPool({ size: 2, maxSize: 8 })
      await pool.waitUntilReady()
      
      expect(pool.size).toBe(2)
      
      pool.terminate()
      
      expect(pool.size).toBe(0)
      expect(pool.readyCount).toBe(0)
    })
  })

  describe('getSharedWorkerPool', () => {
    it('returns the same instance on multiple calls', () => {
      const pool1 = getSharedWorkerPool()
      const pool2 = getSharedWorkerPool()
      
      expect(pool1).toBe(pool2)
    })

    it('creates new instance after reset', () => {
      const pool1 = getSharedWorkerPool()
      resetSharedWorkerPool()
      const pool2 = getSharedWorkerPool()
      
      expect(pool1).not.toBe(pool2)
    })
  })
})
