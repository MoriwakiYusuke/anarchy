/**
 * FaucetButton Component Tests
 * 
 * T-101: Faucetボタンが残高表示の下に表示される
 * T-106: 計算中はローディング状態が表示される
 */

import React from 'react'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'
import { FaucetButton } from '@/components/FaucetButton'

// Mock the useFaucet hook
const mockStartMining = jest.fn()
const mockCancel = jest.fn()

const defaultMockHook = {
  status: 'idle' as const,
  error: null,
  progress: null,
  startMining: mockStartMining,
  cancel: mockCancel,
}

jest.mock('@/hooks/useFaucet', () => ({
  useFaucet: jest.fn(() => defaultMockHook),
}))

// Mock i18n
jest.mock('@/i18n', () => ({
  useLocale: () => ({
    t: (key: string) => {
      const translations: Record<string, string> = {
        'faucet.button': 'Faucet',
        'faucet.mining': 'Mining...',
        'faucet.submitting': 'Submitting...',
        'faucet.success': 'Received 100 MORAL!',
        'faucet.error': 'Error',
        'faucet.cancel': 'Cancel',
        'error.alreadyClaimed': 'Already claimed',
        'error.challengeExpired': 'Challenge expired',
        'error.invalidProof': 'Invalid proof',
        'error.blockNotFound': 'Block not found',
        'error.insufficientBalance': 'Insufficient balance',
        'error.faucetNetworkError': 'Network error',
      }
      return translations[key] || key
    },
  }),
}))

// Mock polkadot-api signer
const mockSigner = {
  sign: jest.fn(),
} as any

const mockUnsafeApi = {
  query: {
    Faucet: {
      TotalClaims: { getValue: jest.fn().mockResolvedValue(BigInt(0)) },
    },
    System: {
      Number: { getValue: jest.fn().mockResolvedValue(100) },
    },
  },
  constants: {
    Faucet: {
      BaseDifficulty: jest.fn().mockReturnValue(18),
      DifficultyScalingFactor: jest.fn().mockReturnValue(BigInt(1000)),
      MaxDifficulty: jest.fn().mockReturnValue(28),
    },
  },
  tx: {
    Faucet: {
      claim: jest.fn().mockReturnValue({
        signAndSubmit: jest.fn().mockResolvedValue({}),
      }),
    },
  },
}

const mockClient = {
  _request: jest.fn().mockResolvedValue('0x' + '00'.repeat(32)),
}

describe('FaucetButton', () => {
  beforeEach(() => {
    jest.clearAllMocks()
    // Reset mock hook state
    const useFaucetMock = require('@/hooks/useFaucet').useFaucet
    useFaucetMock.mockReturnValue(defaultMockHook)
  })

  describe('T-101: Button Display', () => {
    it('should render Faucet button', () => {
      render(
        <FaucetButton
          client={mockClient}
          unsafeApi={mockUnsafeApi}
          account="5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY"
          signer={mockSigner}
        />
      )

      expect(screen.getByRole('button')).toBeInTheDocument()
      expect(screen.getByText('Faucet')).toBeInTheDocument()
    })

    it('should be disabled when no account is connected', () => {
      render(
        <FaucetButton
          client={mockClient}
          unsafeApi={mockUnsafeApi}
          account={null}
          signer={mockSigner}
        />
      )

      expect(screen.getByRole('button')).toBeDisabled()
    })

    it('should be disabled when no signer is available', () => {
      render(
        <FaucetButton
          client={mockClient}
          unsafeApi={mockUnsafeApi}
          account="5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY"
          signer={null}
        />
      )

      expect(screen.getByRole('button')).toBeDisabled()
    })

    it('should be enabled when account and signer are available', () => {
      render(
        <FaucetButton
          client={mockClient}
          unsafeApi={mockUnsafeApi}
          account="5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY"
          signer={mockSigner}
        />
      )

      expect(screen.getByRole('button')).not.toBeDisabled()
    })
  })

  describe('T-106: Loading State', () => {
    it('should show mining status during PoW calculation', () => {
      const useFaucetMock = require('@/hooks/useFaucet').useFaucet
      useFaucetMock.mockReturnValue({
        ...defaultMockHook,
        status: 'mining',
        progress: { hashRate: 50000, elapsed: 1500, currentNonce: BigInt(75000) },
      })

      render(
        <FaucetButton
          client={mockClient}
          unsafeApi={mockUnsafeApi}
          account="5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY"
          signer={mockSigner}
        />
      )

      expect(screen.getByText(/Mining.../)).toBeInTheDocument()
    })

    it('should show submitting status when sending transaction', () => {
      const useFaucetMock = require('@/hooks/useFaucet').useFaucet
      useFaucetMock.mockReturnValue({
        ...defaultMockHook,
        status: 'submitting',
      })

      render(
        <FaucetButton
          client={mockClient}
          unsafeApi={mockUnsafeApi}
          account="5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY"
          signer={mockSigner}
        />
      )

      expect(screen.getByText('Submitting...')).toBeInTheDocument()
    })

    it('should show spinner during processing', () => {
      const useFaucetMock = require('@/hooks/useFaucet').useFaucet
      useFaucetMock.mockReturnValue({
        ...defaultMockHook,
        status: 'mining',
      })

      render(
        <FaucetButton
          client={mockClient}
          unsafeApi={mockUnsafeApi}
          account="5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY"
          signer={mockSigner}
        />
      )

      // Button should have processing class for spinner
      const button = screen.getByRole('button')
      expect(button.querySelector('.spinner')).toBeTruthy()
    })
  })

  describe('Success and Error States', () => {
    it('should show success message after successful claim', () => {
      const useFaucetMock = require('@/hooks/useFaucet').useFaucet
      useFaucetMock.mockReturnValue({
        ...defaultMockHook,
        status: 'success',
      })

      render(
        <FaucetButton
          client={mockClient}
          unsafeApi={mockUnsafeApi}
          account="5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY"
          signer={mockSigner}
        />
      )

      expect(screen.getByText('Received 100 MORAL!')).toBeInTheDocument()
    })

    it('should show error message on failure', () => {
      const useFaucetMock = require('@/hooks/useFaucet').useFaucet
      useFaucetMock.mockReturnValue({
        ...defaultMockHook,
        status: 'error',
        error: { code: 'AlreadyClaimed', message: 'Already claimed' },
      })

      render(
        <FaucetButton
          client={mockClient}
          unsafeApi={mockUnsafeApi}
          account="5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY"
          signer={mockSigner}
        />
      )

      expect(screen.getByText('Already claimed')).toBeInTheDocument()
    })
  })

  describe('Click Handling', () => {
    it('should call startMining when clicked in idle state', () => {
      render(
        <FaucetButton
          client={mockClient}
          unsafeApi={mockUnsafeApi}
          account="5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY"
          signer={mockSigner}
        />
      )

      fireEvent.click(screen.getByRole('button'))
      expect(mockStartMining).toHaveBeenCalled()
    })

    it('should call cancel when clicked during processing', () => {
      const useFaucetMock = require('@/hooks/useFaucet').useFaucet
      useFaucetMock.mockReturnValue({
        ...defaultMockHook,
        status: 'mining',
      })

      render(
        <FaucetButton
          client={mockClient}
          unsafeApi={mockUnsafeApi}
          account="5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY"
          signer={mockSigner}
        />
      )

      fireEvent.click(screen.getByRole('button'))
      expect(mockCancel).toHaveBeenCalled()
    })

    it('should ignore rapid clicks (debounce)', () => {
      render(
        <FaucetButton
          client={mockClient}
          unsafeApi={mockUnsafeApi}
          account="5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY"
          signer={mockSigner}
        />
      )

      const button = screen.getByRole('button')
      
      // Rapid clicks should only trigger once due to cooldown
      fireEvent.click(button)
      fireEvent.click(button)
      fireEvent.click(button)
      
      // Only the first click should register
      expect(mockStartMining).toHaveBeenCalledTimes(1)
    })
  })
})
