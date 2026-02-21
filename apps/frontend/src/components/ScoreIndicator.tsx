/**
 * ScoreIndicator Component (T060, FR-304)
 *
 * Displays content score status and forgetting candidate warnings.
 *
 * Features:
 * - Score percentage bar
 * - "Forgetting Candidate" warning for low-score content
 * - "Content Unavailable" message when shares < threshold
 */

'use client';

import { useEffect, useState } from 'react';
import { AlertTriangle, Info, XCircle } from 'lucide-react';

export interface ScoreIndicatorProps {
  /** Content hash (hex string) */
  contentHash: string;
  /** Current content score (0-1000+) */
  score?: number;
  /** Number of available shares for this content */
  availableShares?: number;
  /** Whether content is marked as forgetting candidate */
  isForgettingCandidate?: boolean;
  /** Minimum shares required for recovery (default: 3) */
  recoveryThreshold?: number;
  /** Score threshold for rewards (default: 100) */
  scoreThreshold?: number;
  /** Compact mode for timeline display */
  compact?: boolean;
}

type ContentStatus = 'healthy' | 'warning' | 'unavailable';

export function ScoreIndicator({
  contentHash,
  score = 1000,
  availableShares,
  isForgettingCandidate = false,
  recoveryThreshold = 3,
  scoreThreshold = 100,
  compact = false,
}: ScoreIndicatorProps) {
  const [status, setStatus] = useState<ContentStatus>('healthy');

  useEffect(() => {
    // Determine status based on available shares and score
    if (availableShares !== undefined && availableShares < recoveryThreshold) {
      setStatus('unavailable');
    } else if (isForgettingCandidate || score < scoreThreshold) {
      setStatus('warning');
    } else {
      setStatus('healthy');
    }
  }, [availableShares, recoveryThreshold, isForgettingCandidate, score, scoreThreshold]);

  // Calculate score percentage (0-100)
  const scorePercentage = Math.min(100, Math.max(0, (score / 1000) * 100));

  if (compact) {
    return (
      <CompactIndicator
        status={status}
        score={score}
        scorePercentage={scorePercentage}
        scoreThreshold={scoreThreshold}
      />
    );
  }

  return (
    <div className="score-indicator">
      {/* Status Messages */}
      {status === 'unavailable' && (
        <div
          role="alert"
          className="flex items-center gap-2 p-3 bg-red-100 dark:bg-red-900/30 border border-red-300 dark:border-red-700 rounded-lg text-red-800 dark:text-red-200"
        >
          <XCircle className="w-5 h-5" />
          <span>このコンテンツは利用できなくなりました</span>
        </div>
      )}

      {status === 'warning' && (
        <div
          role="alert"
          className="warning flex items-center gap-2 p-3 bg-yellow-100 dark:bg-yellow-900/30 border border-yellow-300 dark:border-yellow-700 rounded-lg text-yellow-800 dark:text-yellow-200"
        >
          <AlertTriangle className="w-5 h-5" />
          <div>
            <span>忘却候補</span>
            <span className="ml-2 text-sm opacity-75">
              スコアが低いため、まもなく利用できなくなる可能性があります
            </span>
          </div>
        </div>
      )}

      {/* Score Bar */}
      <div className="mt-2">
        <div className="flex justify-between text-sm mb-1">
          <span className="text-gray-600 dark:text-gray-400">コンテンツスコア</span>
          <span className="font-mono">{score}</span>
        </div>
        <div className="w-full h-2 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
          <div
            role="progressbar"
            aria-valuenow={score}
            aria-valuemin={0}
            aria-valuemax={1000}
            className={`h-full transition-all duration-300 ${getScoreColor(score, scoreThreshold)}`}
            style={{ width: `${scorePercentage}%` }}
          />
        </div>
        {score < scoreThreshold && (
          <div className="flex items-center gap-1 mt-1 text-xs text-yellow-600 dark:text-yellow-400">
            <Info className="w-3 h-3" />
            <span>閾値 ({scoreThreshold}) 未満のため報酬対象外</span>
          </div>
        )}
      </div>

      {/* Share Count (if available) */}
      {availableShares !== undefined && (
        <div className="mt-2 text-sm text-gray-600 dark:text-gray-400">
          利用可能シェア: {availableShares} / {recoveryThreshold} 必要
        </div>
      )}
    </div>
  );
}

function CompactIndicator({
  status,
  score,
  scorePercentage,
  scoreThreshold,
}: {
  status: ContentStatus;
  score: number;
  scorePercentage: number;
  scoreThreshold: number;
}) {
  const statusIcon = {
    healthy: null,
    warning: <AlertTriangle className="w-3 h-3 text-yellow-500" />,
    unavailable: <XCircle className="w-3 h-3 text-red-500" />,
  };

  return (
    <div className="flex items-center gap-2">
      {statusIcon[status]}
      <div className="w-16 h-1 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
        <div
          className={`h-full ${getScoreColor(score, scoreThreshold)}`}
          style={{ width: `${scorePercentage}%` }}
        />
      </div>
      <span className="text-xs font-mono text-gray-500">{score}</span>
    </div>
  );
}

function getScoreColor(score: number, threshold: number): string {
  if (score < threshold) {
    return 'bg-red-500';
  }
  if (score < threshold * 2) {
    return 'bg-yellow-500';
  }
  if (score < threshold * 5) {
    return 'bg-green-400';
  }
  return 'bg-green-500';
}

export default ScoreIndicator;
