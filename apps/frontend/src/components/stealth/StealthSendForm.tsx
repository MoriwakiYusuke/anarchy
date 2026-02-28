'use client';

/**
 * T046-T047: StealthSendForm Component
 *
 * Form for sending funds to a stealth address.
 * Validates meta-address format and amount before submission.
 */

import { useState, useCallback } from 'react';
import styles from './StealthSendForm.module.css';

// MORAL token has 12 decimals
const MORAL_DECIMALS = 12;
const MORAL_MULTIPLIER = BigInt(10 ** MORAL_DECIMALS);

export interface ValidationResult {
  valid: boolean;
  error?: string;
}

export interface StealthSendFormProps {
  onSend: (metaAddress: string, amount: string, ephemeralPubkey: Uint8Array) => Promise<void>;
  disabled?: boolean;
}

/**
 * Validate a meta-address format
 */
export function validateMetaAddress(address: string): ValidationResult {
  if (!address || address.trim() === '') {
    return { valid: false, error: 'メタアドレスを入力してください' };
  }

  const trimmed = address.trim();

  // Check prefix
  if (!trimmed.startsWith('st:anarchy:')) {
    return { valid: false, error: 'メタアドレスは st:anarchy: で始まる必要があります' };
  }

  const parts = trimmed.split(':');
  if (parts.length !== 4) {
    return { valid: false, error: '無効なメタアドレス形式です' };
  }

  const spendPubkey = parts[2];
  const viewPubkey = parts[3];

  // Check hex format (32 bytes = 64 hex chars)
  const hexRegex = /^[0-9a-fA-F]{64}$/;
  if (!hexRegex.test(spendPubkey)) {
    return { valid: false, error: 'spend公開鍵が無効です' };
  }
  if (!hexRegex.test(viewPubkey)) {
    return { valid: false, error: 'view公開鍵が無効です' };
  }

  // Try to parse with wasm module (lazy load)
  try {
    // Dynamic import will be used in actual component
    return { valid: true };
  } catch {
    return { valid: false, error: 'メタアドレスの解析に失敗しました' };
  }
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
        setAddressError(result.error || '無効なアドレスです');
      }
    }
  }, [metaAddress]);

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
        setAmountError('有効な金額を入力してください');
      }
    }
  }, [amount]);

  const handleSubmit = useCallback(async (e: React.FormEvent) => {
    e.preventDefault();

    // Validate address
    const addressValidation = validateMetaAddress(metaAddress);
    if (!addressValidation.valid) {
      setAddressError(addressValidation.error || '無効なアドレスです');
      return;
    }

    // Validate amount
    const formattedAmount = formatAmount(amount);
    if (formattedAmount === null) {
      setAmountError('有効な金額を入力してください');
      return;
    }

    setIsSubmitting(true);
    setTxStatus('pending');
    setTxMessage('トランザクションを送信中...');

    try {
      // Import wasm module and derive stealth address
      const wasm = await import('anarchy-wasm-engine');
      const derivation = wasm.derive_stealth_address(metaAddress);

      // Call the parent's onSend handler
      await onSend(metaAddress, formattedAmount, derivation.ephemeral_pubkey);

      setTxStatus('success');
      setTxMessage('送金が完了しました');
      
      // Clear form
      setMetaAddress('');
      setAmount('');
    } catch (error) {
      setTxStatus('error');
      setTxMessage(error instanceof Error ? error.message : '送金に失敗しました');
    } finally {
      setIsSubmitting(false);
    }
  }, [metaAddress, amount, onSend]);

  const isDisabled = disabled || isSubmitting;

  return (
    <form onSubmit={handleSubmit} className={styles.form}>
      {/* Meta-address input */}
      <div className={styles.inputGroup}>
        <label htmlFor="stealth-meta-address" className={styles.label}>
          受取人のメタアドレス
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
          送金額 (MORAL)
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
        {isSubmitting ? '送信中...' : 'ステルス送金'}
      </button>
    </form>
  );
}

export default StealthSendForm;
