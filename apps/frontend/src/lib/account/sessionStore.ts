/**
 * sessionStore: ログイン状態 (アドレス + seed) と DM/ステルス鍵の IndexedDB 永続化。
 *
 * リロードしても接続状態と DM 鍵を復帰させるために保存する。
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

/** DM/ステルス鍵の秘密鍵素材 (StealthKeyManager.loadFromBackup と同じ引数)。 */
export interface PersistedStealthKeys {
  /** X25519 view/scan 秘密鍵 (32 bytes) */
  scanPriv: Uint8Array
  /** Ed25519 spend seed (32 bytes) */
  spendPriv: Uint8Array
}

type AuthRecord = PersistedSession | PersistedStealthKeys

const kv = createIdbKv<AuthRecord>('anarchy-auth', 'session')
const KEY = 'current'
// ステルス鍵は account ごと (別アカウントの鍵を誤ってロードしない)
const stealthKey = (account: string) => `stealth:${account}`

export async function saveSession(session: PersistedSession): Promise<void> {
  await kv.put(KEY, session)
}

export async function loadSession(): Promise<PersistedSession | null> {
  const s = (await kv.get(KEY)) as Partial<PersistedSession> | undefined
  if (!s || typeof s.account !== 'string' || typeof s.seed !== 'string') return null
  return { account: s.account, seed: s.seed }
}

export async function clearSession(): Promise<void> {
  await kv.delete(KEY)
}

export async function saveStealthKeys(account: string, keys: PersistedStealthKeys): Promise<void> {
  await kv.put(stealthKey(account), { scanPriv: keys.scanPriv, spendPriv: keys.spendPriv })
}

export async function loadStealthKeys(account: string): Promise<PersistedStealthKeys | null> {
  const k = (await kv.get(stealthKey(account))) as Partial<PersistedStealthKeys> | undefined
  if (!k || !k.scanPriv || !k.spendPriv) return null
  const scanPriv = new Uint8Array(k.scanPriv)
  const spendPriv = new Uint8Array(k.spendPriv)
  if (scanPriv.length !== 32 || spendPriv.length !== 32) return null
  return { scanPriv, spendPriv }
}

export async function clearStealthKeys(account: string): Promise<void> {
  await kv.delete(stealthKey(account))
}

/** 切断: session と全 account のステルス鍵をまとめて消す。 */
export async function clearAllAuth(): Promise<void> {
  await kv.clear()
}
