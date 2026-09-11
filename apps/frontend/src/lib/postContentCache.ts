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

import { createIdbKv } from '@/lib/idbKv'

interface CachedRecord {
  data: Uint8Array
  storedAt: number
}

const memory = new Map<string, Uint8Array>()
const kv = createIdbKv<CachedRecord>('anarchy-post-content', 'contents')

function rootToKey(root: Uint8Array): string {
  return Array.from(root, (b) => b.toString(16).padStart(2, '0')).join('')
}

/** キャッシュ済みの投稿バイト列を返す。未登録 / IDB 不可なら null。 */
export async function getCachedContent(root: Uint8Array): Promise<Uint8Array | null> {
  const key = rootToKey(root)
  const hit = memory.get(key)
  if (hit) return hit

  const record = await kv.get(key)
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

  await kv.put(key, { data, storedAt: Date.now() })
}
