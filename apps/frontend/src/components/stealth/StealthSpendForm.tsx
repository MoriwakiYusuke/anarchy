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
}

/**
 * ステルス残高使用フォーム
 */
export function StealthSpendForm({
  balances,
  onSpend,
  onCancel,
  isProcessing = false,
}: StealthSpendFormProps) {
  const [selectedAddresses, setSelectedAddresses] = useState<Set<string>>(new Set());
  const [recipientAddress, setRecipientAddress] = useState('');
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
    <div className="stealth-spend-form space-y-4">
      {/* 残高選択 */}
      <div className="balance-selection">
        <div className="flex justify-between items-center mb-2">
          <h3 className="text-lg font-semibold">使用する残高</h3>
          <div className="space-x-2">
            <button
              type="button"
              onClick={selectAll}
              className="text-sm text-blue-600 hover:underline"
              disabled={isProcessing}
            >
              全選択
            </button>
            <button
              type="button"
              onClick={clearSelection}
              className="text-sm text-gray-600 hover:underline"
              disabled={isProcessing}
            >
              解除
            </button>
          </div>
        </div>

        {availableBalances.length === 0 ? (
          <p className="text-gray-500">利用可能な残高がありません</p>
        ) : (
          <ul className="space-y-2">
            {availableBalances.map(balance => (
              <li
                key={balance.stealthAddress}
                className={`p-3 border rounded cursor-pointer transition ${
                  selectedAddresses.has(balance.stealthAddress)
                    ? 'border-blue-500 bg-blue-50'
                    : 'border-gray-200 hover:border-gray-400'
                }`}
                onClick={() => !isProcessing && toggleBalance(balance.stealthAddress)}
              >
                <div className="flex justify-between items-center">
                  <span className="font-mono text-sm truncate max-w-[60%]">
                    {balance.stealthAddress.slice(0, 8)}...{balance.stealthAddress.slice(-8)}
                  </span>
                  <span className="font-semibold">
                    {formatMoral(balance.balance)}
                  </span>
                </div>
                <div className="text-xs text-gray-500 mt-1">
                  Block #{balance.receivedAt}
                </div>
              </li>
            ))}
          </ul>
        )}

        <div className="mt-2 text-right text-sm text-gray-600">
          選択中: {formatMoral(selectedTotal)}
        </div>
      </div>

      {/* 金額入力 */}
      <div className="amount-input">
        <label className="block text-sm font-medium mb-1">送金額 (MORAL)</label>
        <div className="flex gap-2">
          <input
            type="text"
            value={amountInput}
            onChange={e => setAmountInput(e.target.value)}
            placeholder="0.0"
            className="flex-1 px-3 py-2 border rounded focus:ring-2 focus:ring-blue-500"
            disabled={isProcessing}
          />
          <button
            type="button"
            onClick={autoSelect}
            className="px-3 py-2 bg-gray-100 hover:bg-gray-200 rounded text-sm"
            disabled={isProcessing || !amountInput}
          >
            自動選択
          </button>
        </div>
      </div>

      {/* 送金先入力 */}
      <div className="recipient-input">
        <label className="block text-sm font-medium mb-1">送金先アドレス</label>
        <input
          type="text"
          value={recipientAddress}
          onChange={e => setRecipientAddress(e.target.value)}
          placeholder="5Grwva..."
          className="w-full px-3 py-2 border rounded focus:ring-2 focus:ring-blue-500"
          disabled={isProcessing}
        />
      </div>

      {/* エラー表示 */}
      {error && (
        <div className="text-red-600 text-sm">{error}</div>
      )}

      {/* リンク可能性警告ダイアログ */}
      {showLinkabilityWarning && (
        <div className="bg-yellow-50 border border-yellow-400 rounded p-4">
          <h4 className="font-semibold text-yellow-800 mb-2">
            ⚠️ プライバシー警告
          </h4>
          <p className="text-sm text-yellow-700 mb-3">
            複数のステルスアドレスを同時に使用すると、それらが同じ受取人のもの
            であることがブロックチェーン上で明らかになります。
            これによりプライバシーが低下する可能性があります。
          </p>
          <div className="flex gap-2">
            <button
              type="button"
              onClick={confirmLinkability}
              className="px-4 py-2 bg-yellow-600 text-white rounded hover:bg-yellow-700"
              disabled={isProcessing}
            >
              理解して続行
            </button>
            <button
              type="button"
              onClick={() => setShowLinkabilityWarning(false)}
              className="px-4 py-2 bg-gray-200 rounded hover:bg-gray-300"
              disabled={isProcessing}
            >
              キャンセル
            </button>
          </div>
        </div>
      )}

      {/* ボタン */}
      <div className="flex gap-3">
        <button
          type="button"
          onClick={handleSubmit}
          disabled={isProcessing || selectedBalances.length === 0}
          className="flex-1 px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 disabled:bg-gray-400 disabled:cursor-not-allowed"
        >
          {isProcessing ? '処理中...' : '送金'}
        </button>
        {onCancel && (
          <button
            type="button"
            onClick={onCancel}
            disabled={isProcessing}
            className="px-4 py-2 bg-gray-200 rounded hover:bg-gray-300"
          >
            キャンセル
          </button>
        )}
      </div>
    </div>
  );
}
