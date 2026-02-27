/**
 * T059: StealthBalanceList Component
 *
 * Displays a list of detected stealth balances with scan controls.
 */

'use client';

import React from 'react';
import type { DetectedStealthBalance, ScanProgress } from '@/lib/stealth/types';

// MORAL token has 12 decimals
const MORAL_DECIMALS = 12;

export interface StealthBalanceListProps {
  /** List of detected balances */
  balances: DetectedStealthBalance[];
  /** Whether scanning is in progress */
  isScanning?: boolean;
  /** Current scan progress */
  scanProgress?: ScanProgress;
  /** Whether to show spent balances */
  showSpent?: boolean;
  /** Called when a balance is selected */
  onSelect?: (balance: DetectedStealthBalance) => void;
  /** Called when scan button is clicked */
  onStartScan?: () => void;
  /** Called when stop button is clicked */
  onStopScan?: () => void;
}

/**
 * Format balance amount for display
 */
function formatBalance(balance: bigint): string {
  const divisor = BigInt(10 ** MORAL_DECIMALS);
  const wholePart = balance / divisor;
  const fractionalPart = balance % divisor;
  
  // Format fractional part with 4 decimal places
  const fractionalStr = fractionalPart.toString().padStart(MORAL_DECIMALS, '0').slice(0, 4);
  
  return `${wholePart}.${fractionalStr}`;
}

/**
 * Truncate address for display
 */
function truncateAddress(address: string): string {
  if (address.length <= 12) return address;
  return `${address.slice(0, 6)}...${address.slice(-4)}`;
}

/**
 * StealthBalanceList component
 */
export default function StealthBalanceList({
  balances,
  isScanning = false,
  scanProgress,
  showSpent = true,
  onSelect,
  onStartScan,
  onStopScan,
}: StealthBalanceListProps) {
  // Filter balances based on showSpent
  const displayBalances = showSpent 
    ? balances 
    : balances.filter(b => !b.spent);

  // Calculate total balance (unspent only)
  const totalBalance = balances
    .filter(b => !b.spent)
    .reduce((sum, b) => sum + b.balance, BigInt(0));

  return (
    <div className="stealth-balance-list">
      {/* Header with total and scan controls */}
      <div className="flex items-center justify-between mb-4">
        <div>
          <h3 className="text-lg font-semibold">ステルス残高</h3>
          <p className="text-sm text-gray-500">
            合計: {formatBalance(totalBalance)} MORAL
          </p>
        </div>
        <div>
          {isScanning ? (
            <button
              onClick={onStopScan}
              className="px-4 py-2 text-sm font-medium text-white bg-red-600 rounded-md hover:bg-red-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-red-500"
            >
              停止
            </button>
          ) : (
            <button
              onClick={onStartScan}
              className="px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500"
            >
              スキャン
            </button>
          )}
        </div>
      </div>

      {/* Scan progress */}
      {isScanning && scanProgress && (
        <div className="mb-4 p-3 bg-blue-50 rounded-md">
          <p className="text-sm text-blue-800">
            スキャン中... {scanProgress.percentage}%
          </p>
          <div className="mt-1 w-full bg-blue-200 rounded-full h-2">
            <div 
              className="bg-blue-600 h-2 rounded-full transition-all duration-300" 
              style={{ width: `${scanProgress.percentage}%` }}
            />
          </div>
          <p className="mt-1 text-xs text-blue-600">
            ブロック {scanProgress.currentBlock} / {scanProgress.targetBlock}
            {scanProgress.detectedCount > 0 && ` (${scanProgress.detectedCount}件検出)`}
          </p>
        </div>
      )}

      {/* Balance list */}
      {displayBalances.length === 0 ? (
        <div className="text-center py-8 text-gray-500">
          <p>残高がありません</p>
          <p className="text-sm mt-1">
            スキャンを実行して送金を検出してください
          </p>
        </div>
      ) : (
        <ul className="space-y-2">
          {displayBalances.map((balance, index) => (
            <li
              key={`${balance.stealthAddress}-${index}`}
              role="button"
              tabIndex={0}
              onClick={() => onSelect?.(balance)}
              onKeyDown={(e) => {
                if (e.key === 'Enter' || e.key === ' ') {
                  onSelect?.(balance);
                }
              }}
              className={`
                p-3 rounded-md border cursor-pointer transition-colors
                ${balance.spent 
                  ? 'bg-gray-50 border-gray-200 opacity-60' 
                  : 'bg-white border-gray-300 hover:border-blue-400 hover:bg-blue-50'
                }
              `}
            >
              <div className="flex items-center justify-between">
                <div>
                  <p className="font-mono text-sm">
                    {truncateAddress(balance.stealthAddress)}
                  </p>
                  <p className="text-xs text-gray-500">
                    ブロック #{balance.receivedAt}
                  </p>
                </div>
                <div className="text-right">
                  <p className="font-semibold">
                    {formatBalance(balance.balance)} MORAL
                  </p>
                  {balance.spent && (
                    <span className="text-xs text-red-500">使用済み</span>
                  )}
                </div>
              </div>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
