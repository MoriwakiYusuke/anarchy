'use client'

import { createContext, useContext, useState, useCallback, useEffect, ReactNode, useMemo } from 'react'
import { useWebAuthn, UseWebAuthnResult } from '../hooks/useWebAuthn'
import {
  RegisterResult,
  PostResult,
  CurrentIdentity,
  WebAuthnError,
} from '../types/webauthn'

// ============================================
// LocalStorage Keys
// ============================================

const STORAGE_KEY_IDENTITY = 'anarchy_webauthn_identity'
const STORAGE_KEY_CREDENTIALS = 'anarchy_webauthn_credentials'

// ============================================
// Types
// ============================================

export interface StoredIdentity {
  identityId: string // Stored as string for JSON serialization
  passkeyId: string // Base64 encoded
  credentialId: string
  deviceName?: string
  lastUsed?: number // Timestamp
}

export interface StoredCredential {
  credentialId: string
  identityId: string
  deviceName?: string
  createdAt: number
}

export interface WebAuthnContextValue extends UseWebAuthnResult {
  // Persistence
  persistedIdentity: StoredIdentity | null
  persistedCredentials: StoredCredential[]
  clearPersistedData: () => void
  
  // Multi-credential management
  switchCredential: (credentialId: string) => void
  removeCredential: (credentialId: string) => void
}

export interface WebAuthnProviderProps {
  children: ReactNode
  api: any | null
  signer: any | null
  onRegistrationSuccess?: (result: RegisterResult) => void
  onPostSuccess?: (result: PostResult) => void
  onError?: (error: WebAuthnError) => void
}

// ============================================
// Context
// ============================================

const WebAuthnContext = createContext<WebAuthnContextValue | null>(null)

// ============================================
// Helper Functions
// ============================================

/**
 * Convert Uint8Array to base64 string for storage
 */
function uint8ArrayToBase64(arr: Uint8Array): string {
  let binary = ''
  for (let i = 0; i < arr.length; i++) {
    binary += String.fromCharCode(arr[i])
  }
  return btoa(binary)
}

/**
 * Convert base64 string back to Uint8Array
 */
function base64ToUint8Array(base64: string): Uint8Array {
  const binary = atob(base64)
  const bytes = new Uint8Array(binary.length)
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i)
  }
  return bytes
}

/**
 * Load stored identity from LocalStorage
 */
function loadStoredIdentity(): StoredIdentity | null {
  if (typeof window === 'undefined') return null
  
  try {
    const stored = localStorage.getItem(STORAGE_KEY_IDENTITY)
    if (!stored) return null
    return JSON.parse(stored)
  } catch (e) {
    console.error('Failed to load stored identity:', e)
    return null
  }
}

/**
 * Save identity to LocalStorage
 */
function saveStoredIdentity(identity: StoredIdentity | null): void {
  if (typeof window === 'undefined') return
  
  try {
    if (identity) {
      localStorage.setItem(STORAGE_KEY_IDENTITY, JSON.stringify(identity))
    } else {
      localStorage.removeItem(STORAGE_KEY_IDENTITY)
    }
  } catch (e) {
    console.error('Failed to save identity:', e)
  }
}

/**
 * Load stored credentials from LocalStorage
 */
function loadStoredCredentials(): StoredCredential[] {
  if (typeof window === 'undefined') return []
  
  try {
    const stored = localStorage.getItem(STORAGE_KEY_CREDENTIALS)
    if (!stored) return []
    return JSON.parse(stored)
  } catch (e) {
    console.error('Failed to load stored credentials:', e)
    return []
  }
}

/**
 * Save credentials to LocalStorage
 */
function saveStoredCredentials(credentials: StoredCredential[]): void {
  if (typeof window === 'undefined') return
  
  try {
    localStorage.setItem(STORAGE_KEY_CREDENTIALS, JSON.stringify(credentials))
  } catch (e) {
    console.error('Failed to save credentials:', e)
  }
}

/**
 * Convert CurrentIdentity to StoredIdentity
 */
function toStoredIdentity(identity: CurrentIdentity): StoredIdentity {
  return {
    identityId: identity.identityId.toString(),
    passkeyId: uint8ArrayToBase64(identity.passkeyId),
    credentialId: identity.credentialId,
    deviceName: identity.deviceName,
    lastUsed: Date.now(),
  }
}

/**
 * Convert StoredIdentity to CurrentIdentity
 */
function fromStoredIdentity(stored: StoredIdentity): CurrentIdentity {
  return {
    identityId: BigInt(stored.identityId),
    passkeyId: base64ToUint8Array(stored.passkeyId),
    credentialId: stored.credentialId,
    deviceName: stored.deviceName,
  }
}

// ============================================
// Provider Component
// ============================================

export function WebAuthnProvider({
  children,
  api,
  signer,
  onRegistrationSuccess,
  onPostSuccess,
  onError,
}: WebAuthnProviderProps) {
  // Persisted data state
  const [persistedIdentity, setPersistedIdentity] = useState<StoredIdentity | null>(null)
  const [persistedCredentials, setPersistedCredentials] = useState<StoredCredential[]>([])
  const [isInitialized, setIsInitialized] = useState(false)

  // Load from LocalStorage on mount
  useEffect(() => {
    const storedIdentity = loadStoredIdentity()
    const storedCredentials = loadStoredCredentials()
    
    setPersistedIdentity(storedIdentity)
    setPersistedCredentials(storedCredentials)
    setIsInitialized(true)
  }, [])

  // Convert persisted identity to initial identity for useWebAuthn
  const initialIdentity = useMemo(() => {
    if (!persistedIdentity) return undefined
    return fromStoredIdentity(persistedIdentity)
  }, [persistedIdentity])

  // Handle registration success - persist identity and credential
  const handleRegistrationSuccess = useCallback((result: RegisterResult) => {
    if (result.success && result.identityId !== undefined && result.passkeyId) {
      // Create stored identity from result
      // Note: We need the credential ID which comes from the WebAuthn response
      // The hook will update its identity state which we can observe
      onRegistrationSuccess?.(result)
    }
  }, [onRegistrationSuccess])

  // Use the main WebAuthn hook
  const webauthn = useWebAuthn({
    api,
    signer,
    initialIdentity,
    onRegistrationSuccess: handleRegistrationSuccess,
    onPostSuccess,
    onError,
  })

  // Sync identity changes to LocalStorage
  useEffect(() => {
    if (!isInitialized) return
    
    if (webauthn.identity) {
      const stored = toStoredIdentity(webauthn.identity)
      setPersistedIdentity(stored)
      saveStoredIdentity(stored)
      
      // Also add to credentials list if not already present
      const credentialExists = persistedCredentials.some(
        c => c.credentialId === webauthn.identity!.credentialId
      )
      if (!credentialExists) {
        const newCredential: StoredCredential = {
          credentialId: webauthn.identity.credentialId,
          identityId: webauthn.identity.identityId.toString(),
          deviceName: webauthn.identity.deviceName,
          createdAt: Date.now(),
        }
        const updated = [...persistedCredentials, newCredential]
        setPersistedCredentials(updated)
        saveStoredCredentials(updated)
      }
    }
  }, [webauthn.identity, isInitialized, persistedCredentials])

  // Clear all persisted data
  const clearPersistedData = useCallback(() => {
    setPersistedIdentity(null)
    setPersistedCredentials([])
    saveStoredIdentity(null)
    saveStoredCredentials([])
    webauthn.reset()
  }, [webauthn])

  // Switch to a different credential
  const switchCredential = useCallback((credentialId: string) => {
    const credential = persistedCredentials.find(c => c.credentialId === credentialId)
    if (!credential) {
      console.error('Credential not found:', credentialId)
      return
    }

    // Load identity for this credential
    if (api) {
      webauthn.loadIdentityById(BigInt(credential.identityId), credentialId)
        .catch(err => {
          console.error('Failed to switch credential:', err)
        })
    }
  }, [api, persistedCredentials, webauthn])

  // Remove a credential from storage
  const removeCredential = useCallback((credentialId: string) => {
    const updated = persistedCredentials.filter(c => c.credentialId !== credentialId)
    setPersistedCredentials(updated)
    saveStoredCredentials(updated)
    
    // If this was the current credential, clear identity
    if (persistedIdentity?.credentialId === credentialId) {
      setPersistedIdentity(null)
      saveStoredIdentity(null)
      webauthn.reset()
    }
  }, [persistedCredentials, persistedIdentity, webauthn])

  const contextValue: WebAuthnContextValue = {
    // Spread all values from useWebAuthn
    ...webauthn,
    
    // Add persistence-related values
    persistedIdentity,
    persistedCredentials,
    clearPersistedData,
    switchCredential,
    removeCredential,
  }

  return (
    <WebAuthnContext.Provider value={contextValue}>
      {children}
    </WebAuthnContext.Provider>
  )
}

// ============================================
// Hook
// ============================================

/**
 * Hook to access WebAuthn context
 * 
 * Must be used within a WebAuthnProvider
 * 
 * @example
 * ```tsx
 * function MyComponent() {
 *   const { 
 *     identity, 
 *     registerPasskey, 
 *     signAndPost,
 *     persistedCredentials 
 *   } = useWebAuthnContext()
 *   
 *   // ...
 * }
 * ```
 */
export function useWebAuthnContext(): WebAuthnContextValue {
  const context = useContext(WebAuthnContext)
  
  if (context === null) {
    throw new Error('useWebAuthnContext must be used within a WebAuthnProvider')
  }
  
  return context
}
