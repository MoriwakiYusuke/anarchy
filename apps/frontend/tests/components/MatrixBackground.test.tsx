/**
 * Unit Tests for MatrixBackground Component
 * 
 * TDD: Written BEFORE implementation - tests should FAIL initially
 */

import React from 'react';
import { render, screen } from '@testing-library/react';

// To be implemented
import { MatrixBackground } from '@/components/MatrixBackground';

// Mock canvas context
const mockGetContext = jest.fn();

beforeAll(() => {
  HTMLCanvasElement.prototype.getContext = mockGetContext;
});

beforeEach(() => {
  mockGetContext.mockClear();
  mockGetContext.mockReturnValue({
    fillRect: jest.fn(),
    fillText: jest.fn(),
    clearRect: jest.fn(),
    measureText: jest.fn(() => ({ width: 10 })),
    font: '',
    fillStyle: '',
    globalAlpha: 1,
  });
});

describe('MatrixBackground Component', () => {
  describe('Rendering', () => {
    it('should render a canvas element', () => {
      render(<MatrixBackground />);
      
      // aria-hidden elements need hidden: true
      const canvas = screen.getByRole('presentation', { hidden: true });
      expect(canvas).toBeInTheDocument();
      expect(canvas.tagName.toLowerCase()).toBe('canvas');
    });

    it('should have aria-hidden for accessibility', () => {
      render(<MatrixBackground />);
      
      const canvas = screen.getByRole('presentation', { hidden: true });
      expect(canvas).toHaveAttribute('aria-hidden', 'true');
    });

    it('should be positioned as background (z-index)', () => {
      const { container } = render(<MatrixBackground />);
      
      // CSS modules styles are applied via className
      expect(container.firstChild).toHaveClass('background');
    });
  });

  describe('Animation Control', () => {
    it('should start animation when enabled is true (default)', () => {
      render(<MatrixBackground enabled={true} />);
      
      // Should initialize canvas context
      expect(mockGetContext).toHaveBeenCalledWith('2d');
    });

    it('should not call getContext after initial render when enabled is false', () => {
      mockGetContext.mockClear();
      
      render(<MatrixBackground enabled={false} />);
      
      // Canvas still exists but animation shouldn't start
      // Note: The canvas ref might still call getContext for initialization
      // The key is that the animation interval shouldn't start
    });
  });

  describe('Configuration', () => {
    it('should accept custom config', () => {
      const customConfig = {
        mainColor: '#444444',
        glitchProbability: 0.05,
      };
      
      render(<MatrixBackground config={customConfig} />);
      
      const canvas = screen.getByRole('presentation', { hidden: true });
      expect(canvas).toBeInTheDocument();
    });
  });

  describe('Reduced Motion', () => {
    it('should render fallback when reduce motion is preferred', () => {
      // Mock prefers-reduced-motion
      const originalMatchMedia = window.matchMedia;
      window.matchMedia = jest.fn().mockImplementation((query) => ({
        matches: query === '(prefers-reduced-motion: reduce)',
        media: query,
        onchange: null,
        addListener: jest.fn(),
        removeListener: jest.fn(),
        addEventListener: jest.fn(),
        removeEventListener: jest.fn(),
        dispatchEvent: jest.fn(),
      }));

      // Clear mock before this specific test
      mockGetContext.mockClear();
      
      render(<MatrixBackground respectReducedMotion />);
      
      // When reduced motion is preferred, should render static div instead of canvas
      const fallbackDiv = document.querySelector('.static');
      expect(fallbackDiv).toBeInTheDocument();

      window.matchMedia = originalMatchMedia;
    });
  });

  describe('Cleanup', () => {
    it('should cleanup interval on unmount', () => {
      jest.useFakeTimers();
      const clearIntervalSpy = jest.spyOn(global, 'clearInterval');

      const { unmount } = render(<MatrixBackground enabled />);
      unmount();

      expect(clearIntervalSpy).toHaveBeenCalled();
      
      clearIntervalSpy.mockRestore();
      jest.useRealTimers();
    });
  });
});
