/**
 * useScore Hook (T061-T067, Issue 13 Fix)
 *
 * Fetches content score from on-chain ScoreCache storage using PAPI.
 * Returns default score when ScoreProvider is unavailable.
 *
 * FR-303: Display score in frontend when connected
 * FR-305: Skip score display when unavailable
 * FR-022: useScoreは実際のブロックチェーンからスコアデータを取得MUST
 */

'use client';

import { useState, useEffect, useCallback } from 'react';

// Default score when ScoreCache is empty or unavailable
const DEFAULT_SCORE = 1000;

// Score threshold for rewards (should match pallet config)
const SCORE_THRESHOLD = 100;

export interface UseScoreOptions {
  /** Content hash (Uint8Array[32] or number[32]) */
  contentHash: Uint8Array | number[];
  /** Polkadot API instance (from useApi hook) */
  unsafeApi?: unknown;
  /** Polling interval in ms (0 to disable) */
  pollInterval?: number;
}

export interface UseScoreResult {
  /** Current score (undefined while loading) */
  score: number | undefined;
  /** Whether score is being loaded */
  isLoading: boolean;
  /** Error if fetch failed */
  error: Error | null;
  /** Whether score provider is available */
  isProviderAvailable: boolean;
  /** Whether score is above reward threshold */
  isEligibleForReward: boolean;
  /** Refresh score */
  refresh: () => Promise<void>;
}

/**
 * Hook to fetch content score from on-chain ScoreCache
 *
 * @example
 * ```tsx
 * const { unsafeApi } = useApi();
 * const { score, isLoading, isProviderAvailable } = useScore({
 *   contentHash: new Uint8Array(32),
 *   unsafeApi,
 *   pollInterval: 30000, // Poll every 30s
 * });
 * ```
 */
export function useScore({
  contentHash,
  unsafeApi,
  pollInterval = 0,
}: UseScoreOptions): UseScoreResult {
  const [score, setScore] = useState<number | undefined>(undefined);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);
  const [isProviderAvailable, setIsProviderAvailable] = useState(true);

  const fetchScore = useCallback(async () => {
    if (!contentHash || contentHash.length !== 32) {
      setScore(undefined);
      setIsLoading(false);
      return;
    }

    if (!unsafeApi) {
      // API not available - return default
      setScore(DEFAULT_SCORE);
      setIsProviderAvailable(false);
      setIsLoading(false);
      return;
    }

    setIsLoading(true);
    setError(null);

    try {
      // Query ScoreCache from pallet-storage via PAPI
      const api = unsafeApi as {
        query: {
          Storage?: {
            ScoreCache?: {
              getValue: (hash: Uint8Array | number[]) => Promise<number | undefined>;
            };
          };
        };
      };

      // Check if Storage pallet and ScoreCache are available
      if (!api.query?.Storage?.ScoreCache) {
        // ScoreCache not available in runtime - use default
        setScore(DEFAULT_SCORE);
        setIsProviderAvailable(false);
        setIsLoading(false);
        return;
      }

      // Convert to Uint8Array if needed
      const hashBytes = contentHash instanceof Uint8Array 
        ? contentHash 
        : new Uint8Array(contentHash);

      // Fetch score from chain
      const chainScore = await api.query.Storage.ScoreCache.getValue(hashBytes);
      
      if (chainScore !== undefined && chainScore !== null) {
        // Score exists on-chain
        const scoreValue = typeof chainScore === 'number' ? chainScore : Number(chainScore);
        setScore(scoreValue);
        setIsProviderAvailable(true);
      } else {
        // No score cached - return default (ScoreProvider may not have run yet)
        setScore(DEFAULT_SCORE);
        setIsProviderAvailable(false);
      }
    } catch (err) {
      const error = err instanceof Error ? err : new Error('Failed to fetch score');
      setError(error);
      // Fall back to default score on error
      setScore(DEFAULT_SCORE);
      setIsProviderAvailable(false);
    } finally {
      setIsLoading(false);
    }
  }, [contentHash, unsafeApi]);

  // Initial fetch
  useEffect(() => {
    fetchScore();
  }, [fetchScore]);

  // Polling
  useEffect(() => {
    if (pollInterval <= 0) return;

    const interval = setInterval(fetchScore, pollInterval);
    return () => clearInterval(interval);
  }, [fetchScore, pollInterval]);

  return {
    score,
    isLoading,
    error,
    isProviderAvailable,
    isEligibleForReward: (score ?? DEFAULT_SCORE) >= SCORE_THRESHOLD,
    refresh: fetchScore,
  };
}

export default useScore;
