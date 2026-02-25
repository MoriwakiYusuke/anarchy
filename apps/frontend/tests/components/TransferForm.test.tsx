/**
 * TransferForm Component Tests
 * 
 * T-023: TransferForm component tests
 * Test-First Development - tests written before implementation
 */

import React from 'react'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import '@testing-library/jest-dom'

// Mock the useTransfer hook
const mockTransfer = jest.fn()
const mockConfirm = jest.fn()
const mockCancel = jest.fn()
const mockReset = jest.fn()
const mockValidateRecipient = jest.fn()
const mockValidateAmount = jest.fn()

const defaultMockState = {
  status: 'idle' as const,
  recipient: undefined,
  amount: undefined,
  txHash: undefined,
  error: undefined,
}

const defaultMockHook = {
  state: defaultMockState,
  transfer: mockTransfer,
  confirm: mockConfirm,
  cancel: mockCancel,
  reset: mockReset,
  validateRecipient: mockValidateRecipient,
  validateAmount: mockValidateAmount,
}

jest.mock('@/hooks/useTransfer', () => ({
  useTransfer: jest.fn(() => defaultMockHook),
}))

// Mock i18n
jest.mock('@/i18n', () => ({
  useLocale: () => ({
    t: (key: string, params?: Record<string, string | number>) => {
      const translations: Record<string, string> = {
        'transfer.title': 'Transfer',
        'transfer.recipient': 'Recipient',
        'transfer.recipientPlaceholder': 'Enter AccountId...',
        'transfer.amount': 'Amount',
        'transfer.amountPlaceholder': '0.00',
        'transfer.send': 'Send',
        'transfer.sending': 'Sending...',
        'transfer.confirm': 'Confirm Transfer',
        'transfer.confirmMessage': `Send ${params?.amount} MORAL to ${params?.recipient}`,
        'transfer.cancel': 'Cancel',
        'transfer.success': 'Transfer complete!',
        'transfer.error': 'Transfer failed',
        'error.invalidRecipient': 'Invalid recipient address',
        'error.invalidAmount': 'Invalid amount',
        'error.amountTooSmall': 'Amount is too small',
        'error.amountExceedsBalance': 'Amount exceeds balance',
      }
      return translations[key] || key
    },
  }),
}))

// Import after mocks
import { TransferForm } from '@/components/TransferForm'

describe('TransferForm Component', () => {
  const mockSigner = {
    publicKey: new Uint8Array(32).fill(1),
    sign: jest.fn(),
  } as any

  const mockUnsafeApi = {} as any

  const defaultProps = {
    unsafeApi: mockUnsafeApi,
    signer: mockSigner,
    senderAddress: '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
    balance: BigInt(100_000_000_000_000), // 100 MORAL
  }

  beforeEach(() => {
    jest.clearAllMocks()
    mockValidateRecipient.mockReturnValue({ valid: true })
    mockValidateAmount.mockReturnValue({ valid: true })
    
    // Reset mock hook state
    const useTransferMock = require('@/hooks/useTransfer').useTransfer
    useTransferMock.mockReturnValue(defaultMockHook)
  })

  // ============================================================================
  // T-023a: Render Tests
  // ============================================================================

  describe('Render', () => {
    it('should render transfer form with title', () => {
      render(<TransferForm {...defaultProps} />)
      
      expect(screen.getByText('Transfer')).toBeInTheDocument()
    })

    it('should render recipient input field', () => {
      render(<TransferForm {...defaultProps} />)
      
      const recipientInput = screen.getByPlaceholderText('Enter AccountId...')
      expect(recipientInput).toBeInTheDocument()
    })

    it('should render amount input field', () => {
      render(<TransferForm {...defaultProps} />)
      
      const amountInput = screen.getByPlaceholderText('0.00')
      expect(amountInput).toBeInTheDocument()
    })

    it('should render send button', () => {
      render(<TransferForm {...defaultProps} />)
      
      expect(screen.getByRole('button', { name: 'Send' })).toBeInTheDocument()
    })
  })

  // ============================================================================
  // T-023b: User Input Tests
  // ============================================================================

  describe('User Input', () => {
    it('should update recipient value on input', async () => {
      render(<TransferForm {...defaultProps} />)
      
      const recipientInput = screen.getByPlaceholderText('Enter AccountId...')
      await userEvent.type(recipientInput, '5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty')
      
      expect(recipientInput).toHaveValue('5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty')
    })

    it('should update amount value on input', async () => {
      render(<TransferForm {...defaultProps} />)
      
      const amountInput = screen.getByPlaceholderText('0.00')
      await userEvent.type(amountInput, '10')
      
      expect(amountInput).toHaveValue('10')
    })

    it('should call transfer() when form is submitted with valid inputs', async () => {
      render(<TransferForm {...defaultProps} />)
      
      const recipientInput = screen.getByPlaceholderText('Enter AccountId...')
      const amountInput = screen.getByPlaceholderText('0.00')
      const sendButton = screen.getByRole('button', { name: 'Send' })
      
      await userEvent.type(recipientInput, '5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty')
      await userEvent.type(amountInput, '10')
      await userEvent.click(sendButton)
      
      expect(mockTransfer).toHaveBeenCalled()
    })
  })

  // ============================================================================
  // T-023c: Validation Display Tests
  // ============================================================================

  describe('Validation Display', () => {
    it('should show error message for invalid recipient', async () => {
      mockValidateRecipient.mockReturnValue({ valid: false, error: 'error.invalidRecipient' })
      
      render(<TransferForm {...defaultProps} />)
      
      const recipientInput = screen.getByPlaceholderText('Enter AccountId...')
      await userEvent.type(recipientInput, 'invalid')
      
      // Blur to trigger validation display
      fireEvent.blur(recipientInput)
      
      await waitFor(() => {
        expect(screen.getByText('Invalid recipient address')).toBeInTheDocument()
      })
    })

    it('should show error message for invalid amount', async () => {
      mockValidateAmount.mockReturnValue({ valid: false, error: 'error.amountExceedsBalance' })
      
      render(<TransferForm {...defaultProps} />)
      
      const amountInput = screen.getByPlaceholderText('0.00')
      await userEvent.type(amountInput, '999999')
      
      // Blur to trigger validation display
      fireEvent.blur(amountInput)
      
      await waitFor(() => {
        expect(screen.getByText('Amount exceeds balance')).toBeInTheDocument()
      })
    })

    it('should disable send button when inputs are invalid', async () => {
      mockValidateRecipient.mockReturnValue({ valid: false, error: 'error.invalidRecipient' })
      
      render(<TransferForm {...defaultProps} />)
      
      const recipientInput = screen.getByPlaceholderText('Enter AccountId...')
      await userEvent.type(recipientInput, 'invalid')
      
      const sendButton = screen.getByRole('button', { name: 'Send' })
      expect(sendButton).toBeDisabled()
    })
  })

  // ============================================================================
  // T-023d: Confirmation Dialog Tests
  // ============================================================================

  describe('Confirmation Dialog', () => {
    it('should show confirmation dialog when state is confirming', () => {
      const useTransferMock = require('@/hooks/useTransfer').useTransfer
      useTransferMock.mockReturnValue({
        ...defaultMockHook,
        state: {
          status: 'confirming',
          recipient: '5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty',
          amount: BigInt(10_000_000_000_000),
        },
      })
      
      render(<TransferForm {...defaultProps} />)
      
      expect(screen.getByText('Confirm Transfer')).toBeInTheDocument()
    })

    it('should call confirm() when confirm button is clicked', async () => {
      const useTransferMock = require('@/hooks/useTransfer').useTransfer
      useTransferMock.mockReturnValue({
        ...defaultMockHook,
        state: {
          status: 'confirming',
          recipient: '5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty',
          amount: BigInt(10_000_000_000_000),
        },
      })
      
      render(<TransferForm {...defaultProps} />)
      
      const confirmButton = screen.getByRole('button', { name: 'Confirm Transfer' })
      await userEvent.click(confirmButton)
      
      expect(mockConfirm).toHaveBeenCalled()
    })

    it('should call cancel() when cancel button is clicked', async () => {
      const useTransferMock = require('@/hooks/useTransfer').useTransfer
      useTransferMock.mockReturnValue({
        ...defaultMockHook,
        state: {
          status: 'confirming',
          recipient: '5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty',
          amount: BigInt(10_000_000_000_000),
        },
      })
      
      render(<TransferForm {...defaultProps} />)
      
      const cancelButton = screen.getByRole('button', { name: 'Cancel' })
      await userEvent.click(cancelButton)
      
      expect(mockCancel).toHaveBeenCalled()
    })
  })

  // ============================================================================
  // T-023e: Status Display Tests
  // ============================================================================

  describe('Status Display', () => {
    it('should show loading state when pending', () => {
      const useTransferMock = require('@/hooks/useTransfer').useTransfer
      useTransferMock.mockReturnValue({
        ...defaultMockHook,
        state: { status: 'pending' },
      })
      
      render(<TransferForm {...defaultProps} />)
      
      expect(screen.getByText('Sending...')).toBeInTheDocument()
    })

    it('should show success message when transfer succeeds', () => {
      const useTransferMock = require('@/hooks/useTransfer').useTransfer
      useTransferMock.mockReturnValue({
        ...defaultMockHook,
        state: { status: 'success', txHash: '0x1234' },
      })
      
      render(<TransferForm {...defaultProps} />)
      
      expect(screen.getByText('Transfer complete!')).toBeInTheDocument()
    })

    it('should show error message when transfer fails', () => {
      const useTransferMock = require('@/hooks/useTransfer').useTransfer
      useTransferMock.mockReturnValue({
        ...defaultMockHook,
        state: { status: 'error', error: 'Something went wrong' },
      })
      
      render(<TransferForm {...defaultProps} />)
      
      expect(screen.getByText('Transfer failed')).toBeInTheDocument()
    })
  })

  // ============================================================================
  // T-023f: Accessibility Tests
  // ============================================================================

  describe('Accessibility', () => {
    it('should have accessible labels for inputs', () => {
      render(<TransferForm {...defaultProps} />)
      
      expect(screen.getByLabelText(/recipient/i)).toBeInTheDocument()
      expect(screen.getByLabelText(/amount/i)).toBeInTheDocument()
    })

    it('should disable form during pending state', () => {
      const useTransferMock = require('@/hooks/useTransfer').useTransfer
      useTransferMock.mockReturnValue({
        ...defaultMockHook,
        state: { status: 'pending' },
      })
      
      render(<TransferForm {...defaultProps} />)
      
      const recipientInput = screen.getByPlaceholderText('Enter AccountId...')
      const amountInput = screen.getByPlaceholderText('0.00')
      
      expect(recipientInput).toBeDisabled()
      expect(amountInput).toBeDisabled()
    })
  })
})
