/**
 * Matrix Background Type Definitions
 * 
 * Types for the cMatrix-style falling character animation (Blood Glitch theme)
 */

/**
 * Configuration for the matrix animation
 */
export interface MatrixConfig {
  // Colors
  mainColor: string;       // Default: '#333333' - Main falling character color (dark gray)
  headColor: string;       // Default: '#999999' - Leading character color (light gray)
  glitchColor: string;     // Default: '#CC0000' - Blood Glitch color (red)
  trailAlpha: number;      // Default: 0.05 - Trail fade opacity

  // Animation
  intervalMs: number;      // Default: 80 - Update interval in milliseconds
  glitchProbability: number; // Default: 0.02 - Probability of glitch effect (2%)

  // Characters
  charset: string;         // Characters to use for the animation
  fontSize: number;        // Default: 16 - Font size in pixels
  streamLength: number;    // Default: 15 - Length of each falling stream
  columnGap: number;       // Default: 2 - Gap between columns (multiplier of fontSize)

  // Performance
  enabled: boolean;        // Whether animation is active
}

/**
 * State for each column of falling characters
 */
export interface MatrixColumn {
  x: number;              // X coordinate in pixels
  y: number;              // Current Y position in pixels
  speed: number;          // Fall speed (1-3 range)
  chars: string[];        // Characters in this column
  glitchIndex?: number;   // Index of glitch character (if any)
}

/**
 * Canvas context for matrix rendering (used internally)
 */
export interface MatrixCanvasContext extends CanvasRenderingContext2D {
  canvas: HTMLCanvasElement;
}

/**
 * Props for MatrixBackground component
 */
export interface MatrixBackgroundProps {
  config?: Partial<MatrixConfig>;
  disabled?: boolean;
}
