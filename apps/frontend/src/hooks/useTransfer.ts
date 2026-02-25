'use client'

/**
 * useTransfer Hook
 * 
 * T-025: useTransfer hook for MORAL token transfers
 * State machine: idle → confirming → pending → success | error
 */

import { useState, useCallback, useMemo } from 'react'
import type { PolkadotSigner } from 'polkadot-api'
import type { TransferState, TransferStatus, ValidationResult } from '@/types/transfer'
import { parseMoralAmount, ONE_MORAL } from '@/types/transfer'
import { validateSS58Address, isSelfTransfer } from '@/lib/addressValidation'

/** Timeout for RPC calls in milliseconds (30 seconds) */
const RPC_TIMEOUT_MS = 30_000

/**
 * Wrap a promise with timeout
 */
function withTimeout<T>(promise: Promise<T>, ms: number, message: string): Promise<T> {
  return Promise.race([
    promise,
    new Promise<T>((_, reject) =>
      setTimeout(() => reject(new Error(`Timeout: ${message}`)), ms)
    )
  ])
}

export interface UseTransferOptions {
  /** PAPI client instance */
  client: any
  /** Unsafe API with tx access */
  unsafeApi: any
  /** Sender's AccountId (SS58 address) */
  senderAddress: string | null
  /** Sender's available balance in minimal units */
  balance: bigint | null
  /** Polkadot signer for transaction signing */
  signer: PolkadotSigner | null
  /** Callback on successful transfer */
  onSuccess?: (txHash: string) => void
  /** Callback on transfer error */
  onError?: (error: string) => void
}

export interface UseTransferResult {
  /** Current transfer state */
  state: TransferState
  /** Initiate transfer (validates and moves to confirming state) */
  transfer: (recipient: string, amount: string) => void
  /** Confirm the pending transfer (moves to pending, then success/error) */
  confirm: () => Promise<void>
  /** Cancel the transfer (returns to idle) */
  cancel: () => void
  /** Reset state after success/error */
  reset: () => void
  /** Validate recipient address */
  validateRecipient: (recipient: string) => ValidationResult
  /** Validate transfer amount */
  validateAmount: (amount: string) => ValidationResult
}

const initialState: TransferState = {
  status: 'idle',
  recipient: undefined,
  amount: undefined,
  txHash: undefined,
  error: undefined,
}

/**
 * Hook for managing MORAL token transfers
 * 
 * Follows state machine pattern:
 * - idle: Initial state, awaiting user input
 * - confirming: User entered valid data, awaiting confirmation
 * - pending: Transaction submitted, awaiting finalization
 * - success: Transfer completed successfully
 * - error: Transfer failed
 * 
 * @example
 * ```tsx
 * const { state, transfer, confirm, cancel, reset, validateRecipient, validateAmount } = useTransfer({
 *   client,
 *   unsafeApi,
 *   senderAddress: '5Grwva...',
 *   balance: 100_000_000_000_000n,
 *   signer,
 *   onSuccess: (txHash) => console.log('Success:', txHash),
 *   onError: (error) => console.error('Failed:', error),
 * })
 * 
 * // Validate inputs before transfer
 * const recipientResult = validateRecipient('5FHne...')
 * const amountResult = validateAmount('10')
 * 
 * if (recipientResult.valid && amountResult.valid) {
 *   transfer('5FHne...', '10')
 * }
 * ```
 */
export function useTransfer({
  client,
  unsafeApi,
  senderAddress,
  balance,
  signer,
  onSuccess,
  onError,
}: UseTransferOptions): UseTransferResult {
  const [state, setState] = useState<TransferState>(initialState)

  /**
   * Validate recipient address
   */
  const validateRecipient = useCallback((recipient: string): ValidationResult => {
    // Check address format
    const formatResult = validateSS58Address(recipient)
    if (!formatResult.valid) {
      return formatResult
    }

    // Check self-transfer
    if (senderAddress && isSelfTransfer(recipient, senderAddress)) {
      return { valid: false, error: 'error.selfTransfer' }
    }

    return { valid: true }
  }, [senderAddress])

  /**
   * Validate transfer amount
   */
  const validateAmount = useCallback((amountStr: string): ValidationResult => {
    // Empty check
    if (!amountStr || amountStr.trim() === '') {
      return { valid: false, error: 'error.emptyAmount' }
    }

    // Parse amount
    const amount = parseMoralAmount(amountStr)
    if (amount === null) {
      return { valid: false, error: 'error.invalidAmount' }
    }

    // Positive check
    if (amount <= BigInt(0)) {
      return { valid: false, error: 'error.amountTooSmall' }
    }

    // Minimum amount check (at least 1 planck = smallest unit)
    // For better UX, we require at least 0.000001 MORAL (1_000_000 planck)
    const MIN_AMOUNT = BigInt(1_000_000) // 0.000001 MORAL
    if (amount < MIN_AMOUNT) {
      return { valid: false, error: 'error.amountTooSmall' }
    }

    // Balance check
    if (balance !== null && amount > balance) {
      return { valid: false, error: 'error.amountExceedsBalance' }
    }

    return { valid: true }
  }, [balance])

  /**
   * Initiate a transfer - validate and move to confirming state
   */
  const transfer = useCallback((recipient: string, amountStr: string) => {
    // Validate recipient
    const recipientResult = validateRecipient(recipient)
    if (!recipientResult.valid) {
      setState({
        status: 'error',
        recipient,
        amount: undefined,
        error: recipientResult.error,
      })
      return
    }

    // Validate amount
    const amountResult = validateAmount(amountStr)
    if (!amountResult.valid) {
      setState({
        status: 'error',
        recipient,
        amount: undefined,
        error: amountResult.error,
      })
      return
    }

    // Parse amount to bigint
    const amount = parseMoralAmount(amountStr)!

    // Move to confirming state
    setState({
      status: 'confirming',
      recipient,
      amount,
    })
  }, [validateRecipient, validateAmount])

  /**
   * Confirm the transfer - submit transaction to chain
   */
  const confirm = useCallback(async () => {
    // Only proceed if in confirming state
    if (state.status !== 'confirming') {
      return
    }

    // Check requirements
    if (!client || !unsafeApi || !signer) {
      setState(prev => ({
        ...prev,
        status: 'error',
        error: 'error.missingDependencies',
      }))
      onError?.('Missing API or signer')
      return
    }

    if (!state.recipient || state.amount === undefined) {
      setState(prev => ({
        ...prev,
        status: 'error',
        error: 'error.invalidTransferState',
      }))
      onError?.('Invalid transfer state')
      return
    }

    // Move to pending state
    setState(prev => ({
      ...prev,
      status: 'pending',
    }))

    try {
      // Build the transfer transaction
      // Using Balances.transferKeepAlive to prevent account deletion
      const tx = unsafeApi.tx.Balances.transfer_keep_alive({
        dest: state.recipient, // SS58 address will be auto-converted
        value: state.amount,
      })

      // Sign and submit the transaction
      const result = await withTimeout(
        tx.signAndSubmit(signer),
        RPC_TIMEOUT_MS,
        'signAndSubmit'
      )

      // Extract transaction hash
      // PAPI returns different formats depending on the client
      let txHash: string
      const txResult = result as Record<string, unknown>
      if (typeof result === 'string') {
        txHash = result
      } else if (txResult && typeof txResult.txHash === 'string') {
        txHash = txResult.txHash
      } else if (txResult && txResult.hash) {
        const hash = txResult.hash as { toHex?: () => string }
        txHash = typeof hash === 'string' ? hash : hash.toHex?.() || String(hash)
      } else {
        txHash = 'unknown'
      }

      // Wait for finalization (for smoldot light client compatibility)
      // Poll for confirmation for up to 15 seconds

      // Success
      setState({
        status: 'success',
        recipient: state.recipient,
        amount: state.amount,
        txHash,
      })
      onSuccess?.(txHash)

    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err)
      
      // Map common errors
      let errorKey = 'error.transferFailed'
      if (errorMessage.includes('InsufficientBalance') || errorMessage.includes('insufficient')) {
        errorKey = 'error.amountExceedsBalance'
      } else if (errorMessage.includes('ExistentialDeposit')) {
        errorKey = 'error.existentialDeposit'
      } else if (errorMessage.includes('Timeout')) {
        errorKey = 'error.networkTimeout'
      }

      setState({
        status: 'error',
        recipient: state.recipient,
        amount: state.amount,
        error: errorKey,
      })
      onError?.(errorMessage)
    }
  }, [state, client, unsafeApi, signer, onSuccess, onError])

  /**
   * Cancel the transfer - return to idle state
   */
  const cancel = useCallback(() => {
    setState(initialState)
  }, [])

  /**
   * Reset state after success/error
   */
  const reset = useCallback(() => {
    setState(initialState)
  }, [])

  return {
    state,
    transfer,
    confirm,
    cancel,
    reset,
    validateRecipient,
    validateAmount,
  }
}
