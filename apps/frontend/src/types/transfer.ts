/**
 * Transfer Types for MORAL token transfer
 * @module types/transfer
 */

/**
 * Transfer status states
 */
export type TransferStatus = 'idle' | 'confirming' | 'pending' | 'success' | 'error'

/**
 * Transfer request interface
 * Represents a MORAL token transfer request
 */
export interface TransferRequest {
  /** Sender AccountId (SS58 format) */
  sender: string
  /** Recipient AccountId (SS58 format) */
  recipient: string
  /** Amount in planck (1 MORAL = 1_000_000_000_000n) */
  amount: bigint
  /** Current transfer status */
  status: TransferStatus
  /** Transaction hash (set after submission) */
  txHash?: string
  /** Error message (set on failure) */
  error?: string
}

/**
 * Transfer state for useTransfer hook
 */
export interface TransferState {
  /** Current status */
  status: TransferStatus
  /** Recipient address (when confirming/pending) */
  recipient?: string
  /** Transfer amount (when confirming/pending) */
  amount?: bigint
  /** Transaction hash (when success) */
  txHash?: string
  /** Error message (when error) */
  error?: string
}

/**
 * Validation result for transfer inputs
 */
export interface ValidationResult {
  /** Whether validation passed */
  valid: boolean
  /** i18n error key if validation failed */
  error?: string
}

/**
 * MORAL token decimals (12)
 */
export const MORAL_DECIMALS = 12

/**
 * One MORAL in planck units
 */
export const ONE_MORAL = BigInt(10 ** MORAL_DECIMALS) // 1_000_000_000_000n

/**
 * Parse MORAL amount string to planck bigint
 * @param amount - Amount string (e.g., "1.5")
 * @returns Amount in planck
 */
export function parseMoralAmount(amount: string): bigint {
  const trimmed = amount.trim()
  if (!trimmed || trimmed === '') return BigInt(0)
  
  const parts = trimmed.split('.')
  if (parts.length > 2) return BigInt(0)
  
  const integerPart = parts[0] || '0'
  const decimalPart = (parts[1] || '').padEnd(MORAL_DECIMALS, '0').slice(0, MORAL_DECIMALS)
  
  try {
    const integer = BigInt(integerPart) * ONE_MORAL
    const decimal = BigInt(decimalPart)
    return integer + decimal
  } catch {
    return BigInt(0)
  }
}

/**
 * Format planck amount to MORAL string
 * @param planck - Amount in planck
 * @param decimals - Number of decimal places to show (default: 4)
 * @returns Formatted MORAL amount string
 */
export function formatMoralAmount(planck: bigint, decimals: number = 4): string {
  const integer = planck / ONE_MORAL
  const remainder = planck % ONE_MORAL
  const decimalStr = remainder.toString().padStart(MORAL_DECIMALS, '0').slice(0, decimals)
  
  // Remove trailing zeros
  const trimmed = decimalStr.replace(/0+$/, '')
  
  if (trimmed === '') {
    return integer.toString()
  }
  return `${integer}.${trimmed}`
}

/**
 * Format balance with MORAL suffix
 * @param balance - Balance in planck (or null if loading)
 * @returns Formatted balance string with "MORAL" suffix
 */
export function formatMoralBalance(balance: bigint | null): string {
  if (balance === null) return '-'
  return `${formatMoralAmount(balance, 2)} MORAL`
}
