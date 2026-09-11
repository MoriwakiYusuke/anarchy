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
jest.mock('@/lib/stealth/keyManager', () => ({ stealthKeyManager: { destroy: jest.fn() } }))
jest.mock('@/lib/dm/store', () => ({
  useDmStore: { getState: () => ({ resetForAccountChange: jest.fn() }) },
}))

const mockLoad = jest.fn()
const mockSave = jest.fn().mockResolvedValue(undefined)
const mockClear = jest.fn().mockResolvedValue(undefined)
jest.mock('@/lib/account/sessionStore', () => ({
  loadSession: () => mockLoad(),
  saveSession: (s: unknown) => mockSave(s),
  clearSession: () => mockClear(),
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
  mockSave.mockClear()
  mockClear.mockClear()
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
