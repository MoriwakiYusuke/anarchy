/**
 * T059: Jest tests for StealthBalanceList component
 *
 * Tests the balance list display and notification functionality
 */

import React from 'react';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import StealthBalanceList from '@/components/stealth/StealthBalanceList';
import type { DetectedStealthBalance, ScanProgress } from '@/lib/stealth/types';

// Mock balance data (matching DetectedStealthBalance interface)
const mockBalances: DetectedStealthBalance[] = [
  {
    stealthAddress: '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
    balance: BigInt('1000000000000'), // 1 MORAL
    receivedAt: 100,
    ephemeralPubkey: new Uint8Array(32),
    txHash: new Uint8Array(32),
    spent: false,
  },
  {
    stealthAddress: '5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty',
    balance: BigInt('500000000000'), // 0.5 MORAL
    receivedAt: 150,
    ephemeralPubkey: new Uint8Array(32),
    txHash: new Uint8Array(32),
    spent: false,
  },
];

const mockScanProgress: ScanProgress = {
  currentBlock: 200,
  targetBlock: 300,
  percentage: 67,
  detectedCount: 2,
};

describe('StealthBalanceList', () => {
  describe('rendering', () => {
    it('should render empty state when no balances', () => {
      render(<StealthBalanceList balances={[]} />);
      
      expect(screen.getByText(/残高がありません/i)).toBeInTheDocument();
    });

    it('should render list of balances', () => {
      render(<StealthBalanceList balances={mockBalances} />);
      
      // Should show balance amounts (getAllByText since amounts appear multiple places)
      const morals = screen.getAllByText(/MORAL/i);
      expect(morals.length).toBeGreaterThan(1); // Total + 2 balances
    });

    it('should show total balance', () => {
      render(<StealthBalanceList balances={mockBalances} />);
      
      // Total should be 1.5 MORAL
      expect(screen.getByText(/合計/i)).toBeInTheDocument();
      // Total is displayed in header
      expect(screen.getByText(/1\.5000\s*MORAL/i)).toBeInTheDocument();
    });

    it('should truncate stealth addresses', () => {
      render(<StealthBalanceList balances={mockBalances} />);
      
      // Full address should not be shown, truncated version should
      expect(screen.queryByText('5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY')).not.toBeInTheDocument();
      expect(screen.getByText(/5Grwva\.\.\.utQY/i)).toBeInTheDocument();
    });
  });

  describe('scan progress', () => {
    it('should show scan progress when scanning', () => {
      render(
        <StealthBalanceList 
          balances={mockBalances} 
          isScanning={true}
          scanProgress={mockScanProgress}
        />
      );
      
      expect(screen.getByText(/スキャン中/i)).toBeInTheDocument();
      expect(screen.getByText(/67%/i)).toBeInTheDocument();
    });

    it('should hide progress when not scanning', () => {
      render(
        <StealthBalanceList 
          balances={mockBalances} 
          isScanning={false}
        />
      );
      
      expect(screen.queryByText(/スキャン中/i)).not.toBeInTheDocument();
    });
  });

  describe('interactions', () => {
    it('should call onSelect when balance is clicked', () => {
      const onSelect = jest.fn();
      render(
        <StealthBalanceList 
          balances={mockBalances} 
          onSelect={onSelect}
        />
      );
      
      const firstBalance = screen.getByText(/5Grwva\.\.\.utQY/i).closest('button, div[role="button"], li');
      if (firstBalance) {
        fireEvent.click(firstBalance);
        expect(onSelect).toHaveBeenCalledWith(mockBalances[0]);
      }
    });

    it('should call onStartScan when scan button is clicked', () => {
      const onStartScan = jest.fn();
      render(
        <StealthBalanceList 
          balances={[]} 
          onStartScan={onStartScan}
        />
      );
      
      const scanButton = screen.getByRole('button', { name: /スキャン/i });
      fireEvent.click(scanButton);
      expect(onStartScan).toHaveBeenCalled();
    });

    it('should call onStopScan when stop button is clicked during scanning', () => {
      const onStopScan = jest.fn();
      render(
        <StealthBalanceList 
          balances={mockBalances}
          isScanning={true}
          scanProgress={mockScanProgress}
          onStopScan={onStopScan}
        />
      );
      
      const stopButton = screen.getByRole('button', { name: /停止/i });
      fireEvent.click(stopButton);
      expect(onStopScan).toHaveBeenCalled();
    });
  });

  describe('spent balances', () => {
    it('should show spent indicator for spent balances', () => {
      const balancesWithSpent: DetectedStealthBalance[] = [
        {
          ...mockBalances[0],
          spent: true,
        },
        mockBalances[1],
      ];

      render(<StealthBalanceList balances={balancesWithSpent} />);
      
      expect(screen.getByText(/使用済み/i)).toBeInTheDocument();
    });

    it('should filter out spent balances when showSpent is false', () => {
      const balancesWithSpent: DetectedStealthBalance[] = [
        {
          ...mockBalances[0],
          spent: true,
        },
        mockBalances[1],
      ];

      render(<StealthBalanceList balances={balancesWithSpent} showSpent={false} />);
      
      expect(screen.queryByText(/使用済み/i)).not.toBeInTheDocument();
      // Only unspent balance should be shown (getAllByText for amount appearing in total and item)
      const elements = screen.getAllByText(/0\.5000\s*MORAL/i);
      expect(elements.length).toBeGreaterThan(0);
    });
  });

  describe('formatting', () => {
    it('should format amounts with proper decimals', () => {
      const smallAmount: DetectedStealthBalance = {
        stealthAddress: '5DAAnrj7VHTznn2AWBemMuyBwZWs6FNFjdyVXUeYum3PTXFy',
        balance: BigInt('123456789012'), // 0.123456789012 MORAL
        receivedAt: 200,
        ephemeralPubkey: new Uint8Array(32),
        txHash: new Uint8Array(32),
        spent: false,
      };

      render(<StealthBalanceList balances={[smallAmount]} />);
      
      // Should show truncated decimal (getAllByText since it appears in both item and total)
      const elements = screen.getAllByText(/0\.1234/i);
      expect(elements.length).toBeGreaterThan(0);
    });

    it('should show block number for detection', () => {
      render(<StealthBalanceList balances={mockBalances} />);
      
      // First balance was received at block 100
      expect(screen.getByText(/ブロック #100/i)).toBeInTheDocument();
    });
  });
});
