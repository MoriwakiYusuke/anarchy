/**
 * @jest-environment node
 *
 * Cloudflare Worker の Basic 認証ゲート。
 * Request / Response は Node 18+ の標準グローバルを使う (Workers ランタイムと同じ WHATWG API)。
 */
import { checkBasicAuth, handleRequest, type Env } from '../index'

const basic = (user: string, pass: string) => `Basic ${Buffer.from(`${user}:${pass}`).toString('base64')}`

describe('checkBasicAuth', () => {
  test('正しい user:pass なら true', () => {
    expect(checkBasicAuth(basic('anarchy', 's3cret'), 'anarchy', 's3cret')).toBe(true)
  })

  test('パスワードが違えば false', () => {
    expect(checkBasicAuth(basic('anarchy', 'wrong'), 'anarchy', 's3cret')).toBe(false)
  })

  test('ユーザー名が違えば false', () => {
    expect(checkBasicAuth(basic('other', 's3cret'), 'anarchy', 's3cret')).toBe(false)
  })

  test('パスワードに ":" を含んでも最初の ":" だけで分割する', () => {
    expect(checkBasicAuth(basic('anarchy', 'a:b:c'), 'anarchy', 'a:b:c')).toBe(true)
  })

  test('ヘッダ無し / Basic 以外 / 壊れた base64 は false', () => {
    expect(checkBasicAuth(null, 'anarchy', 's3cret')).toBe(false)
    expect(checkBasicAuth('Bearer abc', 'anarchy', 's3cret')).toBe(false)
    expect(checkBasicAuth('Basic !!!notbase64!!!', 'anarchy', 's3cret')).toBe(false)
    expect(checkBasicAuth('Basic ' + Buffer.from('nocolon').toString('base64'), 'anarchy', 's3cret')).toBe(false)
  })
})

describe('handleRequest', () => {
  const assetsFetch = jest.fn(async () => new Response('asset body', { status: 200 }))
  const env = (overrides: Partial<Env> = {}): Env => ({
    ASSETS: { fetch: assetsFetch },
    BASIC_AUTH_USER: 'anarchy',
    BASIC_AUTH_PASSWORD: 's3cret',
    ...overrides,
  })
  const req = (auth?: string) =>
    new Request('https://anarchy2026.org/', { headers: auth ? { Authorization: auth } : {} })

  beforeEach(() => assetsFetch.mockClear())

  test('secrets 未設定なら fail-closed で 503 (アセットを返さない)', async () => {
    const res = await handleRequest(req(basic('anarchy', 's3cret')), env({ BASIC_AUTH_USER: undefined }))
    expect(res.status).toBe(503)
    expect(assetsFetch).not.toHaveBeenCalled()
  })

  test('Authorization 無しは 401 + WWW-Authenticate', async () => {
    const res = await handleRequest(req(), env())
    expect(res.status).toBe(401)
    expect(res.headers.get('WWW-Authenticate')).toMatch(/^Basic realm=/)
    expect(assetsFetch).not.toHaveBeenCalled()
  })

  test('資格情報が違えば 401', async () => {
    const res = await handleRequest(req(basic('anarchy', 'nope')), env())
    expect(res.status).toBe(401)
    expect(assetsFetch).not.toHaveBeenCalled()
  })

  test('正しい資格情報なら ASSETS に委譲してそのレスポンスを返す', async () => {
    const request = req(basic('anarchy', 's3cret'))
    const res = await handleRequest(request, env())
    expect(assetsFetch).toHaveBeenCalledWith(request)
    expect(res.status).toBe(200)
    expect(await res.text()).toBe('asset body')
  })
})
