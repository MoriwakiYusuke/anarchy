/**
 * ProgressBar Component
 * 
 * T-055: Progress bar subcomponent for uploads
 */

'use client'

import React from 'react'
import styles from './ProgressBar.module.css'

export interface ProgressBarProps {
  /** Progress percentage (0-100) */
  progress: number
  /** Custom class name */
  className?: string
}

export default function ProgressBar({
  progress,
  className,
}: ProgressBarProps): React.ReactElement {
  const clampedProgress = Math.max(0, Math.min(100, progress))

  return (
    <div 
      className={`${styles.container} ${className || ''}`}
      role="progressbar"
      aria-valuenow={clampedProgress}
      aria-valuemin={0}
      aria-valuemax={100}
    >
      <div 
        className={styles.bar}
        style={{ width: `${clampedProgress}%` }}
      />
    </div>
  )
}
