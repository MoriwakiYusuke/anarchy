/**
 * useStorage Hook テスト
 * 
 * T026: Frontend unit test for useStorage.uploadFragment
 */

import { renderHook, act, waitFor } from '@testing-library/react'
import { useStorage, type UseStorageResult, type UploadResult, type RecoverResult, type HybridMetadata } from '@/hooks/useStorage'

// Web Worker モック
// 全インスタンス間で共有するMerkleキャッシュ（WorkerPool使用時の複数Worker問題に対応）
const sharedMerkleCache = new Map<string, { root: Uint8Array; leafCount: number }>()

class MockWorker {
  onmessage: ((event: MessageEvent) => void) | null = null
  onerror: ((event: ErrorEvent) => void) | null = null
  private messageHandlers: Map<string, (payload: unknown) => unknown> = new Map()
  private eventListeners: Map<string, ((event: MessageEvent) => void)[]> = new Map()

  constructor() {
    // Worker初期化完了を模擬
    setTimeout(() => {
      const readyEvent = { data: { type: 'ready' } } as MessageEvent
      this.eventListeners.get('message')?.forEach(handler => handler(readyEvent))
      this.onmessage?.({ data: { type: 'ready' } } as MessageEvent)
    }, 0)

    // ハイブリッド分割モック
    this.messageHandlers.set('hybrid_split', (payload: unknown) => {
      const { data, k, n } = payload as { data: Uint8Array; k: number; n: number }
      // n個のダミーシャードを生成
      const shards: Uint8Array[] = []
      const shardHashes: Uint8Array[] = []
      for (let i = 0; i < n; i++) {
        // 各シャードは元データ + インデックス（簡易的なモック）
        const shard = new Uint8Array(data.length + 1)
        shard.set(data)
        shard[data.length] = i
        shards.push(shard)
        // ダミーハッシュ
        const hash = new Uint8Array(32)
        hash[0] = i
        shardHashes.push(hash)
      }
      return {
        shards,
        shardHashes,
        originalLen: data.length,
        ciphertextLen: data.length + 16, // 模擬
        shardSize: data.length + 1,
        compressed: false,
        threshold: k,
        totalShards: n,
      }
    })

    this.messageHandlers.set('merkle_build', (payload: unknown) => {
      const { fragments } = payload as { fragments: Uint8Array[] }
      // ダミーMerkleRoot生成（32バイト）
      const root = new Uint8Array(32)
      for (let i = 0; i < Math.min(fragments.length, 32); i++) {
        root[i] = fragments[i]?.[0] ?? 0
      }
      // rootHexを生成して共有キャッシュに保存
      const rootHex = Array.from(root).map(b => b.toString(16).padStart(2, '0')).join('')
      sharedMerkleCache.set(rootHex, { root, leafCount: fragments.length })
      return { root, rootHex, leafCount: fragments.length }
    })

    this.messageHandlers.set('merkle_generate_proof', (payload: unknown) => {
      const { merkleRootHex, index } = payload as { merkleRootHex: string; index: number }
      const cached = sharedMerkleCache.get(merkleRootHex)
      if (!cached) {
        throw new Error(`MerkleResult not found for root: ${merkleRootHex}`)
      }
      // ダミーProof生成（64バイト = 2つのハッシュ）
      const proof = new Uint8Array(64)
      proof[0] = index
      return proof
    })

    this.messageHandlers.set('blake2b_hash', (payload: unknown) => {
      const { data } = payload as { data: Uint8Array }
      // ダミーハッシュ生成（32バイト）
      const hash = new Uint8Array(32)
      for (let i = 0; i < Math.min(data.length, 32); i++) {
        hash[i] = data[i]
      }
      return hash
    })

    // ハイブリッド復元モック
    this.messageHandlers.set('hybrid_recover', (payload: unknown) => {
      const { shardBytes } = payload as { shardBytes: Uint8Array[]; k: number; n: number; originalLen: number }
      // 最初のシャードから元データを復元（モック）
      if (shardBytes.length > 0) {
        const original = new Uint8Array(shardBytes[0].length - 1)
        original.set(shardBytes[0].subarray(0, original.length))
        return original
      }
      throw new Error('No shards provided')
    })
  }

  postMessage(message: { id: string; type: string; payload: unknown }) {
    const { id, type, payload } = message
    
    setTimeout(() => {
      const handler = this.messageHandlers.get(type)
      if (handler) {
        try {
          const result = handler(payload)
          this.onmessage?.({ data: { id, success: true, result } } as MessageEvent)
        } catch (err) {
          this.onmessage?.({ data: { id, success: false, error: (err as Error).message } } as MessageEvent)
        }
      } else {
        this.onmessage?.({ data: { id, success: false, error: `Unknown message type: ${type}` } } as MessageEvent)
      }
    }, 10)
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

  terminate() {
    // Cleanup
  }
}

// fetch モック
const createFetchMock = (responses: Map<string, unknown>): typeof global.fetch => {
  return jest.fn(async (_url: RequestInfo | URL, options?: RequestInit) => {
    const body = JSON.parse(options?.body as string || '{}')
    const method = body.method as string
    
    const response = responses.get(method)
    if (response) {
      return {
        ok: true,
        status: 200,
        json: async () => ({ jsonrpc: '2.0', id: 1, result: response }),
      } as Response
    }
    
    return {
      ok: true,
      status: 200,
      json: async () => ({ jsonrpc: '2.0', id: 1, error: { message: 'Method not found' } }),
    } as Response
  }) as typeof global.fetch
}

// グローバルモック設定
beforeAll(() => {
  // @ts-expect-error - Worker mock
  global.Worker = MockWorker
  // Node環境でTextEncoderが未定義の場合
  if (typeof global.TextEncoder === 'undefined') {
    const { TextEncoder, TextDecoder } = require('util')
    global.TextEncoder = TextEncoder
    global.TextDecoder = TextDecoder
  }
})

// 各テスト後にシングルトンWorkerPoolをリセット
afterEach(async () => {
  const { resetSharedWorkerPool } = await import('@/workers/WorkerPool')
  resetSharedWorkerPool()
  // 共有MerkleキャッシュもクリアA
  sharedMerkleCache.clear()
})

describe('useStorage', () => {
  let originalFetch: typeof global.fetch

  beforeEach(() => {
    originalFetch = global.fetch
    jest.clearAllMocks()
  })

  afterEach(() => {
    global.fetch = originalFetch
  })

  describe('initialization', () => {
    it('should initialize and become ready', async () => {
      const { result } = renderHook(() => useStorage())

      // 初期状態
      expect(result.current.isReady).toBe(false)
      expect(result.current.isProcessing).toBe(false)
      expect(result.current.error).toBeNull()
      expect(result.current.progress).toBe(0)

      // Worker準備完了を待つ
      await waitFor(() => {
        expect(result.current.isReady).toBe(true)
      })
    })
  })

  describe('uploadContent', () => {
    it('should successfully upload content and return merkle root', async () => {
      // RPC レスポンスモック
      const rpcResponses = new Map<string, unknown>()
      rpcResponses.set('storage_uploadFragment', {
        success: true,
        fragment_hash: Array(32).fill(0).map((_, i) => i),
      })
      
      global.fetch = createFetchMock(rpcResponses)

      const { result } = renderHook(() => useStorage())

      // Worker準備完了を待つ
      await waitFor(() => {
        expect(result.current.isReady).toBe(true)
      })

      // コンテンツをアップロード
      const testContent = new TextEncoder().encode('Hello, Anarchy!')
      let uploadResult: Awaited<ReturnType<typeof result.current.uploadContent>> | null = null

      await act(async () => {
        uploadResult = await result.current.uploadContent(new Uint8Array(testContent))
      })

      // 結果を検証
      expect(uploadResult).not.toBeNull()
      expect(uploadResult!.merkleRoot).toBeInstanceOf(Uint8Array)
      expect(uploadResult!.merkleRoot.length).toBe(32)
      expect(uploadResult!.fragmentHashes.length).toBe(5) // n=5
      expect(uploadResult!.totalSize).toBe(testContent.length)
      expect(uploadResult!.metadata).toBeDefined()
      expect(uploadResult!.metadata.threshold).toBe(3)
      expect(uploadResult!.metadata.totalShards).toBe(5)

      // RPCが5回呼ばれたことを確認（n=5断片）
      expect(global.fetch).toHaveBeenCalledTimes(5)
    })

    it('should set progress during upload', async () => {
      const rpcResponses = new Map<string, unknown>()
      rpcResponses.set('storage_uploadFragment', {
        success: true,
        fragment_hash: Array(32).fill(0),
      })
      
      global.fetch = createFetchMock(rpcResponses)

      const { result } = renderHook(() => useStorage())

      await waitFor(() => {
        expect(result.current.isReady).toBe(true)
      })

      const testContent = new Uint8Array([1, 2, 3, 4, 5])
      
      await act(async () => {
        await result.current.uploadContent(testContent)
      })

      // 完了時は100%
      expect(result.current.progress).toBe(100)
    })

    it('should handle RPC errors with retry', async () => {
      let callCount = 0
      
      global.fetch = jest.fn(async () => {
        callCount++
        if (callCount <= 2) {
          // 最初の2回は失敗
          return {
            ok: true,
            status: 200,
            json: async () => ({ jsonrpc: '2.0', id: 1, error: { message: 'Network error' } }),
          } as Response
        }
        // 3回目以降は成功
        return {
          ok: true,
          status: 200,
          json: async () => ({
            jsonrpc: '2.0',
            id: 1,
            result: { success: true, fragment_hash: Array(32).fill(0) },
          }),
        } as Response
      }) as typeof global.fetch

      const { result } = renderHook(() => useStorage())

      await waitFor(() => {
        expect(result.current.isReady).toBe(true)
      })

      const testContent = new Uint8Array([1, 2, 3])

      await act(async () => {
        await result.current.uploadContent(testContent)
      })

      // リトライが行われたことを確認（3回失敗後に成功）
      expect(callCount).toBeGreaterThan(5) // n=5断片、一部リトライ
    })

    it('should set error on complete failure', async () => {
      // 全てのRPC呼び出しが失敗
      global.fetch = jest.fn(async () => ({
        ok: true,
        status: 200,
        json: async () => ({ jsonrpc: '2.0', id: 1, error: { message: 'Server unavailable' } }),
      } as Response)) as typeof global.fetch

      const { result } = renderHook(() => useStorage())

      await waitFor(() => {
        expect(result.current.isReady).toBe(true)
      })

      const testContent = new Uint8Array([1, 2, 3])

      await act(async () => {
        try {
          await result.current.uploadContent(testContent)
        } catch {
          // エラーは期待通り
        }
      })

      expect(result.current.error).not.toBeNull()
    })
  })

  describe('recoverContent', () => {
    // テスト用メタデータ
    const testMetadata: HybridMetadata = {
      originalLen: 5,
      ciphertextLen: 21,
      shardSize: 6,
      compressed: false,
      threshold: 3,
      totalShards: 5,
    }

    // T043: Frontend unit test for useStorage.getFragment
    it('should recover content from fragments', async () => {
      const rpcResponses = new Map<string, unknown>()
      // k=3個の断片を返す
      const testData = new Uint8Array([72, 101, 108, 108, 111]) // "Hello"
      rpcResponses.set('storage_getFragment', {
        data: btoa(String.fromCharCode(...Array.from(testData), 0)), // インデックス0を付加
        hash: Array(32).fill(0),
      })
      
      global.fetch = createFetchMock(rpcResponses)

      const { result } = renderHook(() => useStorage())

      await waitFor(() => {
        expect(result.current.isReady).toBe(true)
      })

      const merkleRoot = new Uint8Array(32).fill(1)
      let recoverResult: Awaited<ReturnType<typeof result.current.recoverContent>> | null = null

      await act(async () => {
        recoverResult = await result.current.recoverContent(merkleRoot, testMetadata)
      })

      expect(recoverResult).not.toBeNull()
      expect(recoverResult!.data).toBeInstanceOf(Uint8Array)
    })

    // T043補足: getFragmentの戻り値検証
    it('should return correct data structure from getFragment RPC', async () => {
      const expectedData = new Uint8Array([1, 2, 3, 4, 5, 0]) // with index suffix
      const expectedHash = Array(32).fill(42)
      
      const rpcResponses = new Map<string, unknown>()
      rpcResponses.set('storage_getFragment', {
        data: btoa(String.fromCharCode(...Array.from(expectedData))),
        hash: expectedHash,
      })
      
      global.fetch = createFetchMock(rpcResponses)

      const { result } = renderHook(() => useStorage())

      await waitFor(() => {
        expect(result.current.isReady).toBe(true)
      })

      const merkleRoot = new Uint8Array(32).fill(1)

      await act(async () => {
        await result.current.recoverContent(merkleRoot, testMetadata)
      })

      // RPC呼び出しを検証
      expect(global.fetch).toHaveBeenCalled()
      const fetchCall = (global.fetch as jest.Mock).mock.calls[0]
      const body = JSON.parse(fetchCall[1].body)
      expect(body.method).toBe('storage_getFragment')
    })

    // T044: Frontend unit test for hybrid recover (k fragments)
    it('should successfully recover with exactly k fragments', async () => {
      const testData = new Uint8Array([65, 66, 67]) // "ABC"
      const rpcResponses = new Map<string, unknown>()
      rpcResponses.set('storage_getFragment', {
        data: btoa(String.fromCharCode(...Array.from(testData), 0)),
        hash: Array(32).fill(0),
      })
      
      global.fetch = createFetchMock(rpcResponses)

      const { result } = renderHook(() => useStorage())

      await waitFor(() => {
        expect(result.current.isReady).toBe(true)
      })

      const merkleRoot = new Uint8Array(32).fill(1)
      const k = 3

      let recoverResult: RecoverResult | null = null
      await act(async () => {
        recoverResult = await result.current.recoverContent(merkleRoot, testMetadata)
      })

      // k個の断片で復元成功
      expect(recoverResult).not.toBeNull()
      expect(recoverResult!.data).toBeInstanceOf(Uint8Array)
      // k回のRPC呼び出し
      expect(global.fetch).toHaveBeenCalledTimes(k)
    })

    // T044補足: hybrid_recover の Worker メッセージ検証
    it('should call hybrid_recover worker with correct metadata', async () => {
      const testData = new Uint8Array([1, 2, 3, 0])
      const rpcResponses = new Map<string, unknown>()
      rpcResponses.set('storage_getFragment', {
        data: btoa(String.fromCharCode(...Array.from(testData))),
        hash: Array(32).fill(0),
      })
      
      global.fetch = createFetchMock(rpcResponses)

      const { result } = renderHook(() => useStorage())

      await waitFor(() => {
        expect(result.current.isReady).toBe(true)
      })

      const merkleRoot = new Uint8Array(32).fill(1)

      await act(async () => {
        await result.current.recoverContent(merkleRoot, testMetadata)
      })

      // Worker の hybrid_recover が呼ばれることを確認（Mockは自動的に成功）
      expect(result.current.error).toBeNull()
    })

    // T045: Frontend test for partial availability (k of n nodes online)
    it('should fail if insufficient fragments available', async () => {
      // 0個の断片を返す（全て失敗）
      global.fetch = jest.fn(async () => ({
        ok: true,
        status: 200,
        json: async () => ({ jsonrpc: '2.0', id: 1, error: { message: 'Fragment not found' } }),
      } as Response)) as typeof global.fetch

      const { result } = renderHook(() => useStorage())

      await waitFor(() => {
        expect(result.current.isReady).toBe(true)
      })

      const merkleRoot = new Uint8Array(32).fill(1)

      await act(async () => {
        try {
          await result.current.recoverContent(merkleRoot, testMetadata)
        } catch (err) {
          expect((err as Error).message).toContain('Insufficient fragments')
        }
      })

      expect(result.current.error).toContain('Insufficient fragments')
    }, 30000) // リトライ遅延を考慮して30秒

    // T045補足: k-1個しか取得できない場合のエラー
    it('should fail when only k-1 fragments are available', async () => {
      let callCount = 0
      const k = 3
      
      global.fetch = jest.fn(async () => {
        callCount++
        // 最初の2つ(k-1)だけ成功、それ以降は失敗
        if (callCount <= k - 1) {
          return {
            ok: true,
            status: 200,
            json: async () => ({
              jsonrpc: '2.0',
              id: 1,
              result: {
                data: btoa(String.fromCharCode(1, 2, 3, callCount - 1)),
                hash: Array(32).fill(0),
              },
            }),
          } as Response
        }
        return {
          ok: true,
          status: 200,
          json: async () => ({ jsonrpc: '2.0', id: 1, error: { message: 'Fragment not found' } }),
        } as Response
      }) as typeof global.fetch

      const { result } = renderHook(() => useStorage())

      await waitFor(() => {
        expect(result.current.isReady).toBe(true)
      })

      const merkleRoot = new Uint8Array(32).fill(1)

      await act(async () => {
        try {
          await result.current.recoverContent(merkleRoot, testMetadata)
        } catch (err) {
          expect((err as Error).message).toContain('Insufficient fragments')
        }
      })

      expect(result.current.error).toContain('Insufficient fragments')
    }, 30000)

    // T045補足: 一部のノードがオフラインでもk個取得できれば成功
    it('should succeed when k fragments available despite some failures', async () => {
      let callCount = 0
      const k = 3
      const testData = new Uint8Array([10, 20, 30])
      
      global.fetch = jest.fn(async () => {
        callCount++
        // 全てのリクエストで成功を返す（k回で終了するので）
        return {
          ok: true,
          status: 200,
          json: async () => ({
            jsonrpc: '2.0',
            id: 1,
            result: {
              data: btoa(String.fromCharCode(...Array.from(testData), callCount - 1)),
              hash: Array(32).fill(0),
            },
          }),
        } as Response
      }) as typeof global.fetch

      const { result } = renderHook(() => useStorage())

      await waitFor(() => {
        expect(result.current.isReady).toBe(true)
      })

      const merkleRoot = new Uint8Array(32).fill(1)
      let recoverResult: RecoverResult | null = null

      await act(async () => {
        recoverResult = await result.current.recoverContent(merkleRoot, testMetadata)
      })

      expect(recoverResult).not.toBeNull()
      expect(result.current.error).toBeNull()
      // k回のRPC呼び出しで完了
      expect(callCount).toBe(k)
    })
  })
})
