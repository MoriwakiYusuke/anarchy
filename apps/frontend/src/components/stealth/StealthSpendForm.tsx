/**
 * StealthSpendForm Component
 * 
 * T069 [US4] Create StealthSpendForm component
 * T070 [US4] Add multi-UTXO warning dialog
 * 
 * ステルス残高を使用するためのフォームコンポーネント
 */
'use client';

import { useState, useCallback, useMemo } from 'react';
import { DetectedStealthBalance } from '@/lib/stealth/types';
import { selectCoins, LINKABILITY_WARNING_THRESHOLD } from '@/lib/stealth/coinSelection';
import styles from './StealthSpendForm.module.css';

/**
 * フォーム値の型
 */
export interface SpendFormValues {
  selectedBalances: DetectedStealthBalance[];
  recipientAddress: string;
  amount: bigint;
}

/**
 * バリデーション結果
 */
export interface SpendFormValidation {
  valid: boolean;
  errors: {
    selectedBalances?: string;
    recipientAddress?: string;
    amount?: string;
  };
  linkabilityWarning?: boolean;
}

/**
 * フォームバリデーション
 */
export function validateSpendForm(values: SpendFormValues): SpendFormValidation {
  const errors: SpendFormValidation['errors'] = {};
  
  // 残高選択チェック
  if (values.selectedBalances.length === 0) {
    errors.selectedBalances = '残高を選択してください';
  } else {
    // 使用済み残高のチェック
    const hasSpent = values.selectedBalances.some(b => b.spent);
    if (hasSpent) {
      errors.selectedBalances = '使用済みの残高が含まれています';
    }
  }

  // 送金先アドレスチェック
  if (!values.recipientAddress || values.recipientAddress.trim() === '') {
    errors.recipientAddress = '送金先アドレスを入力してください';
  }

  // 金額チェック
  if (values.amount <= BigInt(0)) {
    errors.amount = '金額を入力してください';
  } else {
    // 残高不足チェック
    const totalSelected = values.selectedBalances.reduce(
      (sum, b) => sum + b.balance,
      BigInt(0)
    );
    if (values.amount > totalSelected) {
      errors.amount = '残高が不足しています';
    }
  }

  // リンク可能性警告
  const linkabilityWarning = values.selectedBalances.length >= LINKABILITY_WARNING_THRESHOLD;

  return {
    valid: Object.keys(errors).length === 0,
    errors,
    linkabilityWarning,
  };
}

/**
 * MORAL表示フォーマット
 */
function formatMoral(amount: bigint): string {
  const DECIMALS = 12;
  const divisor = BigInt(10 ** DECIMALS);
  const whole = amount / divisor;
  const fraction = amount % divisor;
  
  if (fraction === BigInt(0)) {
    return `${whole} MORAL`;
  }
  
  const fractionStr = fraction.toString().padStart(DECIMALS, '0').replace(/0+$/, '');
  return `${whole}.${fractionStr} MORAL`;
}

/**
 * MORAL入力パース
 */
function parseMoralInput(input: string): bigint | null {
  const trimmed = input.trim();
  if (!trimmed) return null;
  
  const num = parseFloat(trimmed);
  if (isNaN(num) || num <= 0) return null;
  
  const DECIMALS = 12;
  const [whole, fraction = ''] = trimmed.split('.');
  const paddedFraction = fraction.padEnd(DECIMALS, '0').slice(0, DECIMALS);
  
  try {
    return BigInt(whole + paddedFraction);
  } catch {
    return null;
  }
}

/**
 * StealthSpendForm Props
 */
export interface StealthSpendFormProps {
  /** 利用可能な残高リスト */
  balances: DetectedStealthBalance[];
  /** 送金実行コールバック */
  onSpend: (values: SpendFormValues) => Promise<void>;
  /** キャンセルコールバック */
  onCancel?: () => void;
  /** 処理中フラグ */
  isProcessing?: boolean;
  /** デフォルトの送金先アドレス（編集可） */
  defaultRecipientAddress?: string;
  /** 成功ステータス */
  successMessage?: string | null;
  /** エラーステータス */
  errorMessage?: string | null;
}

/**
 * ステルス残高使用フォーム
 */
export function StealthSpendForm({
  balances,
  onSpend,
  onCancel,
  isProcessing = false,
  defaultRecipientAddress = '',
  successMessage,
  errorMessage: externalError,
}: StealthSpendFormProps) {
  const [selectedAddresses, setSelectedAddresses] = useState<Set<string>>(new Set());
  const [recipientAddress, setRecipientAddress] = useState(defaultRecipientAddress);
  const [amountInput, setAmountInput] = useState('');
  const [showLinkabilityWarning, setShowLinkabilityWarning] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // 利用可能な(未使用)残高
  const availableBalances = useMemo(
    () => balances.filter(b => !b.spent),
    [balances]
  );

  // 選択された残高
  const selectedBalances = useMemo(
    () => availableBalances.filter(b => selectedAddresses.has(b.stealthAddress)),
    [availableBalances, selectedAddresses]
  );

  // 選択済み合計
  const selectedTotal = useMemo(
    () => selectedBalances.reduce((sum, b) => sum + b.balance, BigInt(0)),
    [selectedBalances]
  );

  // 残高選択トグル
  const toggleBalance = useCallback((address: string) => {
    setSelectedAddresses(prev => {
      const next = new Set(prev);
      if (next.has(address)) {
        next.delete(address);
      } else {
        next.add(address);
      }
      return next;
    });
  }, []);

  // 全選択
  const selectAll = useCallback(() => {
    setSelectedAddresses(new Set(availableBalances.map(b => b.stealthAddress)));
  }, [availableBalances]);

  // 選択解除
  const clearSelection = useCallback(() => {
    setSelectedAddresses(new Set());
  }, []);

  // 自動選択（coin selection使用）
  const autoSelect = useCallback(() => {
    const amount = parseMoralInput(amountInput);
    if (!amount) {
      setError('有効な金額を入力してください');
      return;
    }

    const result = selectCoins(availableBalances, amount);
    if (!result.sufficient) {
      setError('残高が不足しています');
      return;
    }

    setSelectedAddresses(new Set(result.selected.map(b => b.stealthAddress)));
    setError(null);
  }, [availableBalances, amountInput]);

  // 送信処理
  const handleSubmit = useCallback(async () => {
    const amount = parseMoralInput(amountInput);
    if (!amount) {
      setError('有効な金額を入力してください');
      return;
    }

    const values: SpendFormValues = {
      selectedBalances,
      recipientAddress: recipientAddress.trim(),
      amount,
    };

    const validation = validateSpendForm(values);
    
    if (!validation.valid) {
      const firstError = Object.values(validation.errors)[0];
      setError(firstError ?? '入力内容を確認してください');
      return;
    }

    // リンク可能性警告の確認
    if (validation.linkabilityWarning && !showLinkabilityWarning) {
      setShowLinkabilityWarning(true);
      return;
    }

    setError(null);
    await onSpend(values);
  }, [selectedBalances, recipientAddress, amountInput, showLinkabilityWarning, onSpend]);

  // リンク可能性警告を無視して続行
  const confirmLinkability = useCallback(async () => {
    const amount = parseMoralInput(amountInput);
    if (!amount) return;

    const values: SpendFormValues = {
      selectedBalances,
      recipientAddress: recipientAddress.trim(),
      amount,
    };

    setShowLinkabilityWarning(false);
    setError(null);
    await onSpend(values);
  }, [selectedBalances, recipientAddress, amountInput, onSpend]);

  return (
    <div className={styles.container}>
      {/* 残高選択 */}
      <div className={styles.balanceSelection}>
        <div className={styles.balanceHeader}>
          <h3>使用する残高</h3>
          <div className={styles.headerActions}>
            <button
              type="button"
              onClick={selectAll}
              className={styles.linkButton}
              disabled={isProcessing}
            >
              全選択
            </button>
            <button
              type="button"
              onClick={clearSelection}
              className={`${styles.linkButton} ${styles.linkButtonGray}`}
              disabled={isProcessing}
            >
              解除
            </button>
          </div>
        </div>

        {availableBalances.length === 0 ? (
          <p className={styles.emptyText}>利用可能な残高がありません</p>
        ) : (
          <ul className={styles.balanceList}>
            {availableBalances.map(balance => (
              <li
                key={balance.stealthAddress}
                className={`${styles.balanceItem} ${selectedAddresses.has(balance.stealthAddress) ? styles.balanceItemSelected : ''}`}
                onClick={() => !isProcessing && toggleBalance(balance.stealthAddress)}
              >
                <div className={styles.balanceItemRow}>
                  <span className={styles.balanceAddress}>
                    {balance.stealthAddress.slice(0, 8)}...{balance.stealthAddress.slice(-8)}
                  </span>
                  <span className={styles.balanceAmount}>
                    {formatMoral(balance.balance)}
                  </span>
                </div>
                <div className={styles.balanceBlock}>
                  Block #{balance.receivedAt}
                </div>
              </li>
            ))}
          </ul>
        )}

        <div className={styles.selectedTotal}>
          選択中: {formatMoral(selectedTotal)}
        </div>
      </div>

      {/* 金額入力 */}
      <div className={styles.inputGroup}>
        <label className={styles.label}>送金額 (MORAL)</label>
        <div className={styles.inputRow}>
          <input
            type="text"
            value={amountInput}
            onChange={e => setAmountInput(e.target.value)}
            placeholder="0.0"
            className={styles.input}
            disabled={isProcessing}
          />
          <button
            type="button"
            onClick={autoSelect}
            className={styles.autoSelectButton}
            disabled={isProcessing || !amountInput}
          >
            自動選択
          </button>
        </div>
      </div>

      {/* 送金先入力 */}
      <div className={styles.inputGroup}>
        <label className={styles.label}>送金先アドレス</label>
        <input
          type="text"
          value={recipientAddress}
          onChange={e => setRecipientAddress(e.target.value)}
          placeholder="5Grwva..."
          className={`${styles.input} ${styles.inputFull}`}
          disabled={isProcessing}
        />
      </div>

      {/* エラー表示 */}
      {error && (
        <div className={styles.errorText}>{error}</div>
      )}

      {/* リンク可能性警告ダイアログ */}
      {showLinkabilityWarning && (
        <div className={styles.warningBox}>
          <h4>⚠️ プライバシー警告</h4>
          <p>
            複数のステルスアドレスを同時に使用すると、それらが同じ受取人のもの
            であることがブロックチェーン上で明らかになります。
            これによりプライバシーが低下する可能性があります。
          </p>
          <div className={styles.warningActions}>
            <button
              type="button"
              onClick={confirmLinkability}
              className={styles.warningConfirm}
              disabled={isProcessing}
            >
              理解して続行
            </button>
            <button
              type="button"
              onClick={() => setShowLinkabilityWarning(false)}
              className={styles.warningCancel}
              disabled={isProcessing}
            >
              キャンセル
            </button>
          </div>
        </div>
      )}

      {/* 成功メッセージ */}
      {successMessage && (
        <div className={styles.successMessage}>
          {successMessage}
        </div>
      )}

      {/* 外部エラーメッセージ */}
      {externalError && (
        <div className={styles.errorText}>
          {externalError}
        </div>
      )}

      {/* ボタン */}
      <div className={styles.actions}>
        <button
          type="button"
          onClick={handleSubmit}
          disabled={isProcessing || selectedBalances.length === 0}
          className={styles.submitButton}
        >
          {isProcessing ? '処理中...' : '送金'}
        </button>
        {onCancel && (
          <button
            type="button"
            onClick={onCancel}
            disabled={isProcessing}
            className={styles.cancelButton}
          >
            キャンセル
          </button>
        )}
      </div>
    </div>
  );
}
