/**
 * useScore Hook (T066-T067)
 *
 * Fetches content score from on-chain ScoreCache storage.
 * Returns default score when ScoreProvider is unavailable.
 *
 * FR-303: Display score in frontend when connected
 * FR-305: Skip score display when unavailable
 */

'use client';

import { useState, useEffect, useCallback } from 'react';

// Default score when ScoreCache is empty or unavailable
const DEFAULT_SCORE = 1000;

// Score threshold for rewards (should match pallet config)
const SCORE_THRESHOLD = 100;

export interface UseScoreOptions {
  /** Content hash (hex string, 0x prefixed) */
  contentHash: string;
  /** Polling interval in ms (0 to disable) */
  pollInterval?: number;
  /** Custom RPC endpoint */
  rpcEndpoint?: string;
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
 * const { score, isLoading, isProviderAvailable } = useScore({
 *   contentHash: '0x1234...',
 *   pollInterval: 30000, // Poll every 30s
 * });
 * ```
 */
export function useScore({
  contentHash,
  pollInterval = 0,
  rpcEndpoint = 'ws://127.0.0.1:9944',
}: UseScoreOptions): UseScoreResult {
  const [score, setScore] = useState<number | undefined>(undefined);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);
  const [isProviderAvailable, setIsProviderAvailable] = useState(true);

  const fetchScore = useCallback(async () => {
    if (!contentHash) {
      setScore(undefined);
      setIsLoading(false);
      return;
    }

    setIsLoading(true);
    setError(null);

    try {
      // TODO: Replace with actual PAPI chain query
      // const client = createClient(getWsProvider(rpcEndpoint));
      // const api = client.getUnsafeApi();
      // const scoreEntry = await api.query.storage.scoreCache(contentHash);
      
      // For now, simulate checking if score provider is available
      const mockResponse = await simulateScoreFetch(contentHash, rpcEndpoint);
      
      if (mockResponse.available) {
        setScore(mockResponse.score);
        setIsProviderAvailable(true);
      } else {
        // ScoreProvider not available - use default
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
  }, [contentHash, rpcEndpoint]);

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

/**
 * Simulate score fetch (placeholder for actual chain query)
 */
async function simulateScoreFetch(
  contentHash: string,
  _rpcEndpoint: string
): Promise<{ available: boolean; score: number }> {
  // Simulate network delay
  await new Promise((resolve) => setTimeout(resolve, 100));

  // In production, this would query the chain:
  // const scoreEntry = await api.query.storage.scoreCache(contentHash);
  // if (scoreEntry.isSome) {
  //   return { available: true, score: scoreEntry.unwrap().toNumber() };
  // }
  // return { available: false, score: DEFAULT_SCORE };

  // For development, return mock data based on content hash
  const hashNum = parseInt(contentHash.slice(2, 6), 16);
  if (hashNum % 10 === 0) {
    // 10% chance of unavailable
    return { available: false, score: DEFAULT_SCORE };
  }
  
  // Return a "random" score based on hash
  const mockScore = 50 + (hashNum % 950);
  return { available: true, score: mockScore };
}

export default useScore;
