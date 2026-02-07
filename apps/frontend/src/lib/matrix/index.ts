/**
 * Matrix Animation Engine
 * 
 * Implements cMatrix-style falling character animation with Blood Glitch theme.
 * Each column has a string of characters that falls together as a unit.
 */

import type { MatrixConfig, MatrixColumn, MatrixCanvasContext } from './types';
import { DEFAULT_MATRIX_CONFIG, MATRIX_CHARSET, mergeConfig } from './config';

/**
 * Get a random character from the charset
 */
function getRandomChar(charset: string = MATRIX_CHARSET): string {
  return charset[Math.floor(Math.random() * charset.length)];
}

/**
 * Generate an array of random characters (a string to fall)
 */
function generateChars(length: number, charset: string): string[] {
  return Array.from({ length }, () => getRandomChar(charset));
}

/**
 * Create a new matrix column with a character string
 */
export function createMatrixColumn(x: number, canvasHeight: number, config: MatrixConfig = DEFAULT_MATRIX_CONFIG, startOffScreen: boolean = true): MatrixColumn {
  const streamLength = config.streamLength || 12;
  const speed = 0.3 + Math.random() * 0.5;
  
  // Start position: stagger entry by starting at different heights above screen
  const startY = startOffScreen 
    ? -Math.random() * canvasHeight * 1.5
    : -streamLength * config.fontSize;
  
  return {
    x,
    y: startY,
    speed,
    chars: generateChars(streamLength, config.charset), // The string that falls
  };
}

/**
 * Update a column's drop position
 */
export function updateColumn(
  column: MatrixColumn, 
  canvasHeight: number, 
  config: MatrixConfig
): MatrixColumn {
  const streamLength = config.streamLength || 12;
  const newY = column.y + column.speed * config.fontSize;
  
  // Reset if drop has gone far below screen
  if (newY > canvasHeight + streamLength * config.fontSize) {
    return createMatrixColumn(column.x, canvasHeight, config, false);
  }
  
  return {
    ...column,
    y: newY,
  };
}

/**
 * Initialize all columns for the canvas
 */
export function initializeColumns(
  canvasWidth: number, 
  canvasHeight: number, 
  config: MatrixConfig
): MatrixColumn[] {
  const gap = (config.columnGap || 1.5) * config.fontSize;
  const columnCount = Math.ceil(canvasWidth / gap);
  const columns: MatrixColumn[] = [];
  
  for (let i = 0; i < columnCount; i++) {
    columns.push(createMatrixColumn(i * gap, canvasHeight, config, true));
  }
  
  return columns;
}

/**
 * Render a single frame of the matrix animation
 * Characters in each column change randomly each frame (cMatrix style)
 * but the column/stream position moves together
 */
export function renderFrame(
  ctx: MatrixCanvasContext, 
  columns: MatrixColumn[], 
  config: MatrixConfig
): MatrixColumn[] {
  const { width, height } = ctx.canvas;
  const streamLength = config.streamLength || 12;
  
  // Draw semi-transparent black overlay for trail effect
  ctx.fillStyle = `rgba(0, 0, 0, ${config.trailAlpha})`;
  ctx.fillRect(0, 0, width, height);
  
  // Set font
  ctx.font = `${config.fontSize}px monospace`;
  
  // Update and render each column
  const updatedColumns = columns.map((column) => {
    const updated = updateColumn(column, height, config);
    
    // Render the character string: head at y, rest going upward  
    for (let i = 0; i < updated.chars.length; i++) {
      const charY = Math.floor(updated.y) - i * config.fontSize;
      
      // Skip if off-screen
      if (charY < -config.fontSize || charY > height + config.fontSize) {
        continue;
      }
      
      // Each character changes randomly every frame (cMatrix style)
      const char = getRandomChar(config.charset);
      
      // Determine character color with fade effect
      if (i === 0) {
        // Head character (brightest)
        ctx.fillStyle = config.headColor;
      } else if (Math.random() < config.glitchProbability) {
        // Blood glitch (random chance)
        ctx.fillStyle = config.glitchColor;
      } else {
        // Fade out based on position in stream
        const fadeRatio = 1 - (i / streamLength);
        const alpha = Math.max(0.1, fadeRatio * 0.7);
        ctx.fillStyle = hexToRgba(config.mainColor, alpha);
      }
      
      ctx.fillText(char, updated.x, charY);
    }
    
    return updated;
  });
  
  return updatedColumns;
}

/**
 * Convert hex color to rgba with alpha
 */
function hexToRgba(hex: string, alpha: number): string {
  const r = parseInt(hex.slice(1, 3), 16);
  const g = parseInt(hex.slice(3, 5), 16);
  const b = parseInt(hex.slice(5, 7), 16);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

/**
 * Matrix animation controller class
 */
export class MatrixAnimationEngine {
  private ctx: MatrixCanvasContext | null = null;
  private columns: MatrixColumn[] = [];
  private config: MatrixConfig;
  private intervalId: ReturnType<typeof setInterval> | null = null;
  private isRunning = false;

  constructor(config?: Partial<MatrixConfig>) {
    this.config = mergeConfig(config);
  }

  /**
   * Initialize the engine with a canvas context
   */
  initialize(canvas: HTMLCanvasElement): void {
    const ctx = canvas.getContext('2d');
    if (!ctx) {
      throw new Error('Failed to get 2d context from canvas');
    }
    
    this.ctx = ctx as MatrixCanvasContext;
    this.columns = initializeColumns(canvas.width, canvas.height, this.config);
  }

  /**
   * Start the animation
   */
  start(): void {
    if (this.isRunning || !this.ctx) return;
    
    this.isRunning = true;
    this.intervalId = setInterval(() => {
      if (this.ctx) {
        this.columns = renderFrame(this.ctx, this.columns, this.config);
      }
    }, this.config.intervalMs);
  }

  /**
   * Stop the animation
   */
  stop(): void {
    if (this.intervalId) {
      clearInterval(this.intervalId);
      this.intervalId = null;
    }
    this.isRunning = false;
  }

  /**
   * Resize handler - reinitialize columns
   */
  resize(width: number, height: number): void {
    if (!this.ctx) return;
    
    this.ctx.canvas.width = width;
    this.ctx.canvas.height = height;
    this.columns = initializeColumns(width, height, this.config);
  }

  /**
   * Update configuration
   */
  updateConfig(config: Partial<MatrixConfig>): void {
    this.config = mergeConfig(config);
  }

  /**
   * Check if animation is running
   */
  get running(): boolean {
    return this.isRunning;
  }

  /**
   * Cleanup
   */
  destroy(): void {
    this.stop();
    this.ctx = null;
    this.columns = [];
  }
}

// Re-export types and config
export type { MatrixConfig, MatrixColumn, MatrixCanvasContext } from './types';
export { DEFAULT_MATRIX_CONFIG, MATRIX_CHARSET, mergeConfig } from './config';
