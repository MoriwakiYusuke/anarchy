'use client'

/**
 * TransferForm Component
 * 
 * T-026: TransferForm component for MORAL token transfers
 * Handles user input, validation display, confirmation dialog, and status
 */

import { useState, useCallback, useMemo, useEffect } from 'react'
import { createPortal } from 'react-dom'
import { PolkadotSigner } from 'polkadot-api'
import { useTransfer } from '@/hooks/useTransfer'
import { useLocale, type TranslationKey } from '@/i18n'
import { formatMoralAmount, formatMoralBalance } from '@/types/transfer'
import { shortenAddress } from '@/lib/addressValidation'
import { StealthModal } from './stealth/StealthModal'
import styles from './TransferForm.module.css'

interface Props {
  /** PAPI client instance */
  client: any
  /** Unsafe API with tx access */
  unsafeApi: any
  /** Sender's AccountId (SS58 address) */
  senderAddress: string
  /** Sender's available balance in minimal units */
  balance: bigint
  /** Polkadot signer for transaction signing */
  signer: PolkadotSigner | null
  /** Callback on successful transfer */
  onSuccess?: () => void
}

export function TransferForm({
  client,
  unsafeApi,
  senderAddress,
  balance,
  signer,
  onSuccess,
}: Props) {
  const { t } = useLocale()
  const [recipient, setRecipient] = useState('')
  const [amount, setAmount] = useState('')
  const [recipientTouched, setRecipientTouched] = useState(false)
  const [amountTouched, setAmountTouched] = useState(false)
  const [mounted, setMounted] = useState(false)
  const [isStealthModalOpen, setStealthModalOpen] = useState(false)
  
  // For portal rendering
  useEffect(() => {
    setMounted(true)
  }, [])

  const {
    state,
    transfer,
    confirm,
    cancel,
    reset,
    validateRecipient,
    validateAmount,
  } = useTransfer({
    client,
    unsafeApi,
    senderAddress,
    balance,
    signer,
    onSuccess: (txHash) => {
      onSuccess?.()
      // Auto-reset after 5 seconds on success
      setTimeout(() => {
        reset()
        setRecipient('')
        setAmount('')
        setRecipientTouched(false)
        setAmountTouched(false)
      }, 5000)
    },
    onError: (error) => {
      console.error('[TransferForm] Transfer error:', error)
    },
  })

  // Validate inputs for display
  const recipientValidation = useMemo(
    () => (recipientTouched && recipient ? validateRecipient(recipient) : { valid: true }),
    [recipient, recipientTouched, validateRecipient]
  )

  const amountValidation = useMemo(
    () => (amountTouched && amount ? validateAmount(amount) : { valid: true }),
    [amount, amountTouched, validateAmount]
  )

  // Form validity
  const isFormValid = useMemo(() => {
    if (!recipient || !amount) return false
    return validateRecipient(recipient).valid && validateAmount(amount).valid
  }, [recipient, amount, validateRecipient, validateAmount])

  // Handle form submission
  const handleSubmit = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault()
      if (!isFormValid) return
      transfer(recipient, amount)
    },
    [isFormValid, recipient, amount, transfer]
  )

  // Handle confirmation
  const handleConfirm = useCallback(async () => {
    await confirm()
  }, [confirm])

  // Handle cancel
  const handleCancel = useCallback(() => {
    cancel()
    setRecipient('')
    setAmount('')
    setRecipientTouched(false)
    setAmountTouched(false)
  }, [cancel])

  // Format amount for display
  const formattedAmount = state.amount
    ? formatMoralAmount(state.amount)
    : amount

  // Determine if form should be disabled
  const isDisabled =
    !signer || state.status === 'pending' || state.status === 'confirming'

  return (
    <div className={styles.form}>
      <form onSubmit={handleSubmit}>
        {/* Recipient Input */}
        <div className={styles.fieldGroup}>
          <label htmlFor="transfer-recipient" className={styles.label}>
            {t('transfer.recipient')}
          </label>
          <input
            id="transfer-recipient"
            type="text"
            className={`${styles.input} ${
              !recipientValidation.valid ? styles.error : ''
            }`}
            placeholder={t('transfer.recipientPlaceholder')}
            value={recipient}
            onChange={(e) => setRecipient(e.target.value)}
            onBlur={() => setRecipientTouched(true)}
            disabled={isDisabled}
            autoComplete="off"
          />
          {!recipientValidation.valid && recipientValidation.error && (
            <div className={styles.errorText}>
              {t(recipientValidation.error as TranslationKey)}
            </div>
          )}
        </div>

        {/* Amount Input */}
        <div className={styles.fieldGroup}>
          <label htmlFor="transfer-amount" className={styles.label}>
            {t('transfer.amount')}
          </label>
          <div className={styles.inputWithSuffix}>
            <input
              id="transfer-amount"
              type="text"
              inputMode="decimal"
              className={`${styles.input} ${
                !amountValidation.valid ? styles.error : ''
              }`}
              placeholder={t('transfer.amountPlaceholder')}
              value={amount}
              onChange={(e) => setAmount(e.target.value)}
              onBlur={() => setAmountTouched(true)}
              disabled={isDisabled}
              autoComplete="off"
            />
            <span className={styles.suffix}>MORAL</span>
          </div>
          {!amountValidation.valid && amountValidation.error && (
            <div className={styles.errorText}>{t(amountValidation.error as TranslationKey)}</div>
          )}
          <div className={styles.balanceInfo}>
            {t('transfer.balance')}: {formatMoralBalance(balance)}
          </div>
        </div>

        {/* Submit Button */}
        <div className={styles.footer}>
          <button
            type="submit"
            className={styles.submitBtn}
            disabled={!isFormValid || isDisabled}
          >
            {t('transfer.send')}
          </button>
          <button
            type="button"
            className={styles.stealthBtn}
            onClick={() => setStealthModalOpen(true)}
          >
            {'🔐 ステルス送金'}
          </button>
        </div>
      </form>

      {/* Status Messages */}
      {state.status === 'pending' && (
        <div className={`${styles.status} ${styles.pending}`}>
          <span className={styles.spinner} />
          {t('transfer.sending')}
        </div>
      )}

      {state.status === 'success' && (
        <div className={`${styles.status} ${styles.success}`}>
          {t('transfer.success')}
          {state.txHash && (
            <div className={styles.txHash}>TX: {state.txHash}</div>
          )}
        </div>
      )}

      {state.status === 'error' && (
        <div className={`${styles.status} ${styles.error}`}>
          {state.error ? t(state.error as TranslationKey) : t('transfer.error')}
        </div>
      )}

      {/* Confirmation Dialog - rendered via Portal to escape stacking context */}
      {state.status === 'confirming' && mounted && createPortal(
        <div className={styles.confirmOverlay}>
          <div className={styles.confirmDialog}>
            <h4 className={styles.confirmTitle}>{t('transfer.confirmTitle')}</h4>
            <p className={styles.confirmMessage}>
              {t('transfer.confirmMessage')}
            </p>
            <div className={styles.confirmDetails}>
              <div className={styles.confirmRow}>
                <span className={styles.confirmLabel}>
                  {t('transfer.recipient')}:
                </span>
                <span className={styles.confirmValue}>
                  {shortenAddress(state.recipient || '', 8, 8)}
                </span>
              </div>
              <div className={styles.confirmRow}>
                <span className={styles.confirmLabel}>
                  {t('transfer.amount')}:
                </span>
                <span className={styles.confirmValue}>
                  {formattedAmount} MORAL
                </span>
              </div>
            </div>
            <div className={styles.confirmActions}>
              <button
                type="button"
                className={styles.cancelBtn}
                onClick={handleCancel}
              >
                {t('transfer.cancel')}
              </button>
              <button
                type="button"
                className={styles.submitBtn}
                onClick={handleConfirm}
              >
                {t('transfer.confirm')}
              </button>
            </div>
          </div>
        </div>,
        document.body
      )}

      {/* Stealth Address Modal */}
      <StealthModal
        isOpen={isStealthModalOpen}
        onClose={() => setStealthModalOpen(false)}
        unsafeApi={unsafeApi}
        signer={signer}
        accountAddress={senderAddress}
        isConnected={!!signer}
      />
    </div>
  )
}
