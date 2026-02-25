/**
 * PostSkeleton Component
 * 
 * T-073: Loading skeleton for post items in timeline
 */

'use client'

import React from 'react'
import { AvatarSkeleton, TextSkeleton, MediaSkeleton } from '@/components/Skeleton'
import styles from './PostSkeleton.module.css'

export interface PostSkeletonProps {
  /** Whether to show media skeleton */
  showMedia?: boolean
  /** Number of skeletons to render */
  count?: number
}

function SinglePostSkeleton({ showMedia = true }: { showMedia?: boolean }) {
  return (
    <div className={styles.post}>
      <div className={styles.header}>
        <AvatarSkeleton />
        <div className={styles.headerText}>
          <TextSkeleton width="30%" />
          <TextSkeleton width="20%" height={12} />
        </div>
      </div>
      <div className={styles.content}>
        <TextSkeleton lines={3} />
      </div>
      {showMedia && (
        <div className={styles.media}>
          <MediaSkeleton />
        </div>
      )}
      <div className={styles.actions}>
        <TextSkeleton width={60} height={24} />
        <TextSkeleton width={60} height={24} />
        <TextSkeleton width={60} height={24} />
      </div>
    </div>
  )
}

export default function PostSkeleton({
  showMedia = true,
  count = 3,
}: PostSkeletonProps): React.ReactElement {
  return (
    <div className={styles.container}>
      {Array.from({ length: count }).map((_, index) => (
        <SinglePostSkeleton key={index} showMedia={showMedia && index === 0} />
      ))}
    </div>
  )
}
