'use client'

import { useState, useCallback, useRef } from 'react'
import { Binary } from 'polkadot-api'
import { extractCosePublicKey } from '../utils/cose'
import { derivePasskeyId, createCredentialCreationOptions, base64UrlEncode } from '../utils/webauthn'
import {
  RegistrationStatus,
  RegistrationErrorCode,
  RegistrationError,
  RegisterResult,
  UseWebAuthnRegistrationOptions,
  REGISTRATION_ERROR_MESSAGES,
} from '../types/webauthn'

/** Maximum device name length in bytes */
const MAX_DEVICE_NAME_BYTES = 64

/** RP ID for WebAuthn (defaults to current origin) */
const RP_ID = typeof window !== 'undefined' ? window.location.hostname : 'localhost'

/** RP name for WebAuthn */
const RP_NAME = 'Anarchy'

export interface UseWebAuthnRegistrationResult {
  status: RegistrationStatus
  register: (deviceName?: string) => Promise<RegisterResult>
  reset: () => void
  error: RegistrationError | null
}

/**
 * Create a RegistrationError from various error types
 */
function createRegistrationError(
  code: RegistrationErrorCode,
  originalError?: unknown
): RegistrationError {
  return {
    code,
    message: REGISTRATION_ERROR_MESSAGES[code],
    originalError,
  }
}

/**
 * Map WebAuthn error to our error code
 */
function mapWebAuthnError(error: unknown): RegistrationErrorCode {
  if (error instanceof Error) {
    // DOMException types from WebAuthn
    if (error.name === 'NotAllowedError') {
      return 'USER_CANCELLED'
    }
    if (error.name === 'InvalidStateError') {
      // Credential already exists
      return 'PASSKEY_ALREADY_REGISTERED'
    }
    if (error.name === 'NotSupportedError') {
      return 'WEBAUTHN_NOT_SUPPORTED'
    }
    if (error.name === 'SecurityError') {
      return 'WEBAUTHN_NOT_SUPPORTED'
    }
    if (error.name === 'AbortError') {
      return 'USER_CANCELLED'
    }
  }
  return 'AUTHENTICATOR_ERROR'
}

/**
 * Map blockchain transaction error to our error code
 */
function mapTransactionError(error: unknown): RegistrationErrorCode {
  if (error instanceof Error) {
    const msg = error.message.toLowerCase()
    if (msg.includes('passkey') && msg.includes('register')) {
      return 'PASSKEY_ALREADY_REGISTERED'
    }
    if (msg.includes('passkeyalreadyregistered')) {
      return 'PASSKEY_ALREADY_REGISTERED'
    }
    if (msg.includes('network') || msg.includes('connection')) {
      return 'NETWORK_ERROR'
    }
  }
  return 'TRANSACTION_FAILED'
}

/**
 * Truncate device name to max bytes (UTF-8 aware)
 */
function truncateDeviceName(name: string): string {
  const encoder = new TextEncoder()
  const bytes = encoder.encode(name)
  if (bytes.length <= MAX_DEVICE_NAME_BYTES) {
    return name
  }

  // Truncate bytes and decode back, handling partial characters
  let truncated = bytes.slice(0, MAX_DEVICE_NAME_BYTES)
  const decoder = new TextDecoder('utf-8', { fatal: false })
  let result = decoder.decode(truncated)

  // Remove potential broken character at the end
  while (encoder.encode(result).length > MAX_DEVICE_NAME_BYTES) {
    result = result.slice(0, -1)
  }

  return result
}

/**
 * Generate a random user ID for WebAuthn credential
 */
function generateUserId(): Uint8Array {
  const id = new Uint8Array(32)
  crypto.getRandomValues(id)
  return id
}

/**
 * Extract identity ID from transaction events
 * PAPI returns events as {type, value, topics} directly
 */
function extractIdentityIdFromEvents(events: any[]): bigint | undefined {
  for (const ev of events) {
    // Handle both PAPI format {type, value} and nested format {event: {type, value}}
    const event = ev.event ?? ev
    if (event?.type === 'Identity' && event?.value?.type === 'IdentityCreated') {
      return event.value.value.identity_id
    }
  }
  return undefined
}

/**
 * Hook for WebAuthn passkey registration
 * 
 * Handles the complete registration flow:
 * 1. Call navigator.credentials.create() to create a passkey
 * 2. Extract COSE public key from attestation
 * 3. Derive passkey ID (Blake2b-256)
 * 4. Submit register_identity transaction via PAPI
 * 
 * @example
 * ```tsx
 * function RegisterButton() {
 *   const { api, signer } = useApi()
 *   const { status, register, error } = useWebAuthnRegistration({ api, signer })
 * 
 *   const handleRegister = async () => {
 *     const result = await register('MacBook Pro')
 *     if (result.success) {
 *       console.log('Registered with ID:', result.identityId)
 *     }
 *   }
 * 
 *   return (
 *     <button onClick={handleRegister} disabled={status !== 'idle'}>
 *       {status === 'idle' ? 'パスキーで登録' : '処理中...'}
 *     </button>
 *   )
 * }
 * ```
 */
export function useWebAuthnRegistration(
  options: UseWebAuthnRegistrationOptions
): UseWebAuthnRegistrationResult {
  const { api, signer, onSuccess, onError } = options

  const [status, setStatus] = useState<RegistrationStatus>('idle')
  const [error, setError] = useState<RegistrationError | null>(null)

  // Prevent concurrent registration attempts
  const isProcessingRef = useRef(false)

  /**
   * Reset state to idle
   */
  const reset = useCallback(() => {
    setStatus('idle')
    setError(null)
    isProcessingRef.current = false
  }, [])

  /**
   * Register a new passkey
   */
  const register = useCallback(
    async (deviceName?: string): Promise<RegisterResult> => {
      // Check if already processing
      if (isProcessingRef.current) {
        // Return empty result for concurrent calls
        return {
          success: false,
          error: createRegistrationError('AUTHENTICATOR_ERROR'),
        }
      }

      isProcessingRef.current = true
      setError(null)

      // Validate prerequisites
      if (!api) {
        const err = createRegistrationError('NETWORK_ERROR')
        setStatus('error')
        setError(err)
        onError?.(err)
        isProcessingRef.current = false
        return { success: false, error: err }
      }

      if (!signer) {
        const err = createRegistrationError('NETWORK_ERROR')
        setStatus('error')
        setError(err)
        onError?.(err)
        isProcessingRef.current = false
        return { success: false, error: err }
      }

      // Check WebAuthn support
      if (
        typeof navigator === 'undefined' ||
        !navigator.credentials ||
        !navigator.credentials.create
      ) {
        const err = createRegistrationError('WEBAUTHN_NOT_SUPPORTED')
        setStatus('error')
        setError(err)
        onError?.(err)
        isProcessingRef.current = false
        return { success: false, error: err }
      }

      // Track current phase for error handling
      let currentPhase: 'authenticating' | 'extracting' | 'submitting' | 'confirming' = 'authenticating'

      try {
        // Step 1: Authenticating - Create WebAuthn credential
        setStatus('authenticating')
        currentPhase = 'authenticating'

        const userId = generateUserId()
        const userName = `anarchy-user-${base64UrlEncode(userId).slice(0, 8)}`

        const credentialCreationOptions = createCredentialCreationOptions({
          challenge: crypto.getRandomValues(new Uint8Array(32)),
          rpId: RP_ID,
          rpName: RP_NAME,
          userId,
          userName,
          userDisplayName: deviceName || 'Anarchy User',
        })

        const credential = (await navigator.credentials.create({
          publicKey: credentialCreationOptions,
        })) as PublicKeyCredential | null

        if (!credential) {
          throw Object.assign(new Error('No credential returned'), {
            name: 'AuthenticatorError',
          })
        }

        // Step 2: Extracting - Get COSE public key from attestation
        setStatus('extracting')
        currentPhase = 'extracting'

        const response = credential.response as AuthenticatorAttestationResponse
        let cosePublicKey: Uint8Array

        try {
          cosePublicKey = extractCosePublicKey(response.attestationObject)
        } catch (extractionError) {
          const err = createRegistrationError('EXTRACTION_FAILED', extractionError)
          setStatus('error')
          setError(err)
          onError?.(err)
          isProcessingRef.current = false
          return { success: false, error: err }
        }

        // Derive passkey ID (for reference, not used in transaction)
        const passkeyId = derivePasskeyId(cosePublicKey)

        // Step 3: Submitting - Create and submit transaction
        setStatus('submitting')
        currentPhase = 'submitting'

        // Truncate device name if necessary and encode to Binary
        const truncatedDeviceName = deviceName
          ? truncateDeviceName(deviceName)
          : undefined
        const encodedDeviceName = truncatedDeviceName
          ? Binary.fromBytes(new TextEncoder().encode(truncatedDeviceName))
          : undefined

        // Create transaction
        const tx = api.tx.Identity.register_identity({
          public_key: Binary.fromBytes(cosePublicKey),
          device_name: encodedDeviceName,
        })

        // Step 4: Confirming - Wait for transaction finalization
        setStatus('confirming')
        currentPhase = 'confirming'

        const result = await tx.signAndSubmit(signer)

        // Check if transaction succeeded
        if (!result.ok) {
          console.error('Transaction failed:', result)
          const dispatchError = (result as any).dispatchError
          const errorMsg = dispatchError ? JSON.stringify(dispatchError) : 'Unknown transaction error'
          throw new Error(`Transaction failed: ${errorMsg}`)
        }

        // Extract identity ID from events
        const identityId = extractIdentityIdFromEvents(result.events || [])

        // Success!
        setStatus('success')
        isProcessingRef.current = false

        const successResult: RegisterResult = {
          success: true,
          identityId,
          passkeyId,
        }

        onSuccess?.(successResult)
        return successResult
      } catch (err) {
        console.error('Registration failed:', err)
        console.error('Error details:', {
          name: err instanceof Error ? err.name : 'unknown',
          message: err instanceof Error ? err.message : String(err),
          cause: err instanceof Error ? (err as any).cause : undefined,
          phase: currentPhase,
        })

        // Determine error type based on current phase
        let errorCode: RegistrationErrorCode

        if (currentPhase === 'authenticating') {
          errorCode = mapWebAuthnError(err)
        } else if (currentPhase === 'submitting' || currentPhase === 'confirming') {
          errorCode = mapTransactionError(err)
        } else {
          errorCode = 'AUTHENTICATOR_ERROR'
        }

        const registrationError = createRegistrationError(errorCode, err)
        setStatus('error')
        setError(registrationError)
        onError?.(registrationError)
        isProcessingRef.current = false

        return { success: false, error: registrationError }
      }
    },
    [api, signer, onSuccess, onError]
  )

  return {
    status,
    register,
    reset,
    error,
  }
}
