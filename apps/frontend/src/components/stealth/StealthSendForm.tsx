'use client';

/**
 * T046-T047: StealthSendForm Component
 *
 * Form for sending funds to a stealth address.
 * Validates meta-address format and amount before submission.
 */

import { useState, useCallback } from 'react';
import { useLocale } from '../../i18n/context';
import type { TranslationKey } from '../../i18n/types';
import { deriveStealthAddress } from '@/lib/stealth/keyManager';
import styles from './StealthSendForm.module.css';

// MORAL token has 12 decimals
const MORAL_DECIMALS = 12;
const MORAL_MULTIPLIER = BigInt(10 ** MORAL_DECIMALS);

export interface ValidationResult {
  valid: boolean;
  error?: string;
}

export interface StealthSendFormProps {
  onSend: (stealthAddress: string, amount: string, ephemeralPubkey: Uint8Array) => Promise<void>;
  disabled?: boolean;
}

/**
 * Validate a meta-address format
 * Returns translation keys for errors
 */
export function validateMetaAddress(address: string): ValidationResult {
  if (!address || address.trim() === '') {
    return { valid: false, error: 'stealth.sendForm.error.metaAddressRequired' };
  }

  const trimmed = address.trim();

  // Check prefix
  if (!trimmed.startsWith('st:anarchy:')) {
    return { valid: false, error: 'stealth.sendForm.error.metaAddressPrefix' };
  }

  const parts = trimmed.split(':');
  // Format: st:anarchy:<base58_encoded_keys>
  if (parts.length !== 3) {
    return { valid: false, error: 'stealth.sendForm.error.metaAddressFormat' };
  }

  const encoded = parts[2];
  
  // Base58 character validation
  const base58Regex = /^[1-9A-HJ-NP-Za-km-z]+$/;
  if (!base58Regex.test(encoded)) {
    return { valid: false, error: 'stealth.sendForm.error.metaAddressBase58' };
  }
  
  // Base58 encoded 64 bytes should be around 87-88 characters
  if (encoded.length < 80 || encoded.length > 90) {
    return { valid: false, error: 'stealth.sendForm.error.metaAddressLength' };
  }

  return { valid: true };
}

/**
 * Format user input amount to on-chain units
 * Returns null if the amount is invalid
 */
export function formatAmount(input: string): string | null {
  if (!input || input.trim() === '') {
    return null;
  }

  const trimmed = input.trim();

  // Parse as float first to validate
  const num = parseFloat(trimmed);
  if (isNaN(num) || num <= 0) {
    return null;
  }

  // Handle decimal conversion properly
  try {
    // Split into integer and decimal parts
    const parts = trimmed.split('.');
    const integerPart = parts[0] || '0';
    let decimalPart = parts[1] || '';

    // Pad or truncate decimal part to MORAL_DECIMALS
    if (decimalPart.length > MORAL_DECIMALS) {
      decimalPart = decimalPart.slice(0, MORAL_DECIMALS);
    } else {
      decimalPart = decimalPart.padEnd(MORAL_DECIMALS, '0');
    }

    // Combine into single integer
    const combined = integerPart + decimalPart;
    const result = BigInt(combined);

    // Check for zero
    if (result === BigInt(0)) {
      return null;
    }

    return result.toString();
  } catch {
    return null;
  }
}

/**
 * StealthSendForm Component
 */
export function StealthSendForm({
  onSend,
  disabled = false,
}: StealthSendFormProps) {
  const { t } = useLocale();
  const [metaAddress, setMetaAddress] = useState('');
  const [amount, setAmount] = useState('');
  const [addressError, setAddressError] = useState<string | null>(null);
  const [amountError, setAmountError] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [txStatus, setTxStatus] = useState<'idle' | 'pending' | 'success' | 'error'>('idle');
  const [txMessage, setTxMessage] = useState<string | null>(null);

  const handleAddressChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const value = e.target.value;
    setMetaAddress(value);
    
    // Clear previous error
    if (addressError) {
      setAddressError(null);
    }
  }, [addressError]);

  const handleAddressBlur = useCallback(() => {
    if (metaAddress) {
      const result = validateMetaAddress(metaAddress);
      if (!result.valid) {
        setAddressError(t((result.error || 'stealth.sendForm.error.invalidAddress') as TranslationKey));
      }
    }
  }, [metaAddress, t]);

  const handleAmountChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const value = e.target.value;
    setAmount(value);

    // Clear previous error
    if (amountError) {
      setAmountError(null);
    }
  }, [amountError]);

  const handleAmountBlur = useCallback(() => {
    if (amount) {
      const formatted = formatAmount(amount);
      if (formatted === null) {
        setAmountError(t('stealth.sendForm.error.invalidAmount'));
      }
    }
  }, [amount, t]);

  const handleSubmit = useCallback(async (e: React.FormEvent) => {
    e.preventDefault();

    // Validate address
    const addressValidation = validateMetaAddress(metaAddress);
    if (!addressValidation.valid) {
      setAddressError(t((addressValidation.error || 'stealth.sendForm.error.invalidAddress') as TranslationKey));
      return;
    }

    // Validate amount
    const formattedAmount = formatAmount(amount);
    if (formattedAmount === null) {
      setAmountError(t('stealth.sendForm.error.invalidAmount'));
      return;
    }

    setIsSubmitting(true);
    setTxStatus('pending');
    setTxMessage(t('stealth.sendForm.sending'));

    try {
      // Derive stealth address using properly initialized wasm
      const derivation = await deriveStealthAddress(metaAddress);

      // Call the parent's onSend handler with the derived stealth address
      // Important: pass stealthAddress (not metaAddress) so ephemeralPubkey matches
      await onSend(derivation.stealthAddress, formattedAmount, derivation.ephemeralPubkey);

      setTxStatus('success');
      setTxMessage(t('stealth.sendForm.success'));
      
      // Clear form
      setMetaAddress('');
      setAmount('');
    } catch (error) {
      setTxStatus('error');
      setTxMessage(error instanceof Error ? error.message : t('stealth.sendForm.error'));
    } finally {
      setIsSubmitting(false);
    }
  }, [metaAddress, amount, onSend, t]);

  const isDisabled = disabled || isSubmitting;

  return (
    <form onSubmit={handleSubmit} className={styles.form}>
      {/* Meta-address input */}
      <div className={styles.inputGroup}>
        <label htmlFor="stealth-meta-address" className={styles.label}>
          {t('stealth.sendForm.recipientLabel')}
        </label>
        <input
          id="stealth-meta-address"
          type="text"
          value={metaAddress}
          onChange={handleAddressChange}
          onBlur={handleAddressBlur}
          placeholder="st:anarchy:..."
          disabled={isDisabled}
          className={`${styles.input} ${addressError ? styles.inputError : ''}`}
        />
        {addressError && (
          <p className={styles.errorText} role="alert">
            {addressError}
          </p>
        )}
      </div>

      {/* Amount input */}
      <div className={styles.inputGroup}>
        <label htmlFor="stealth-amount" className={styles.label}>
          {t('stealth.sendForm.amountLabel')}
        </label>
        <input
          id="stealth-amount"
          type="text"
          inputMode="decimal"
          value={amount}
          onChange={handleAmountChange}
          onBlur={handleAmountBlur}
          placeholder="10.0"
          disabled={isDisabled}
          className={`${styles.input} ${amountError ? styles.inputError : ''}`}
        />
        {amountError && (
          <p className={styles.errorText} role="alert">
            {amountError}
          </p>
        )}
      </div>

      {/* Transaction status */}
      {txStatus !== 'idle' && txMessage && (
        <div
          className={`${styles.statusMessage} ${
            txStatus === 'pending' ? styles.statusPending :
            txStatus === 'success' ? styles.statusSuccess :
            txStatus === 'error' ? styles.statusError : ''
          }`}
          role="status"
        >
          {txMessage}
        </div>
      )}

      {/* Submit button */}
      <button
        type="submit"
        disabled={isDisabled}
        className={styles.submitButton}
      >
        {isSubmitting ? t('stealth.sendForm.submitting') : t('stealth.sendForm.submit')}
      </button>
    </form>
  );
}

export default StealthSendForm;
