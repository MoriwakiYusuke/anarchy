/**
 * Skeleton Component
 * 
 * T-073: Loading skeleton UI component
 * 
 * Features:
 * - Multiple variants (text, avatar, media, card)
 * - Shimmer animation
 * - Customizable dimensions
 */

'use client'

import React from 'react'
import styles from './Skeleton.module.css'

export type SkeletonVariant = 'text' | 'avatar' | 'media' | 'card' | 'button'

export interface SkeletonProps {
  /** Skeleton variant */
  variant?: SkeletonVariant
  /** Width (px or CSS value) */
  width?: number | string
  /** Height (px or CSS value) */
  height?: number | string
  /** Custom class name */
  className?: string
  /** Number of text lines (for text variant) */
  lines?: number
  /** Whether to show animation */
  animate?: boolean
}

export default function Skeleton({
  variant = 'text',
  width,
  height,
  className,
  lines = 1,
  animate = true,
}: SkeletonProps): React.ReactElement {
  const baseClasses = [
    styles.skeleton,
    styles[variant],
    animate && styles.animate,
    className,
  ].filter(Boolean).join(' ')

  const style: React.CSSProperties = {
    ...(width && { width: typeof width === 'number' ? `${width}px` : width }),
    ...(height && { height: typeof height === 'number' ? `${height}px` : height }),
  }

  // For text variant with multiple lines
  if (variant === 'text' && lines > 1) {
    return (
      <div className={styles.textGroup}>
        {Array.from({ length: lines }).map((_, index) => (
          <div
            key={index}
            className={baseClasses}
            style={{
              ...style,
              width: index === lines - 1 ? '60%' : width || '100%',
            }}
          />
        ))}
      </div>
    )
  }

  return <div className={baseClasses} style={style} />
}

// Export individual skeleton variants for convenience
export function TextSkeleton(props: Omit<SkeletonProps, 'variant'>) {
  return <Skeleton {...props} variant="text" />
}

export function AvatarSkeleton(props: Omit<SkeletonProps, 'variant'>) {
  return <Skeleton {...props} variant="avatar" />
}

export function MediaSkeleton(props: Omit<SkeletonProps, 'variant'>) {
  return <Skeleton {...props} variant="media" />
}

export function CardSkeleton(props: Omit<SkeletonProps, 'variant'>) {
  return <Skeleton {...props} variant="card" />
}

export function ButtonSkeleton(props: Omit<SkeletonProps, 'variant'>) {
  return <Skeleton {...props} variant="button" />
}
