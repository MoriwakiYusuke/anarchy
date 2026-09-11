/**
 * idbKv: IndexedDB を「1 DB = 1 object store の key-value」として使う最小ヘルパ。
 *
 * - open は 1 回だけ (Promise をキャッシュ)。失敗 / 未対応環境では null を返し、
 *   get は undefined、put / delete は何もしない。呼び出し側を止めない方針。
 * - 利用者: lib/postContentCache (投稿本文), lib/account/sessionStore (ログイン状態)。
 */

export interface IdbKv<T> {
  get(key: string): Promise<T | undefined>
  put(key: string, value: T): Promise<void>
  delete(key: string): Promise<void>
  /** store 内の全レコードを消す */
  clear(): Promise<void>
}

export function createIdbKv<T>(dbName: string, storeName: string, version = 1): IdbKv<T> {
  let dbPromise: Promise<IDBDatabase | null> | null = null

  const open = (): Promise<IDBDatabase | null> => {
    if (dbPromise) return dbPromise
    dbPromise = new Promise((resolve) => {
      try {
        const factory = globalThis.indexedDB
        if (!factory) {
          resolve(null)
          return
        }
        const req = factory.open(dbName, version)
        req.onupgradeneeded = () => {
          req.result.createObjectStore(storeName)
        }
        req.onsuccess = () => resolve(req.result)
        req.onerror = () => resolve(null)
        req.onblocked = () => resolve(null)
      } catch {
        resolve(null)
      }
    })
    return dbPromise
  }

  const write = async (fn: (store: IDBObjectStore) => void): Promise<void> => {
    const db = await open()
    if (!db) return
    return new Promise((resolve) => {
      try {
        const tx = db.transaction(storeName, 'readwrite')
        fn(tx.objectStore(storeName))
        tx.oncomplete = () => resolve()
        tx.onerror = () => resolve()
        tx.onabort = () => resolve()
      } catch {
        resolve()
      }
    })
  }

  return {
    async get(key) {
      const db = await open()
      if (!db) return undefined
      return new Promise((resolve) => {
        try {
          const req = db.transaction(storeName, 'readonly').objectStore(storeName).get(key)
          req.onsuccess = () => resolve(req.result as T | undefined)
          req.onerror = () => resolve(undefined)
        } catch {
          resolve(undefined)
        }
      })
    },
    put: (key, value) => write((store) => store.put(value, key)),
    delete: (key) => write((store) => store.delete(key)),
    clear: () => write((store) => store.clear()),
  }
}
