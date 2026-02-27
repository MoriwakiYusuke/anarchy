/**
 * Address Validation Helper
 * 
 * T-024: AccountId validation helper
 * SS58 address validation for Substrate/Polkadot based chains
 */

import { Binary } from 'polkadot-api'
import type { ValidationResult } from '@/types/transfer'

/**
 * SS58 alphabet used in address encoding
 * Excludes easily confused characters: 0, O, I, l
 */
const SS58_ALPHABET = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz'

/**
 * SS58 prefix for our chain (default Substrate = 42)
 * Can be customized based on chain configuration
 */
export const SS58_PREFIX = 42

/**
 * Expected length of encoded public key (32 bytes)
 */
export const PUBLIC_KEY_LENGTH = 32

/**
 * Valid SS58 address lengths for 32-byte public keys
 * 1 byte prefix + 32 bytes key + 2 bytes checksum = 47-48 characters when base58 encoded
 */
export const VALID_SS58_LENGTHS = [47, 48]

/**
 * Validate if a string is a valid SS58 address
 * 
 * @param address - The address string to validate
 * @returns ValidationResult with valid flag and optional error key
 * 
 * @example
 * ```ts
 * const result = validateSS58Address('5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY')
 * if (result.valid) {
 *   // Address is valid
 * } else {
 *   console.error(result.error) // 'error.invalidRecipient'
 * }
 * ```
 */
export function validateSS58Address(address: string): ValidationResult {
  // Empty check
  if (!address || address.trim() === '') {
    return { valid: false, error: 'error.emptyRecipient' }
  }

  const trimmed = address.trim()

  // Length check - SS58 addresses are typically 47-48 characters
  if (!VALID_SS58_LENGTHS.includes(trimmed.length)) {
    return { valid: false, error: 'error.invalidAddressLength' }
  }

  // Alphabet check - all characters must be in SS58 alphabet
  for (const char of trimmed) {
    if (!SS58_ALPHABET.includes(char)) {
      return { valid: false, error: 'error.invalidAddressCharacter' }
    }
  }

  // First character check - must start with valid SS58 prefix character
  // For prefix 42, valid starting characters are typically '5'
  // This is a simplification - full validation would decode and verify the prefix
  if (!['1', '2', '3', '4', '5', '6', '7', '8', '9'].includes(trimmed[0])) {
    return { valid: false, error: 'error.invalidAddressPrefix' }
  }

  // Try to decode using base58 logic
  try {
    // Simple base58 decode validation
    // Real implementation would verify checksum using blake2b
    const decoded = base58Decode(trimmed)
    
    // Must decode to at least prefix + public key + checksum
    // Minimum: 1 byte prefix + 32 bytes key + 2 bytes checksum = 35 bytes
    if (decoded.length < 35 || decoded.length > 36) {
      return { valid: false, error: 'error.invalidRecipient' }
    }

    return { valid: true }
  } catch (e) {
    return { valid: false, error: 'error.invalidRecipient' }
  }
}

/**
 * Check if an address is a self-transfer (same as sender)
 * 
 * @param recipient - Recipient address
 * @param sender - Sender address
 * @returns true if addresses are the same
 */
export function isSelfTransfer(recipient: string, sender: string): boolean {
  // SS58 addresses are case-sensitive (Base58 alphabet)
  // Compare exact trimmed strings without case folding
  return recipient.trim() === sender.trim()
}

/**
 * Validate recipient address with self-transfer check
 * 
 * @param recipient - Recipient address to validate
 * @param sender - Sender address to check against
 * @returns ValidationResult with valid flag and optional error key
 */
export function validateRecipient(
  recipient: string,
  sender: string
): ValidationResult {
  // First validate the address format
  const addressValidation = validateSS58Address(recipient)
  if (!addressValidation.valid) {
    return addressValidation
  }

  // Then check for self-transfer
  if (isSelfTransfer(recipient, sender)) {
    return { valid: false, error: 'error.selfTransfer' }
  }

  return { valid: true }
}

/**
 * Simple base58 decode implementation
 * Used for SS58 address validation
 * 
 * @param encoded - Base58 encoded string
 * @returns Decoded bytes
 */
function base58Decode(encoded: string): Uint8Array {
  const ALPHABET = SS58_ALPHABET
  const BASE = BigInt(58)
  const ZERO = BigInt(0)
  const BYTE_SIZE = BigInt(256)

  let value = ZERO
  for (const char of encoded) {
    const index = ALPHABET.indexOf(char)
    if (index === -1) {
      throw new Error('Invalid base58 character')
    }
    value = value * BASE + BigInt(index)
  }

  // Convert bigint to bytes
  const bytes: number[] = []
  while (value > ZERO) {
    bytes.unshift(Number(value % BYTE_SIZE))
    value = value / BYTE_SIZE
  }

  // Add leading zeros for leading '1's in input
  let leadingOnes = 0
  for (const char of encoded) {
    if (char === '1') {
      leadingOnes++
    } else {
      break
    }
  }

  const result = new Uint8Array(leadingOnes + bytes.length)
  result.set(bytes, leadingOnes)
  return result
}

/**
 * Extract public key bytes from SS58 address
 * 
 * @param address - Valid SS58 address
 * @returns 32-byte public key or undefined if invalid
 */
export function extractPublicKey(address: string): Uint8Array | undefined {
  try {
    const decoded = base58Decode(address.trim())
    
    // Skip the prefix byte(s) and checksum
    // For most addresses: 1 byte prefix + 32 bytes key + 2 bytes checksum
    if (decoded.length === 35) {
      return decoded.slice(1, 33) // Skip 1 prefix byte, take 32 key bytes
    }
    // Some addresses have 2-byte prefix
    if (decoded.length === 36) {
      return decoded.slice(2, 34) // Skip 2 prefix bytes, take 32 key bytes
    }
    
    return undefined
  } catch {
    return undefined
  }
}

/**
 * Format address for display
 * 
 * @param address - Full address
 * @param prefixLength - Characters to show at start (default: 6)
 * @param suffixLength - Characters to show at end (default: 6)
 * @returns Shortened address like "5Grwva...utQY"
 */
export function shortenAddress(
  address: string,
  prefixLength: number = 6,
  suffixLength: number = 6
): string {
  if (!address || address.length <= prefixLength + suffixLength + 3) {
    return address
  }
  return `${address.slice(0, prefixLength)}...${address.slice(-suffixLength)}`
}

/**
 * Compare two addresses for equality (case-insensitive trim)
 * 
 * @param a - First address
 * @param b - Second address
 * @returns true if addresses are equal
 */
export function addressEquals(a: string, b: string): boolean {
  return a.trim().toLowerCase() === b.trim().toLowerCase()
}
