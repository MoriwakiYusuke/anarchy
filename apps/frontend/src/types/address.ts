/**
 * Address Display Types
 * @module types/address
 */

/**
 * Address display data structure
 * Used for showing AccountId with optional nickname
 */
export interface AddressDisplay {
  /** Full AccountId (SS58 format) */
  full: string
  /** Shortened display format: {first 6}...{last 4} */
  short: string
  /** Optional nickname from on-chain registry */
  nickname?: string
}

/**
 * Format AccountId for display
 * @param accountId - Full AccountId in SS58 format
 * @returns AddressDisplay object
 */
export function formatAddress(accountId: string): AddressDisplay {
  if (!accountId || accountId.length < 10) {
    return {
      full: accountId || '',
      short: accountId || '',
      nickname: undefined,
    }
  }
  
  return {
    full: accountId,
    short: `${accountId.slice(0, 6)}...${accountId.slice(-4)}`,
    nickname: undefined,
  }
}

/**
 * Get display name for an address
 * Returns nickname if available, otherwise short format
 * @param display - AddressDisplay object
 * @returns Display name string
 */
export function getDisplayName(display: AddressDisplay): string {
  return display.nickname || display.short
}

/**
 * Check if two addresses are equal (case-sensitive)
 * @param a - First address
 * @param b - Second address
 * @returns true if addresses match
 */
export function addressEquals(a: string, b: string): boolean {
  return a === b
}
