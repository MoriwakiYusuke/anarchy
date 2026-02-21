/**
 * T077: ScoreIndicator Component Tests
 *
 * Tests "Content Unavailable" warning display for forgetting candidates
 *
 * Acceptance Scenario: AS4-3 UI
 * - When content has < 3 available shares
 * - Display "このコンテンツは利用できなくなりました" message
 * - Show "Forgetting Candidate" warning for low-score content
 *
 * spec.md Ref: FR-304
 */

import { render, screen } from '@testing-library/react';
import { ScoreIndicator } from '@/components/ScoreIndicator';

describe('ScoreIndicator', () => {
  describe('T077: Forgetting Candidate Warning Display', () => {
    it('displays "content unavailable" when shares < threshold', () => {
      const contentHash = '0x1234567890abcdef';
      const availableShares = 2; // Below threshold of 3

      render(
        <ScoreIndicator contentHash={contentHash} availableShares={availableShares} />
      );

      expect(screen.getByText(/利用できなくなりました/)).toBeInTheDocument();
      expect(screen.getByRole('alert')).toBeInTheDocument();
    });

    it('displays "forgetting candidate" warning for low-score content', () => {
      const contentHash = '0x1234567890abcdef';
      const score = 50; // Below default threshold of 100

      render(
        <ScoreIndicator
          contentHash={contentHash}
          score={score}
          isForgettingCandidate={true}
        />
      );

      expect(screen.getByText(/忘却候補/)).toBeInTheDocument();
      expect(screen.getByRole('alert')).toHaveClass('warning');
    });

    it('displays normal state for healthy content', () => {
      const contentHash = '0x1234567890abcdef';
      const score = 500; // Above threshold
      const availableShares = 5; // Above recovery threshold

      render(
        <ScoreIndicator
          contentHash={contentHash}
          score={score}
          availableShares={availableShares}
        />
      );

      expect(screen.queryByText(/利用できなくなりました/)).not.toBeInTheDocument();
      expect(screen.queryByText(/忘却候補/)).not.toBeInTheDocument();
    });

    it('shows score percentage indicator', () => {
      const score = 150;

      render(<ScoreIndicator contentHash="0x1234..." score={score} />);

      // aria-valuenow is set to the raw score value (not percentage)
      const progressbar = screen.getByRole('progressbar');
      expect(progressbar).toBeInTheDocument();
      expect(progressbar).toHaveAttribute('aria-valuenow', '150');
    });
  });
});
