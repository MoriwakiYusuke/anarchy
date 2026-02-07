'use client'

import { ReactNode } from 'react'
import { useWebAuthnSupport } from '../hooks/useWebAuthnSupport'
import styles from './WebAuthnGate.module.css'

export interface WebAuthnGateProps {
  /** Child components to render when WebAuthn is available */
  children: ReactNode
  /** Custom loading component */
  loadingComponent?: ReactNode
  /** Custom unsupported browser component */
  unsupportedComponent?: ReactNode
  /** Custom no authenticator component */
  noAuthenticatorComponent?: ReactNode
  /** Whether to require platform authenticator */
  requirePlatformAuthenticator?: boolean
}

/**
 * WebAuthnGate - Gates child content based on WebAuthn availability
 * 
 * Provides appropriate fallback UI when:
 * - WebAuthn is checking (shows loading)
 * - WebAuthn is not supported (shows unsupported message)
 * - Platform authenticator is not available (shows warning)
 * 
 * @example
 * ```tsx
 * function App() {
 *   return (
 *     <WebAuthnGate>
 *       <PasskeyRegister />
 *     </WebAuthnGate>
 *   )
 * }
 * ```
 */
export function WebAuthnGate({
  children,
  loadingComponent,
  unsupportedComponent,
  noAuthenticatorComponent,
  requirePlatformAuthenticator = true,
}: WebAuthnGateProps) {
  const { 
    isSupported, 
    hasPlatformAuthenticator, 
    isChecking 
  } = useWebAuthnSupport()

  // Show loading state
  if (isChecking) {
    return (
      loadingComponent ?? (
        <div className={styles.gateContainer}>
          <div className={styles.loading}>
            <div className={styles.spinner} />
            <p>認証機能を確認中...</p>
          </div>
        </div>
      )
    )
  }

  // WebAuthn not supported
  if (!isSupported) {
    return (
      unsupportedComponent ?? (
        <div className={styles.gateContainer}>
          <div className={styles.unsupported}>
            <div className={styles.icon}>🔒</div>
            <h3>パスキーに非対応</h3>
            <p>
              このブラウザはパスキー（WebAuthn）に対応していません。
              <br />
              最新のChrome、Safari、Firefox、またはEdgeをお使いください。
            </p>
            <ul className={styles.supportedBrowsers}>
              <li>Chrome 67以降</li>
              <li>Safari 14以降</li>
              <li>Firefox 60以降</li>
              <li>Edge 79以降</li>
            </ul>
          </div>
        </div>
      )
    )
  }

  // Platform authenticator not available (when required)
  if (requirePlatformAuthenticator && hasPlatformAuthenticator === false) {
    return (
      noAuthenticatorComponent ?? (
        <div className={styles.gateContainer}>
          <div className={styles.noAuthenticator}>
            <div className={styles.icon}>👆</div>
            <h3>生体認証が利用できません</h3>
            <p>
              このデバイスでは生体認証（Touch ID、Face ID、Windows Hello）
              が利用できないため、パスキー登録ができません。
            </p>
            <p className={styles.hint}>
              生体認証またはPINによるデバイスロックを設定してください。
            </p>
          </div>
        </div>
      )
    )
  }

  // All checks passed, render children
  return <>{children}</>
}

export default WebAuthnGate
