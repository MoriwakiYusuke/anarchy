'use client'

import {
  createContext,
  useContext,
  useEffect,
  useState,
  type ReactNode,
} from 'react'
import { WorkerPool, type WorkerPoolConfig } from '@/workers/WorkerPool'

/**
 * WorkerPool Context
 * 
 * アプリ全体でWorkerPoolを共有するためのReact Context
 * Issue 12修正の一部: 各PostItemが独自のWorkerを作成する代わりに共有プールを使用
 */

interface WorkerPoolContextValue {
  /** 共有WorkerPool */
  pool: WorkerPool | null;
  /** 準備完了フラグ */
  isReady: boolean;
  /** タスク実行 */
  execute: <T = unknown>(type: string, payload: unknown) => Promise<T>;
}

const WorkerPoolContext = createContext<WorkerPoolContextValue | null>(null)

interface WorkerPoolProviderProps {
  children: ReactNode;
  config?: WorkerPoolConfig;
}

/**
 * WorkerPool Provider
 * 
 * アプリのルートで使用し、全子コンポーネントでWorkerPoolを共有
 * 
 * @example
 * ```tsx
 * // app/layout.tsx
 * export default function RootLayout({ children }) {
 *   return (
 *     <WorkerPoolProvider>
 *       {children}
 *     </WorkerPoolProvider>
 *   )
 * }
 * ```
 */
export function WorkerPoolProvider({ children, config }: WorkerPoolProviderProps) {
  const [pool, setPool] = useState<WorkerPool | null>(null)
  const [isReady, setIsReady] = useState(false)

  useEffect(() => {
    // SSR環境ではスキップ
    if (typeof window === 'undefined') {
      return
    }

    const workerPool = new WorkerPool(config)
    setPool(workerPool)

    // 準備完了を待機
    workerPool.waitUntilReady().then(() => {
      setIsReady(true)
    })

    return () => {
      workerPool.terminate()
    }
  }, [config])

  const execute = async <T = unknown,>(type: string, payload: unknown): Promise<T> => {
    if (!pool) {
      throw new Error('WorkerPool not initialized')
    }
    return pool.execute<T>(type, payload)
  }

  return (
    <WorkerPoolContext.Provider value={{ pool, isReady, execute }}>
      {children}
    </WorkerPoolContext.Provider>
  )
}

/**
 * useWorkerPool hook
 * 
 * WorkerPoolContextから共有プールを取得
 * 
 * @throws WorkerPoolProviderでラップされていない場合
 * 
 * @example
 * ```tsx
 * function MyComponent() {
 *   const { execute, isReady } = useWorkerPool()
 *   
 *   const handleCrypto = async () => {
 *     if (!isReady) return
 *     const result = await execute('hybrid_split', { data, k: 3, n: 5 })
 *   }
 * }
 * ```
 */
export function useWorkerPool(): WorkerPoolContextValue {
  const context = useContext(WorkerPoolContext)
  if (!context) {
    throw new Error('useWorkerPool must be used within WorkerPoolProvider')
  }
  return context
}

export { WorkerPoolContext }
