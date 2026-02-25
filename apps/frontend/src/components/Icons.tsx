'use client'

/**
 * Icon Components
 * 
 * SVG icons for consistent UI styling
 */

import React from 'react'

interface IconProps {
  size?: number
  className?: string
  color?: string
}

export function CopyIcon({ size = 14, className, color = 'currentColor' }: IconProps) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke={color}
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      className={className}
    >
      <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
      <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
    </svg>
  )
}

export function CheckIcon({ size = 14, className, color = 'currentColor' }: IconProps) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke={color}
      strokeWidth="2.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      className={className}
    >
      <polyline points="20 6 9 17 4 12" />
    </svg>
  )
}

export function ReplyIcon({ size = 12, className, color = 'currentColor' }: IconProps) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke={color}
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      className={className}
    >
      <polyline points="9 17 4 12 9 7" />
      <path d="M20 18v-2a4 4 0 0 0-4-4H4" />
    </svg>
  )
}

export function ConnectedDot({ size = 8, className, color = '#4ade80' }: IconProps) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 8 8"
      className={className}
    >
      <circle cx="4" cy="4" r="4" fill={color} />
    </svg>
  )
}

export function SyncingDot({ size = 8, className, color = '#facc15' }: IconProps) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 8 8"
      className={className}
    >
      <circle cx="4" cy="4" r="3" fill="none" stroke={color} strokeWidth="1.5" />
      <path d="M4 1 A3 3 0 0 1 7 4" fill={color} />
    </svg>
  )
}

export function DisconnectedDot({ size = 8, className, color = '#888' }: IconProps) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 8 8"
      className={className}
    >
      <circle cx="4" cy="4" r="3" fill="none" stroke={color} strokeWidth="1.5" />
    </svg>
  )
}

export function WarningIcon({ size = 16, className, color = '#facc15' }: IconProps) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      className={className}
    >
      <path
        d="M12 2L1 21h22L12 2z"
        fill={color}
        stroke={color}
        strokeWidth="2"
        strokeLinejoin="round"
      />
      <path
        d="M12 9v4M12 17h.01"
        stroke="#000"
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  )
}
