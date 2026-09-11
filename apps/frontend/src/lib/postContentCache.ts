/**
 * postContentCache: 投稿本文 (復元済み生バイト列) の永続キャッシュ。
 *
 * - キーは Merkle root の hex。root は内容アドレスなので値は不変、TTL も無効化も不要。
 * - 値は decodePostContent 前の生バイト (テキスト + メディア込み)。読み出し側でデコードする。
 * - 2 層構成: モジュールスコープの Map (同一セッション内の再マウント用) → IndexedDB (リロード跨ぎ)。
 * - IDB が使えない環境 (プライベートモード等) では常にミス扱いにして呼び出し側を止めない。
 *
 * セキュリティ: 投稿は公開情報であり秘密鍵は一切含まないので、
 * "秘密鍵をブラウザストレージに残さない" 原則には抵触しない。
 */

const DB_NAME = 'anarchy-post-content'
const DB_VERSION = 1
const STORE_NAME = 'contents'

interface CachedRecord {
  data: Uint8Array
  storedAt: number
}

const memory = new Map<string, Uint8Array>()

let dbPromise: Promise<IDBDatabase | null> | null = null

function rootToKey(root: Uint8Array): string {
  return Array.from(root, (b) => b.toString(16).padStart(2, '0')).join('')
}

function openDb(): Promise<IDBDatabase | null> {
  if (dbPromise) return dbPromise
  dbPromise = new Promise((resolve) => {
    try {
      const factory = globalThis.indexedDB
      if (!factory) {
        resolve(null)
        return
      }
      const req = factory.open(DB_NAME, DB_VERSION)
      req.onupgradeneeded = () => {
        req.result.createObjectStore(STORE_NAME)
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

function idbGet(db: IDBDatabase, key: string): Promise<CachedRecord | undefined> {
  return new Promise((resolve) => {
    try {
      const tx = db.transaction(STORE_NAME, 'readonly')
      const req = tx.objectStore(STORE_NAME).get(key)
      req.onsuccess = () => resolve(req.result as CachedRecord | undefined)
      req.onerror = () => resolve(undefined)
    } catch {
      resolve(undefined)
    }
  })
}

function idbPut(db: IDBDatabase, key: string, record: CachedRecord): Promise<void> {
  return new Promise((resolve) => {
    try {
      const tx = db.transaction(STORE_NAME, 'readwrite')
      tx.objectStore(STORE_NAME).put(record, key)
      tx.oncomplete = () => resolve()
      tx.onerror = () => resolve()
      tx.onabort = () => resolve()
    } catch {
      resolve()
    }
  })
}

/** キャッシュ済みの投稿バイト列を返す。未登録 / IDB 不可なら null。 */
export async function getCachedContent(root: Uint8Array): Promise<Uint8Array | null> {
  const key = rootToKey(root)
  const hit = memory.get(key)
  if (hit) return hit

  const db = await openDb()
  if (!db) return null
  const record = await idbGet(db, key)
  if (!record) return null
  // structured clone 経由で Uint8Array は保たれるが、念のため正規化する
  const data = record.data instanceof Uint8Array ? record.data : new Uint8Array(record.data)
  memory.set(key, data)
  return data
}

/** 復元に成功した投稿バイト列を保存する。IDB 失敗時はメモリ層のみに残して黙って返る。 */
export async function putCachedContent(root: Uint8Array, data: Uint8Array): Promise<void> {
  const key = rootToKey(root)
  memory.set(key, data)

  const db = await openDb()
  if (!db) return
  await idbPut(db, key, { data, storedAt: Date.now() })
}
