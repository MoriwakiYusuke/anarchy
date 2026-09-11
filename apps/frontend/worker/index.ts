/**
 * Anarchy フロントの Cloudflare Worker: 静的アセット (out/) の前段に Basic 認証を挟む。
 *
 * - wrangler.jsonc の `assets.run_worker_first: true` により、全リクエストがまずここを通る。
 * - 認証 OK なら `env.ASSETS.fetch(request)` に委譲 (Static Assets の通常配信)。
 * - 資格情報は wrangler secret (`BASIC_AUTH_USER` / `BASIC_AUTH_PASSWORD`)。
 *   **未設定なら fail-closed (503)**: 設定漏れでサイトが公開状態にならないようにする。
 * - 比較は定数時間 (長さ差も含めて XOR 集約) で行う。
 *
 * 守るのはこの静的フロントだけ。GCP 側の `wss://…/rpc` はここを通らないので別途。
 * `@cloudflare/workers-types` は入れず、Env は必要最小限を手書きしている
 * (Next の tsconfig が worker/ も拾うため、lib.dom の Request/Response で型付けする)。
 */

export interface Env {
  ASSETS: { fetch(request: Request): Promise<Response> }
  BASIC_AUTH_USER?: string
  BASIC_AUTH_PASSWORD?: string
}

const REALM = 'anarchy'

function constantTimeEqual(a: Uint8Array, b: Uint8Array): boolean {
  let diff = a.length ^ b.length
  const n = Math.max(a.length, b.length)
  for (let i = 0; i < n; i++) {
    diff |= (a[i] ?? 0) ^ (b[i] ?? 0)
  }
  return diff === 0
}

/** `Authorization` ヘッダが `Basic base64(user:pass)` で期待値と一致するか。 */
export function checkBasicAuth(
  authorization: string | null,
  expectedUser: string,
  expectedPassword: string,
): boolean {
  if (!authorization) return false
  const [scheme, encoded, ...rest] = authorization.split(' ')
  if (scheme !== 'Basic' || !encoded || rest.length > 0) return false

  let decoded: string
  try {
    decoded = atob(encoded)
  } catch {
    return false
  }
  const sep = decoded.indexOf(':')
  if (sep < 0) return false

  const enc = new TextEncoder()
  const userOk = constantTimeEqual(enc.encode(decoded.slice(0, sep)), enc.encode(expectedUser))
  const passOk = constantTimeEqual(enc.encode(decoded.slice(sep + 1)), enc.encode(expectedPassword))
  // 両方を必ず評価してから結合 (短絡でタイミング差を作らない)
  return userOk && passOk
}

export async function handleRequest(request: Request, env: Env): Promise<Response> {
  if (!env.BASIC_AUTH_USER || !env.BASIC_AUTH_PASSWORD) {
    return new Response('Basic auth is not configured', { status: 503 })
  }
  if (!checkBasicAuth(request.headers.get('Authorization'), env.BASIC_AUTH_USER, env.BASIC_AUTH_PASSWORD)) {
    return new Response('Unauthorized', {
      status: 401,
      headers: { 'WWW-Authenticate': `Basic realm="${REALM}", charset="UTF-8"` },
    })
  }
  return env.ASSETS.fetch(request)
}

export default {
  fetch: handleRequest,
}
