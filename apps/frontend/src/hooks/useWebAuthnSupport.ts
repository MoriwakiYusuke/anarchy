/**
 * useWebAuthnSupport Hook
 * 
 * Detects WebAuthn support and platform authenticator availability.
 * This is a lightweight hook for feature gating.
 */

import { useState, useEffect, useCallback } from 'react';

export interface UseWebAuthnSupportResult {
  /** WebAuthn API available */
  isSupported: boolean;
  /** Platform authenticator available (Touch ID, Face ID, Windows Hello) */
  hasPlatformAuthenticator: boolean | null;
  /** Conditional UI available (autofill) */
  hasConditionalUI: boolean | null;
  /** Check performed */
  isChecked: boolean;
  /** Check in progress */
  isChecking: boolean;
  /** Error during check */
  error: Error | null;
  /** Recheck (useful if user plugs in a security key) */
  recheck: () => Promise<void>;
}

/**
 * Check if WebAuthn is supported in the current browser
 */
export function isWebAuthnSupported(): boolean {
  return (
    typeof window !== 'undefined' &&
    window.PublicKeyCredential !== undefined &&
    typeof window.PublicKeyCredential === 'function'
  );
}

/**
 * Check if a platform authenticator is available
 * (e.g., Touch ID, Face ID, Windows Hello)
 */
export async function isPlatformAuthenticatorAvailable(): Promise<boolean> {
  if (!isWebAuthnSupported()) {
    return false;
  }
  
  try {
    return await PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable();
  } catch {
    return false;
  }
}

/**
 * Check if conditional UI (autofill) is available
 */
export async function isConditionalUIAvailable(): Promise<boolean> {
  if (!isWebAuthnSupported()) {
    return false;
  }
  
  // Check if the browser supports conditional UI
  const pkc = PublicKeyCredential as unknown as {
    isConditionalMediationAvailable?: () => Promise<boolean>;
  };
  
  if (typeof pkc.isConditionalMediationAvailable !== 'function') {
    return false;
  }
  
  try {
    return await pkc.isConditionalMediationAvailable();
  } catch {
    return false;
  }
}

/**
 * Hook to detect WebAuthn support and platform authenticator availability
 * 
 * @example
 * ```tsx
 * function WebAuthnGate({ children }) {
 *   const { isSupported, hasPlatformAuthenticator, isChecking } = useWebAuthnSupport();
 * 
 *   if (isChecking) return <Loading />;
 *   if (!isSupported) return <UnsupportedBrowserMessage />;
 *   if (!hasPlatformAuthenticator) return <NoAuthenticatorMessage />;
 * 
 *   return children;
 * }
 * ```
 */
export function useWebAuthnSupport(): UseWebAuthnSupportResult {
  const [isSupported, setIsSupported] = useState<boolean>(false);
  const [hasPlatformAuthenticator, setHasPlatformAuthenticator] = useState<boolean | null>(null);
  const [hasConditionalUI, setHasConditionalUI] = useState<boolean | null>(null);
  const [isChecked, setIsChecked] = useState<boolean>(false);
  const [isChecking, setIsChecking] = useState<boolean>(true);
  const [error, setError] = useState<Error | null>(null);

  const performCheck = useCallback(async () => {
    setIsChecking(true);
    setError(null);
    
    try {
      // Check basic WebAuthn support
      const supported = isWebAuthnSupported();
      setIsSupported(supported);
      
      if (!supported) {
        setHasPlatformAuthenticator(false);
        setHasConditionalUI(false);
        setIsChecked(true);
        setIsChecking(false);
        return;
      }
      
      // Check platform authenticator availability
      const [platformAuthAvailable, conditionalAvailable] = await Promise.all([
        isPlatformAuthenticatorAvailable(),
        isConditionalUIAvailable(),
      ]);
      
      setHasPlatformAuthenticator(platformAuthAvailable);
      setHasConditionalUI(conditionalAvailable);
      setIsChecked(true);
    } catch (e) {
      setError(e instanceof Error ? e : new Error(String(e)));
      setHasPlatformAuthenticator(null);
      setHasConditionalUI(null);
    } finally {
      setIsChecking(false);
    }
  }, []);

  // Perform initial check on mount
  useEffect(() => {
    performCheck();
  }, [performCheck]);

  return {
    isSupported,
    hasPlatformAuthenticator,
    hasConditionalUI,
    isChecked,
    isChecking,
    error,
    recheck: performCheck,
  };
}

export default useWebAuthnSupport;
