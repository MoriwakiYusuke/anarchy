/**
 * sessionStore: ログイン状態 (アドレス + seed) の IndexedDB 永続化。
 *
 * リロードしても接続状態を復帰させるために seed を保存する。
 * これは元の "秘密鍵は session memory のみ" 原則からの意図的な変更 (CLAUDE.md
 * Security Principle #2 参照、2026-09-12 にプロジェクトオーナー判断で変更):
 * 同一 origin の XSS や端末を触れる相手には seed がそのまま渡るトレードオフを
 * 受け入れている。切断 (Disconnect) で必ず消す。
 */
import { createIdbKv } from '@/lib/idbKv'

export interface PersistedSession {
  /** SS58 アドレス */
  account: string
  /** WalletConnect が受け取った seed 文字列 (ニーモニック、dev ビルドでは //Alice 等も) */
  seed: string
}

const kv = createIdbKv<PersistedSession>('anarchy-auth', 'session')
const KEY = 'current'

export async function saveSession(session: PersistedSession): Promise<void> {
  await kv.put(KEY, session)
}

export async function loadSession(): Promise<PersistedSession | null> {
  const s = await kv.get(KEY)
  if (!s || typeof s.account !== 'string' || typeof s.seed !== 'string') return null
  return s
}

export async function clearSession(): Promise<void> {
  await kv.delete(KEY)
}
