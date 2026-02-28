/**
 * T059: StealthBalanceList Component
 *
 * Displays a list of detected stealth balances with scan controls.
 */

'use client';

import React from 'react';
import type { DetectedStealthBalance, ScanProgress } from '@/lib/stealth/types';
import { formatMoralBalance } from '@/hooks/useMoralBalance';
import styles from './StealthBalanceList.module.css';

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
    <div className={styles.container}>
      {/* Header with total and scan controls */}
      <div className={styles.header}>
        <div className={styles.headerInfo}>
          <h3>ステルス残高</h3>
          <p>
            合計: {formatMoralBalance(totalBalance)}
          </p>
        </div>
        <div>
          {isScanning ? (
            <button
              onClick={onStopScan}
              className={`${styles.scanButton} ${styles.scanButtonStop}`}
            >
              停止
            </button>
          ) : (
            <button
              onClick={onStartScan}
              className={`${styles.scanButton} ${styles.scanButtonStart}`}
            >
              スキャン
            </button>
          )}
        </div>
      </div>

      {/* Scan progress */}
      {isScanning && scanProgress && (
        <div className={styles.progressBox}>
          <p className={styles.progressText}>
            スキャン中... {scanProgress.percentage}%
          </p>
          <div className={styles.progressBar}>
            <div 
              className={styles.progressFill}
              style={{ width: `${scanProgress.percentage}%` }}
            />
          </div>
          <p className={styles.progressDetail}>
            ブロック {scanProgress.currentBlock} / {scanProgress.targetBlock}
            {scanProgress.detectedCount > 0 && ` (${scanProgress.detectedCount}件検出)`}
          </p>
        </div>
      )}

      {/* Balance list */}
      {displayBalances.length === 0 ? (
        <div className={styles.emptyState}>
          <p>残高がありません</p>
          <p>
            スキャンを実行して送金を検出してください
          </p>
        </div>
      ) : (
        <ul className={styles.balanceList}>
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
              className={`${styles.balanceItem} ${balance.spent ? styles.balanceItemSpent : ''}`}
            >
              <div className={styles.balanceItemHeader}>
                <span className={styles.balanceAddress}>
                  {truncateAddress(balance.stealthAddress)}
                </span>
                <span className={styles.balanceAmount}>
                  {formatMoralBalance(balance.balance)}
                </span>
              </div>
              <div className={styles.balanceItemFooter}>
                <span>ブロック #{balance.receivedAt}</span>
                {balance.spent && (
                  <span className={styles.spentBadge}>使用済み</span>
                )}
              </div>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
