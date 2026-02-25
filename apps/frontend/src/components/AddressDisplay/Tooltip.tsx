'use client'

/**
 * Tooltip Subcomponent
 * 
 * T-035: Tooltip subcomponent for AddressDisplay
 */

import React, { ReactNode } from 'react'
import styles from './Tooltip.module.css'

export interface TooltipProps {
  children: ReactNode
  position?: 'top' | 'bottom' | 'left' | 'right'
}

/**
 * Simple tooltip component that renders above the parent element
 */
export default function Tooltip({
  children,
  position = 'top',
}: TooltipProps) {
  return (
    <div 
      className={`${styles.tooltip} ${styles[position]}`}
      role="tooltip"
    >
      <div className={styles.content}>
        {children}
      </div>
      <div className={styles.arrow} />
    </div>
  )
}
