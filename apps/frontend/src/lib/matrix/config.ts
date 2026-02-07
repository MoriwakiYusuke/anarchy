/**
 * Matrix Background Configuration Constants
 * 
 * Default configuration for the Blood Glitch theme cMatrix animation
 */

import type { MatrixConfig } from './types';

/**
 * Alphanumeric + symbols character set for Matrix rain effect
 */
export const MATRIX_CHARSET = 
  'ABCDEFGHIJKLMNOPQRSTUVWXYZ' +
  'abcdefghijklmnopqrstuvwxyz' +
  '0123456789' +
  '!@#$%^&*()-_=+[]{}|;:,.<>?/~`';

/**
 * Default matrix configuration (Blood Glitch theme)
 */
export const DEFAULT_MATRIX_CONFIG: MatrixConfig = {
  // Colors - Blood Glitch theme
  mainColor: '#333333',     // Dark gray for main characters
  headColor: '#CC0000',     // Red for leading character (Blood Glitch head)
  glitchColor: '#00cc0a',   // Green for glitch effect
  trailAlpha: 0.15,         // Trail fade opacity (higher = faster fade)

  // Animation timing
  intervalMs: 100,          // Slower for readable text
  glitchProbability: 0.0005, // 0.5% chance of red glitch (subtle)

  // Characters
  charset: MATRIX_CHARSET,
  fontSize: 16,             // Slightly larger for readability
  streamLength: 12,         // Length of each falling stream
  columnGap: 1.5,           // Gap between columns (less dense)

  // Performance
  enabled: true,
};

/**
 * Minimum interval to prevent performance issues
 */
export const MIN_INTERVAL_MS = 50;

/**
 * Maximum interval for reasonable animation speed
 */
export const MAX_INTERVAL_MS = 80;

/**
 * Clamp interval to valid range
 */
export function clampInterval(intervalMs: number): number {
  return Math.max(MIN_INTERVAL_MS, Math.min(MAX_INTERVAL_MS, intervalMs));
}

/**
 * Clamp glitch probability to valid range (0-1)
 */
export function clampGlitchProbability(probability: number): number {
  return Math.max(0, Math.min(1, probability));
}

/**
 * Merge partial config with defaults
 */
export function mergeConfig(partial?: Partial<MatrixConfig>): MatrixConfig {
  if (!partial) return DEFAULT_MATRIX_CONFIG;
  
  return {
    ...DEFAULT_MATRIX_CONFIG,
    ...partial,
    intervalMs: clampInterval(partial.intervalMs ?? DEFAULT_MATRIX_CONFIG.intervalMs),
    glitchProbability: clampGlitchProbability(
      partial.glitchProbability ?? DEFAULT_MATRIX_CONFIG.glitchProbability
    ),
  };
}
