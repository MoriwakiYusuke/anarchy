'use client';

/**
 * MatrixBackground Component
 * 
 * Renders cMatrix-style falling character animation as a full-screen background.
 * Uses Blood Glitch theme (gray with occasional red glitch characters).
 */

import React, { useRef, useEffect, useCallback } from 'react';
import { MatrixAnimationEngine, DEFAULT_MATRIX_CONFIG } from '@/lib/matrix';
import type { MatrixConfig } from '@/lib/matrix';
import { useReducedMotion } from '@/hooks/useReducedMotion';
import styles from './MatrixBackground.module.css';

export interface MatrixBackgroundProps {
  /** Enable/disable animation */
  enabled?: boolean;
  /** Custom configuration overrides */
  config?: Partial<MatrixConfig>;
  /** Respect prefers-reduced-motion setting */
  respectReducedMotion?: boolean;
  /** Additional CSS class */
  className?: string;
}

/**
 * MatrixBackground Component
 * 
 * @example
 * ```tsx
 * <MatrixBackground enabled respectReducedMotion />
 * ```
 */
export function MatrixBackground({
  enabled = true,
  config,
  respectReducedMotion = true,
  className = '',
}: MatrixBackgroundProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const engineRef = useRef<MatrixAnimationEngine | null>(null);
  const prefersReducedMotion = useReducedMotion();

  // Determine if animation should run
  const shouldAnimate = enabled && !(respectReducedMotion && prefersReducedMotion);

  // Handle window resize
  const handleResize = useCallback(() => {
    if (canvasRef.current && engineRef.current) {
      engineRef.current.resize(window.innerWidth, window.innerHeight);
    }
  }, []);

  // Initialize and manage animation engine
  useEffect(() => {
    if (!canvasRef.current) return;

    const canvas = canvasRef.current;
    
    // Set initial size (full screen)
    canvas.width = window.innerWidth;
    canvas.height = window.innerHeight;

    // Create engine if needed
    if (!engineRef.current) {
      engineRef.current = new MatrixAnimationEngine({
        ...DEFAULT_MATRIX_CONFIG,
        ...config,
      });
      engineRef.current.initialize(canvas);
    }

    // Start/stop based on shouldAnimate
    if (shouldAnimate) {
      engineRef.current.start();
    } else {
      engineRef.current.stop();
    }

    // Add resize listener
    window.addEventListener('resize', handleResize);

    // Cleanup
    return () => {
      window.removeEventListener('resize', handleResize);
    };
  }, [shouldAnimate, config, handleResize]);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (engineRef.current) {
        engineRef.current.destroy();
        engineRef.current = null;
      }
    };
  }, []);

  // Render static fallback for reduced motion
  if (respectReducedMotion && prefersReducedMotion) {
    return (
      <div 
        className={`${styles.background} ${styles.static} ${className}`}
        aria-hidden="true"
        role="presentation"
      />
    );
  }

  return (
    <canvas
      ref={canvasRef}
      className={`${styles.background} ${className}`}
      aria-hidden="true"
      role="presentation"
    />
  );
}

export default MatrixBackground;
