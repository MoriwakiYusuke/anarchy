/**
 * devAccounts: NEXT_PUBLIC_ENABLE_DEV_ACCOUNTS ビルドフラグ。
 * 本番ビルドでは無効 (未設定) で、//Alice 等の DEV_PHRASE 派生は一切行わない。
 */
import { isDevAccountsEnabled, resolveSeedUri } from '@/lib/devAccounts'

function setFlag(value: string | undefined) {
  if (value === undefined) delete process.env.NEXT_PUBLIC_ENABLE_DEV_ACCOUNTS
  else process.env.NEXT_PUBLIC_ENABLE_DEV_ACCOUNTS = value
}

afterEach(() => setFlag(undefined))

describe('isDevAccountsEnabled', () => {
  test('未設定なら無効', () => {
    setFlag(undefined)
    expect(isDevAccountsEnabled()).toBe(false)
  })

  test('"1" のときだけ有効', () => {
    setFlag('1')
    expect(isDevAccountsEnabled()).toBe(true)
    setFlag('true')
    expect(isDevAccountsEnabled()).toBe(false)
    setFlag('0')
    expect(isDevAccountsEnabled()).toBe(false)
  })
})

describe('resolveSeedUri', () => {
  const DEV_PHRASE = 'bottom drive obey lake curtain smoke basket hold race lonely fit walk'

  test('有効時: // 派生パスは DEV_PHRASE を前置する', () => {
    setFlag('1')
    expect(resolveSeedUri('//Alice')).toBe(`${DEV_PHRASE}//Alice`)
  })

  test('有効時: 通常のニーモニックはそのまま', () => {
    setFlag('1')
    expect(resolveSeedUri('word1 word2')).toBe('word1 word2')
  })

  test('無効時: // 派生パスでも DEV_PHRASE を前置しない', () => {
    setFlag(undefined)
    expect(resolveSeedUri('//Alice')).toBe('//Alice')
  })
})
