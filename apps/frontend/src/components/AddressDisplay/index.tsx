'use client'

/**
 * AddressDisplay Component
 * 
 * T-033: AddressDisplay component for showing AccountId with copy functionality
 * 
 * Features:
 * - Short address format (FR-009)
 * - Click to copy (FR-010)
 * - Visual feedback on copy (FR-011)
 * - Tooltip with full address (FR-012)
 * - Nickname display (FR-013)
 */

import React, { useState, useCallback, useRef, useEffect } from 'react'
import { useLocale } from '@/i18n'
import { formatAddress, getDisplayName } from '@/types/address'
import { copyToClipboard } from '@/lib/clipboard'
import Tooltip from './Tooltip'
import styles from './AddressDisplay.module.css'

export interface AddressDisplayProps {
  /** Full AccountId (SS58 format) */
  address: string
  /** Optional nickname to display instead of short address */
  nickname?: string
  /** Show short address alongside nickname */
  showAddressWithNickname?: boolean
  /** Size variant */
  size?: 'compact' | 'default' | 'full'
  /** Custom class name */
  className?: string
  /** Callback when address is copied */
  onCopy?: (address: string) => void
}

/**
 * AddressDisplay component for showing AccountId with copy functionality
 */
export default function AddressDisplay({
  address,
  nickname,
  showAddressWithNickname = false,
  size = 'default',
  className,
  onCopy,
}: AddressDisplayProps) {
  const { t } = useLocale()
  const [copyStatus, setCopyStatus] = useState<'idle' | 'success' | 'error'>('idle')
  const [showTooltip, setShowTooltip] = useState(false)
  const timeoutRef = useRef<NodeJS.Timeout | null>(null)
  const containerRef = useRef<HTMLDivElement>(null)

  // Format address for display
  const displayData = formatAddress(address)
  const displayName = nickname ? nickname : getDisplayName(displayData)

  // Clear timeout on unmount
  useEffect(() => {
    return () => {
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current)
      }
    }
  }, [])

  // Handle copy to clipboard
  const handleCopy = useCallback(async () => {
    const result = await copyToClipboard(address)
    
    if (result.success) {
      setCopyStatus('success')
      onCopy?.(address)
    } else {
      setCopyStatus('error')
    }

    // Clear status after 2 seconds
    if (timeoutRef.current) {
      clearTimeout(timeoutRef.current)
    }
    timeoutRef.current = setTimeout(() => {
      setCopyStatus('idle')
    }, 2000)
  }, [address, onCopy])

  // Handle keyboard events
  const handleKeyDown = useCallback((event: React.KeyboardEvent) => {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault()
      handleCopy()
    }
  }, [handleCopy])

  // Combine class names
  const containerClassName = [
    styles.container,
    styles[size],
    copyStatus !== 'idle' && styles[copyStatus],
    className,
  ].filter(Boolean).join(' ')

  return (
    <div
      ref={containerRef}
      className={containerClassName}
      data-testid="address-display"
      role="button"
      tabIndex={0}
      onClick={handleCopy}
      onKeyDown={handleKeyDown}
      onMouseEnter={() => setShowTooltip(true)}
      onMouseLeave={() => setShowTooltip(false)}
      aria-label={`${displayName}. ${t('address.clickToCopy')}`}
    >
      {/* Main display */}
      <span className={styles.displayName}>
        {displayName}
      </span>

      {/* Short address if showing with nickname */}
      {nickname && showAddressWithNickname && (
        <span className={styles.shortAddress}>
          ({displayData.short})
        </span>
      )}

      {/* Copy status feedback */}
      {copyStatus === 'success' && (
        <span className={styles.feedback} role="status" aria-live="polite">
          {t('address.copied')}
        </span>
      )}
      {copyStatus === 'error' && (
        <span className={styles.feedback} role="status" aria-live="polite">
          {t('address.copyFailed')}
        </span>
      )}

      {/* Tooltip */}
      {showTooltip && copyStatus === 'idle' && (
        <Tooltip>
          <div className={styles.tooltipContent}>
            <div className={styles.fullAddress}>{address}</div>
            <div className={styles.hint}>{t('address.clickToCopy')}</div>
          </div>
        </Tooltip>
      )}
    </div>
  )
}
