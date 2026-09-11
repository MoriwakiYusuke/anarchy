/**
 * AccountProvider の永続化: マウント時に sessionStore から復帰し、
 * 接続で保存、切断でクリアする。
 */
import { render, screen, waitFor, act } from '@testing-library/react'
import '@testing-library/jest-dom'

// jest.setup.ts はグローバルに context をスタブしているので、ここでは本物を使う
jest.unmock('@/lib/account/context')

// useApi は polkadot-api (ESM) を引くのでスタブ。signer 導出の中身はこのテストの関心外。
jest.mock('@/hooks/useApi', () => ({
  useApi: () => ({ createSigner: jest.fn().mockResolvedValue({ publicKey: new Uint8Array(32) }) }),
}))
jest.mock('@polkadot/util-crypto', () => ({ cryptoWaitReady: jest.fn().mockResolvedValue(true) }))
jest.mock('@polkadot/keyring', () => ({
  Keyring: jest.fn().mockImplementation(() => ({
    addFromUri: () => ({ publicKey: new Uint8Array(32), sign: () => new Uint8Array(64) }),
  })),
}))
const mockStealth = {
  destroy: jest.fn(),
  bindAccount: jest.fn(),
  loadFromBackup: jest.fn().mockResolvedValue(undefined),
  hasKeys: jest.fn().mockReturnValue(false),
}
jest.mock('@/lib/stealth/keyManager', () => ({ stealthKeyManager: mockStealth }))
jest.mock('@/lib/dm/store', () => ({
  useDmStore: { getState: () => ({ resetForAccountChange: jest.fn() }) },
}))

const mockLoad = jest.fn()
const mockSave = jest.fn().mockResolvedValue(undefined)
const mockClear = jest.fn().mockResolvedValue(undefined)
const mockLoadStealth = jest.fn()
jest.mock('@/lib/account/sessionStore', () => ({
  loadSession: () => mockLoad(),
  saveSession: (s: unknown) => mockSave(s),
  clearAllAuth: () => mockClear(),
  loadStealthKeys: (a: string) => mockLoadStealth(a),
}))

// eslint-disable-next-line @typescript-eslint/no-var-requires
const { AccountProvider, useAccount } = require('@/lib/account/context') as typeof import('@/lib/account/context')

const ADDR = '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY'

function Probe() {
  const { account, isRestoring, setAccount } = useAccount()
  return (
    <div>
      <span data-testid="account">{account ?? 'none'}</span>
      <span data-testid="restoring">{String(isRestoring)}</span>
      <button onClick={() => setAccount(ADDR, 'seed words')}>connect</button>
      <button onClick={() => setAccount(null, null)}>disconnect</button>
    </div>
  )
}

beforeEach(() => {
  mockLoad.mockReset().mockResolvedValue(null)
  mockLoadStealth.mockReset().mockResolvedValue(null)
  mockSave.mockClear()
  mockClear.mockClear()
  mockStealth.destroy.mockClear()
  mockStealth.bindAccount.mockClear()
  mockStealth.loadFromBackup.mockClear()
  mockStealth.hasKeys.mockReturnValue(false)
})

describe('AccountProvider persistence', () => {
  test('保存済み session があればマウント時に復帰する', async () => {
    mockLoad.mockResolvedValue({ account: ADDR, seed: 'seed words' })
    render(<AccountProvider><Probe /></AccountProvider>)
    expect(screen.getByTestId('restoring')).toHaveTextContent('true')
    await waitFor(() => expect(screen.getByTestId('account')).toHaveTextContent(ADDR))
    expect(screen.getByTestId('restoring')).toHaveTextContent('false')
    // 復帰は保存し直さない
    expect(mockSave).not.toHaveBeenCalled()
  })

  test('保存が無ければ未接続のまま復帰処理だけ終わる', async () => {
    render(<AccountProvider><Probe /></AccountProvider>)
    await waitFor(() => expect(screen.getByTestId('restoring')).toHaveTextContent('false'))
    expect(screen.getByTestId('account')).toHaveTextContent('none')
  })

  test('接続で session を保存し、切断でクリアする', async () => {
    render(<AccountProvider><Probe /></AccountProvider>)
    await waitFor(() => expect(screen.getByTestId('restoring')).toHaveTextContent('false'))

    await act(async () => { screen.getByText('connect').click() })
    await waitFor(() => expect(mockSave).toHaveBeenCalledWith({ account: ADDR, seed: 'seed words' }))
    expect(screen.getByTestId('account')).toHaveTextContent(ADDR)

    await act(async () => { screen.getByText('disconnect').click() })
    await waitFor(() => expect(mockClear).toHaveBeenCalledTimes(1))
    expect(screen.getByTestId('account')).toHaveTextContent('none')
  })
})

describe('AccountProvider stealth key restore', () => {
  const KEYS = { scanPriv: new Uint8Array(32).fill(2), spendPriv: new Uint8Array(32).fill(1) }

  test('session 復帰時に account を bind し、保存済み DM 鍵を manager にロードする', async () => {
    mockLoad.mockResolvedValue({ account: ADDR, seed: 'seed words' })
    mockLoadStealth.mockResolvedValue(KEYS)
    render(<AccountProvider><Probe /></AccountProvider>)
    await waitFor(() => expect(mockStealth.loadFromBackup).toHaveBeenCalledWith(KEYS.scanPriv, KEYS.spendPriv))
    expect(mockLoadStealth).toHaveBeenCalledWith(ADDR)
    // bind は load より先 (loadFromBackup 内の persist が正しい account 名義になる)
    const bindOrder = mockStealth.bindAccount.mock.invocationCallOrder[0]
    const loadOrder = mockStealth.loadFromBackup.mock.invocationCallOrder[0]
    expect(mockStealth.bindAccount).toHaveBeenCalledWith(ADDR)
    expect(bindOrder).toBeLessThan(loadOrder)
  })

  test('保存済み DM 鍵が無ければロードしない', async () => {
    mockLoad.mockResolvedValue({ account: ADDR, seed: 'seed words' })
    render(<AccountProvider><Probe /></AccountProvider>)
    await waitFor(() => expect(mockStealth.bindAccount).toHaveBeenCalledWith(ADDR))
    await waitFor(() => expect(mockLoadStealth).toHaveBeenCalledWith(ADDR))
    expect(mockStealth.loadFromBackup).not.toHaveBeenCalled()
  })

  test('通常の接続でも bind + 保存済み鍵のロードが走る', async () => {
    mockLoadStealth.mockResolvedValue(KEYS)
    render(<AccountProvider><Probe /></AccountProvider>)
    await waitFor(() => expect(screen.getByTestId('restoring')).toHaveTextContent('false'))
    await act(async () => { screen.getByText('connect').click() })
    await waitFor(() => expect(mockStealth.loadFromBackup).toHaveBeenCalledWith(KEYS.scanPriv, KEYS.spendPriv))
  })

  test('切断で bind 解除 + 全 auth 情報クリア', async () => {
    mockLoad.mockResolvedValue({ account: ADDR, seed: 'seed words' })
    render(<AccountProvider><Probe /></AccountProvider>)
    await waitFor(() => expect(screen.getByTestId('account')).toHaveTextContent(ADDR))
    await act(async () => { screen.getByText('disconnect').click() })
    await waitFor(() => expect(mockClear).toHaveBeenCalledTimes(1))
    expect(mockStealth.bindAccount).toHaveBeenLastCalledWith(null)
    expect(mockStealth.destroy).toHaveBeenCalled()
  })
})
