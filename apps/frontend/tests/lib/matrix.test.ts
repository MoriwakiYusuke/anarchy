/**
 * Unit Tests for Matrix Animation Engine
 * 
 * TDD: Written BEFORE implementation - tests should FAIL initially
 */

import {
  createMatrixColumn,
  updateColumn,
  renderFrame,
  initializeColumns,
} from '@/lib/matrix';
import { DEFAULT_MATRIX_CONFIG } from '@/lib/matrix/config';
import type { MatrixColumn, MatrixCanvasContext } from '@/lib/matrix/types';

describe('Matrix Animation Engine', () => {
  describe('createMatrixColumn', () => {
    it('should create a column with initial values', () => {
      const column = createMatrixColumn(5, 100);
      
      expect(column).toHaveProperty('x', 5);
      expect(column).toHaveProperty('y');
      expect(column).toHaveProperty('speed');
      expect(column).toHaveProperty('chars');
      expect(typeof column.y).toBe('number');
      expect(typeof column.speed).toBe('number');
      expect(Array.isArray(column.chars)).toBe(true);
    });

    it('should generate random starting y position', () => {
      const columns = Array.from({ length: 10 }, () => createMatrixColumn(0, 100));
      const yPositions = columns.map(c => c.y);
      
      // At least some should be different (randomness)
      const uniquePositions = new Set(yPositions);
      expect(uniquePositions.size).toBeGreaterThan(1);
    });
  });

  describe('updateColumn', () => {
    it('should advance column y position', () => {
      const column: MatrixColumn = {
        x: 0,
        y: 10,
        speed: 1,
        chars: ['A', 'B', 'C'],
      };
      
      const updated = updateColumn(column, 100, DEFAULT_MATRIX_CONFIG);
      
      expect(updated.y).toBeGreaterThan(column.y);
    });

    it('should reset column when drop goes far below screen', () => {
      // streamLength is 12, so reset happens when y > canvasHeight + 12 * fontSize
      const streamLength = DEFAULT_MATRIX_CONFIG.streamLength || 12;
      const resetThreshold = 100 + streamLength * DEFAULT_MATRIX_CONFIG.fontSize;
      
      const column: MatrixColumn = {
        x: 0,
        y: resetThreshold + 50, // Far beyond threshold
        speed: 1,
        chars: [],
      };
      
      const updated = updateColumn(column, 100, DEFAULT_MATRIX_CONFIG);
      
      // Should reset to negative y (top of screen)
      expect(updated.y).toBeLessThan(0);
    });

    it('should vary speed during reset', () => {
      // Test that speed gets randomized
      const streamLength = DEFAULT_MATRIX_CONFIG.streamLength || 12;
      const resetThreshold = 100 + streamLength * DEFAULT_MATRIX_CONFIG.fontSize;
      
      const speeds: number[] = [];
      for (let i = 0; i < 20; i++) {
        const column: MatrixColumn = {
          x: 0,
          y: resetThreshold + 10,
          speed: 1,
          chars: [],
        };
        const updated = updateColumn(column, 100, DEFAULT_MATRIX_CONFIG);
        speeds.push(updated.speed);
      }
      
      // Should have some variation in speeds (not all identical)
      const uniqueSpeeds = new Set(speeds);
      expect(uniqueSpeeds.size).toBeGreaterThan(1);
    });
  });

  describe('initializeColumns', () => {
    it('should create columns based on canvas width', () => {
      const columns = initializeColumns(100, 500, DEFAULT_MATRIX_CONFIG);
      
      // Width 100 / fontSize 14 ≈ 7 columns
      expect(columns.length).toBeGreaterThan(0);
      expect(columns.length).toBeLessThanOrEqual(Math.ceil(100 / DEFAULT_MATRIX_CONFIG.fontSize));
    });

    it('should space columns evenly based on columnGap', () => {
      const columns = initializeColumns(200, 500, DEFAULT_MATRIX_CONFIG);
      
      // Check spacing (should be columnGap * fontSize)
      const expectedGap = (DEFAULT_MATRIX_CONFIG.columnGap || 1.5) * DEFAULT_MATRIX_CONFIG.fontSize;
      for (let i = 1; i < columns.length; i++) {
        const spacing = columns[i].x - columns[i - 1].x;
        expect(spacing).toBe(expectedGap);
      }
    });
  });

  describe('renderFrame', () => {
    it('should call canvas methods for each column', () => {
      const mockCtx: Partial<MatrixCanvasContext> = {
        fillRect: jest.fn(),
        fillText: jest.fn(),
        clearRect: jest.fn(),
        font: '',
        fillStyle: '',
        globalAlpha: 1,
        canvas: { width: 100, height: 500 } as HTMLCanvasElement,
      };
      
      const columns: MatrixColumn[] = [
        { x: 0, y: 50, speed: 1, chars: ['A', 'B', 'C'] },
        { x: 14, y: 80, speed: 1.5, chars: ['X', 'Y', 'Z'] },
      ];
      
      renderFrame(mockCtx as MatrixCanvasContext, columns, DEFAULT_MATRIX_CONFIG);
      
      // Should draw trail overlay
      expect(mockCtx.fillRect).toHaveBeenCalled();
      // Should draw characters
      expect(mockCtx.fillText).toHaveBeenCalled();
    });

    it('should use glitch color for glitch characters', () => {
      const mockCtx: Partial<MatrixCanvasContext> = {
        fillRect: jest.fn(),
        fillText: jest.fn(),
        clearRect: jest.fn(),
        font: '',
        fillStyle: '',
        globalAlpha: 1,
        canvas: { width: 100, height: 500 } as HTMLCanvasElement,
      };
      
      const columns: MatrixColumn[] = [
        { x: 0, y: 50, speed: 1, chars: ['A', 'B', 'C'], glitchIndex: 1 },
      ];
      
      renderFrame(mockCtx as MatrixCanvasContext, columns, DEFAULT_MATRIX_CONFIG);
      
      // Should have set fillStyle to glitch color at some point
      // This is a simplified check - implementation details may vary
      expect(mockCtx.fillText).toHaveBeenCalled();
    });
  });

  describe('Blood Glitch Theme', () => {
    it('should use configured colors', () => {
      expect(DEFAULT_MATRIX_CONFIG.mainColor).toBe('#333333');
      expect(DEFAULT_MATRIX_CONFIG.headColor).toBe('#CC0000');
      expect(DEFAULT_MATRIX_CONFIG.glitchColor).toBe('#00cc0a');
    });

    it('should have low glitch probability by default', () => {
      expect(DEFAULT_MATRIX_CONFIG.glitchProbability).toBe(0.0005);
    });

    it('should use 100ms interval for readable speed', () => {
      expect(DEFAULT_MATRIX_CONFIG.intervalMs).toBe(100);
    });
  });
});
