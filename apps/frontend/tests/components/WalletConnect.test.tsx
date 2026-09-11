/**
 * WalletConnect: dev アカウントログインの入口は NEXT_PUBLIC_ENABLE_DEV_ACCOUNTS=1 の
 * ビルドでしか描画されない。
 */
import { render, screen } from '@testing-library/react'
import '@testing-library/jest-dom'

jest.mock('@/components/WalletConnect.module.css', () => new Proxy({}, { get: (_, k) => String(k) }))
jest.mock('@/components/FaucetButton', () => ({ FaucetButton: () => null }))
jest.mock('@/hooks/useMoralBalance', () => ({
  useMoralBalance: () => ({ balance: null, isLoading: false, refetch: jest.fn() }),
  formatMoralBalance: (v: unknown) => String(v),
}))

import { WalletConnect } from '@/components/WalletConnect'

// WalletConnect は process.env をレンダー時にインライン参照する (本番ビルドで定数畳み込みされる
// ようにするため)。テストでは env を切り替えて両ビルドを再現する。
function renderWithFlag(enabled: boolean) {
  if (enabled) process.env.NEXT_PUBLIC_ENABLE_DEV_ACCOUNTS = '1'
  else delete process.env.NEXT_PUBLIC_ENABLE_DEV_ACCOUNTS
  return render(<WalletConnect client={null} unsafeApi={null} />)
}

afterEach(() => {
  delete process.env.NEXT_PUBLIC_ENABLE_DEV_ACCOUNTS
})

describe('WalletConnect dev accounts gate', () => {
  test('フラグ未設定: dev タブが無く、シードフレーズ入力が最初から表示される', () => {
    renderWithFlag(false)
    expect(screen.queryByRole('button', { name: '開発用' })).not.toBeInTheDocument()
    expect(screen.queryByText('開発用テストアカウント')).not.toBeInTheDocument()
    expect(screen.getByPlaceholderText('word1 word2 word3 ... word12')).toBeInTheDocument()
  })

  test('フラグ=1: dev タブが表示され、初期タブは dev', () => {
    renderWithFlag(true)
    expect(screen.getByRole('button', { name: '開発用' })).toBeInTheDocument()
    expect(screen.getByText('開発用テストアカウント')).toBeInTheDocument()
  })
})
