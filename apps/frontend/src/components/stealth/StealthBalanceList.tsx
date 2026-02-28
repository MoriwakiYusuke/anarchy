/**
 * T059: StealthBalanceList Component
 *
 * Displays a list of detected stealth balances with scan controls.
 */

'use client';

import React from 'react';
import { useLocale } from '../../i18n/context';
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
  const { t } = useLocale();
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
          <h3>{t('stealth.scan.title')}</h3>
          <p>
            {t('stealth.scan.total')}: {formatMoralBalance(totalBalance)}
          </p>
        </div>
        <div>
          {isScanning ? (
            <button
              onClick={onStopScan}
              className={`${styles.scanButton} ${styles.scanButtonStop}`}
            >
              {t('stealth.scan.stop')}
            </button>
          ) : (
            <button
              onClick={onStartScan}
              className={`${styles.scanButton} ${styles.scanButtonStart}`}
            >
              {t('stealth.scan.start')}
            </button>
          )}
        </div>
      </div>

      {/* Scan progress */}
      {isScanning && scanProgress && (
        <div className={styles.progressBox}>
          <p className={styles.progressText}>
            {t('stealth.scan.scanning')} {scanProgress.percentage}%
          </p>
          <div className={styles.progressBar}>
            <div 
              className={styles.progressFill}
              style={{ width: `${scanProgress.percentage}%` }}
            />
          </div>
          <p className={styles.progressDetail}>
            {t('stealth.scan.block')} {scanProgress.currentBlock} / {scanProgress.targetBlock}
            {scanProgress.detectedCount > 0 && ` (${t('stealth.scan.detected', { count: String(scanProgress.detectedCount) })})`}
          </p>
        </div>
      )}

      {/* Balance list */}
      {displayBalances.length === 0 ? (
        <div className={styles.emptyState}>
          <p>{t('stealth.scan.empty')}</p>
          <p>
            {t('stealth.scan.emptyHint')}
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
                <span>{t('stealth.scan.block')} #{balance.receivedAt}</span>
                {balance.spent && (
                  <span className={styles.spentBadge}>{t('stealth.balance.spent')}</span>
                )}
              </div>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
