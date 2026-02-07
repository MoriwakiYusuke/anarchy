'use client'

import { useState, useCallback, useRef, useEffect } from 'react'
import { Binary } from 'polkadot-api'
import { useWebAuthnSupport } from './useWebAuthnSupport'
import { extractCosePublicKey } from '../utils/cose'
import {
  derivePasskeyId,
  createCredentialCreationOptions,
  generateWysiwysChallenge,
  base64UrlEncode,
  base64UrlDecode,
} from '../utils/webauthn'
import {
  RegistrationStatus,
  SigningStatus,
  RegistrationErrorCode,
  SigningErrorCode,
  WebAuthnErrorCode,
  WebAuthnError,
  RegisterResult,
  PostResult,
  AddPasskeyResult,
  CurrentIdentity,
  UseWebAuthnOptions,
  REGISTRATION_ERROR_MESSAGES,
  SIGNING_ERROR_MESSAGES,
  WEBAUTHN_ERROR_MESSAGES,
} from '../types/webauthn'

/** Maximum device name length in bytes */
const MAX_DEVICE_NAME_BYTES = 64

/** RP ID for WebAuthn (defaults to current origin) */
const RP_ID = typeof window !== 'undefined' ? window.location.hostname : 'localhost'

/** RP name for WebAuthn */
const RP_NAME = 'Anarchy'

export interface UseWebAuthnResult {
  // Feature Detection (from useWebAuthnSupport)
  isSupported: boolean
  hasPlatformAuthenticator: boolean | null

  // Identity State
  identity: CurrentIdentity | null

  // Registration
  registrationStatus: RegistrationStatus
  registerPasskey: (deviceName?: string) => Promise<RegisterResult>

  // Signing
  signingStatus: SigningStatus
  signAndPost: (content: string, parentId?: bigint) => Promise<PostResult>

  // Multi-device
  addPasskey: (deviceName?: string) => Promise<AddPasskeyResult>

  // Utilities
  loadIdentityById: (identityId: bigint, credentialId: string) => Promise<void>
  loginWithPasskey: () => Promise<{ success: boolean; error?: WebAuthnError }>
  reset: () => void
  error: WebAuthnError | null
}

/**
 * Create a WebAuthnError from various error types
 */
function createWebAuthnError(code: WebAuthnErrorCode, originalError?: unknown): WebAuthnError {
  return {
    code,
    message: WEBAUTHN_ERROR_MESSAGES[code],
    originalError,
  }
}

/**
 * Map WebAuthn error to our error code
 */
function mapWebAuthnError(error: unknown): WebAuthnErrorCode {
  if (error instanceof Error) {
    // Check message first (for jsdom compatibility)
    if (error.message.includes('cancel') || error.message.includes('Cancel')) {
      return 'USER_CANCELLED'
    }
    // Check for transaction errors
    if (error.message.includes('Transaction failed')) {
      return 'TRANSACTION_FAILED'
    }
    if (error.message.includes('PasskeyAlreadyRegistered')) {
      return 'PASSKEY_ALREADY_REGISTERED'
    }
    if (error.message.includes('TooManyPasskeys')) {
      return 'TOO_MANY_PASSKEYS'
    }
    if (error.message.includes('IdentityNotFound')) {
      return 'IDENTITY_NOT_FOUND'
    }
    // DOMException types from WebAuthn
    if (error.name === 'NotAllowedError') {
      return 'USER_CANCELLED'
    }
    if (error.name === 'InvalidStateError') {
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
function mapTransactionError(error: unknown): WebAuthnErrorCode {
  if (error instanceof Error) {
    const msg = error.message.toLowerCase()
    if (msg.includes('passkey') && msg.includes('register')) {
      return 'PASSKEY_ALREADY_REGISTERED'
    }
    if (msg.includes('passkeyalreadyregistered')) {
      return 'PASSKEY_ALREADY_REGISTERED'
    }
    if (msg.includes('toomanypasskeys')) {
      return 'TOO_MANY_PASSKEYS'
    }
    if (msg.includes('identitynotfound')) {
      return 'IDENTITY_NOT_FOUND'
    }
    if (msg.includes('network') || msg.includes('connection')) {
      return 'NETWORK_ERROR'
    }
    if (msg.includes('insufficient') || msg.includes('balance')) {
      return 'INSUFFICIENT_BALANCE'
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

  let truncated = bytes.slice(0, MAX_DEVICE_NAME_BYTES)
  const decoder = new TextDecoder('utf-8', { fatal: false })
  let result = decoder.decode(truncated)

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
 * Extract passkey ID from add_passkey transaction events
 * PAPI returns events as {type, value, topics} directly
 */
function extractPasskeyIdFromEvents(events: any[]): Uint8Array | undefined {
  for (const ev of events) {
    const event = ev.event ?? ev
    if (event?.type === 'Identity' && event?.value?.type === 'PasskeyAdded') {
      return event.value.value.passkey_id
    }
  }
  return undefined
}

/**
 * Extract post ID from transaction events
 * PAPI returns events as {type, value, topics} directly
 */
function extractPostIdFromEvents(events: any[]): bigint | undefined {
  for (const ev of events) {
    const event = ev.event ?? ev
    if (event?.type === 'Post' && event?.value?.type === 'PostCreated') {
      return event.value.value.post_id
    }
    if (event?.type === 'Post' && event?.value?.type === 'PostCreatedWithWebAuthn') {
      return event.value.value.post_id
    }
  }
  return undefined
}

/**
 * Extract moral spent from transaction events
 * PAPI returns events as {type, value, topics} directly
 */
function extractMoralSpentFromEvents(events: any[]): bigint | undefined {
  for (const ev of events) {
    const event = ev.event ?? ev
    if (event?.type === 'Post') {
      if (event?.value?.value?.moral_spent !== undefined) {
        return event.value.value.moral_spent
      }
    }
  }
  return undefined
}

/**
 * Hook for complete WebAuthn integration
 *
 * Combines feature detection, registration, signing, and multi-device support.
 *
 * @example
 * ```tsx
 * function WebAuthnApp() {
 *   const { api, signer } = useApi()
 *   const {
 *     isSupported,
 *     hasPlatformAuthenticator,
 *     identity,
 *     registrationStatus,
 *     signingStatus,
 *     registerPasskey,
 *     signAndPost,
 *     addPasskey,
 *     loadIdentityById,
 *     reset,
 *     error
 *   } = useWebAuthn({ api, signer })
 *
 *   if (!isSupported) return <div>WebAuthn非対応</div>
 *
 *   if (!identity) {
 *     return <button onClick={() => registerPasskey('MacBook')}>登録</button>
 *   }
 *
 *   return (
 *     <div>
 *       <button onClick={() => signAndPost('Hello!')}>投稿</button>
 *       <button onClick={() => addPasskey('iPhone')}>デバイス追加</button>
 *     </div>
 *   )
 * }
 * ```
 */
export function useWebAuthn(options: UseWebAuthnOptions): UseWebAuthnResult {
  const { api, signer, initialIdentity, onRegistrationSuccess, onPostSuccess, onError } = options

  // Feature detection via useWebAuthnSupport
  const { isSupported, hasPlatformAuthenticator } = useWebAuthnSupport()

  // Identity state
  const [identity, setIdentity] = useState<CurrentIdentity | null>(initialIdentity ?? null)

  // Sync identity when initialIdentity changes (e.g., after LocalStorage load)
  useEffect(() => {
    if (initialIdentity && !identity) {
      setIdentity(initialIdentity)
    }
  }, [initialIdentity, identity])

  // Registration state
  const [registrationStatus, setRegistrationStatus] = useState<RegistrationStatus>('idle')

  // Signing state
  const [signingStatus, setSigningStatus] = useState<SigningStatus>('idle')

  // Error state
  const [error, setError] = useState<WebAuthnError | null>(null)

  // Prevent double registration
  const registrationInProgress = useRef(false)
  const signingInProgress = useRef(false)

  /**
   * Register new identity with passkey
   */
  const registerPasskey = useCallback(
    async (deviceName?: string): Promise<RegisterResult> => {
      if (registrationInProgress.current) {
        return { success: false, error: createWebAuthnError('AUTHENTICATOR_ERROR') as any }
      }

      registrationInProgress.current = true
      setError(null)
      setRegistrationStatus('authenticating')

      try {
        // Check prerequisites
        if (!api) {
          const err = createWebAuthnError('API_NOT_AVAILABLE')
          setError(err)
          setRegistrationStatus('error')
          onError?.(err)
          return { success: false, error: err as any }
        }

        if (!signer) {
          const err = createWebAuthnError('SIGNER_NOT_AVAILABLE')
          setError(err)
          setRegistrationStatus('error')
          onError?.(err)
          return { success: false, error: err as any }
        }

        // Create WebAuthn credential
        const userId = generateUserId()
        const challenge = crypto.getRandomValues(new Uint8Array(32))
        const credentialOptions = createCredentialCreationOptions({
          challenge,
          rpId: RP_ID,
          rpName: RP_NAME,
          userId,
          userName: deviceName ?? 'Anarchy User',
        })

        const credential = (await navigator.credentials.create({
          publicKey: credentialOptions,
        })) as PublicKeyCredential | null

        if (!credential) {
          const err = createWebAuthnError('AUTHENTICATOR_ERROR')
          setError(err)
          setRegistrationStatus('error')
          onError?.(err)
          return { success: false, error: err as any }
        }

        setRegistrationStatus('extracting')

        // Extract COSE public key
        const attestationResponse = credential.response as AuthenticatorAttestationResponse
        const cosePublicKey = extractCosePublicKey(attestationResponse.attestationObject)

        // Derive passkey ID
        const passkeyId = derivePasskeyId(cosePublicKey)

        setRegistrationStatus('submitting')

        // Truncate device name if provided
        const truncatedName = deviceName ? truncateDeviceName(deviceName) : undefined

        // Build transaction
        const tx = api.tx.Identity.register_identity({
          public_key: Binary.fromBytes(cosePublicKey),
          device_name: truncatedName ? Binary.fromBytes(new TextEncoder().encode(truncatedName)) : undefined,
        })

        setRegistrationStatus('confirming')

        // Submit transaction
        const result = await tx.signAndSubmit(signer)

        // Check if transaction succeeded
        if (!result.ok) {
          console.error('Transaction failed:', result)
          const dispatchError = (result as any).dispatchError
          const errorMsg = dispatchError ? JSON.stringify(dispatchError) : 'Unknown transaction error'
          throw new Error(`Transaction failed: ${errorMsg}`)
        }

        // Extract identity ID from events
        const identityId = extractIdentityIdFromEvents(result.events)

        if (identityId === undefined) {
          const err = createWebAuthnError('TRANSACTION_FAILED')
          setError(err)
          setRegistrationStatus('error')
          onError?.(err)
          return { success: false, error: err as any }
        }

        // Update identity state
        const newIdentity: CurrentIdentity = {
          identityId,
          passkeyId,
          credentialId: credential.id,
          deviceName: truncatedName,
        }
        setIdentity(newIdentity)

        setRegistrationStatus('success')

        const registerResult: RegisterResult = {
          success: true,
          identityId,
          passkeyId,
        }

        onRegistrationSuccess?.(registerResult)

        return registerResult
      } catch (err) {
        console.error('Registration failed:', err)
        const errorCode = mapWebAuthnError(err)
        const webauthnError = createWebAuthnError(errorCode, err)
        setError(webauthnError)
        setRegistrationStatus('error')
        onError?.(webauthnError)
        return { success: false, error: webauthnError as any }
      } finally {
        registrationInProgress.current = false
      }
    },
    [api, signer, onRegistrationSuccess, onError]
  )

  /**
   * Add passkey to existing identity
   */
  const addPasskey = useCallback(
    async (deviceName?: string): Promise<AddPasskeyResult> => {
      if (registrationInProgress.current) {
        return { success: false, error: createWebAuthnError('AUTHENTICATOR_ERROR') }
      }

      registrationInProgress.current = true
      setError(null)
      setRegistrationStatus('authenticating')

      // Track which phase we're in for error mapping
      let currentPhase: 'webauthn' | 'transaction' = 'webauthn'

      try {
        // Check prerequisites
        if (!api) {
          const err = createWebAuthnError('API_NOT_AVAILABLE')
          setError(err)
          setRegistrationStatus('error')
          onError?.(err)
          return { success: false, error: err }
        }

        if (!signer) {
          const err = createWebAuthnError('SIGNER_NOT_AVAILABLE')
          setError(err)
          setRegistrationStatus('error')
          onError?.(err)
          return { success: false, error: err }
        }

        if (!identity) {
          const err = createWebAuthnError('NO_IDENTITY')
          setError(err)
          setRegistrationStatus('error')
          onError?.(err)
          return { success: false, error: err }
        }

        // Create WebAuthn credential
        const userId = generateUserId()
        const challenge = crypto.getRandomValues(new Uint8Array(32))
        const credentialOptions = createCredentialCreationOptions({
          challenge,
          rpId: RP_ID,
          rpName: RP_NAME,
          userId,
          userName: deviceName ?? 'Anarchy User - Additional Device',
        })

        const credential = (await navigator.credentials.create({
          publicKey: credentialOptions,
        })) as PublicKeyCredential | null

        if (!credential) {
          const err = createWebAuthnError('AUTHENTICATOR_ERROR')
          setError(err)
          setRegistrationStatus('error')
          onError?.(err)
          return { success: false, error: err }
        }

        setRegistrationStatus('extracting')

        // Extract COSE public key
        const attestationResponse = credential.response as AuthenticatorAttestationResponse
        const cosePublicKey = extractCosePublicKey(attestationResponse.attestationObject)

        // Derive passkey ID
        const passkeyId = derivePasskeyId(cosePublicKey)

        setRegistrationStatus('submitting')

        // Truncate device name if provided
        const truncatedName = deviceName ? truncateDeviceName(deviceName) : undefined

        // Now entering transaction phase
        currentPhase = 'transaction'

        // Build add_passkey transaction
        const tx = api.tx.Identity.add_passkey({
          identity_id: identity.identityId,
          public_key: Binary.fromBytes(cosePublicKey),
          device_name: truncatedName ? Binary.fromBytes(new TextEncoder().encode(truncatedName)) : undefined,
        })

        setRegistrationStatus('confirming')

        // Submit transaction
        const result = await tx.signAndSubmit(signer)

        // Check if transaction succeeded
        if (!result.ok) {
          console.error('Add passkey transaction failed:', result)
          const dispatchError = (result as any).dispatchError
          const errorMsg = dispatchError ? JSON.stringify(dispatchError) : 'Unknown transaction error'
          throw new Error(`Transaction failed: ${errorMsg}`)
        }

        // Extract passkey ID from events
        const eventPasskeyId = extractPasskeyIdFromEvents(result.events) ?? passkeyId

        setRegistrationStatus('success')

        return {
          success: true,
          passkeyId: eventPasskeyId,
        }
      } catch (err) {
        console.error('Add passkey failed:', err)
        // Use appropriate error mapper based on phase
        const errorCode = currentPhase === 'webauthn' 
          ? mapWebAuthnError(err) 
          : mapTransactionError(err)
        const webauthnError = createWebAuthnError(errorCode, err)
        setError(webauthnError)
        setRegistrationStatus('error')
        onError?.(webauthnError)
        return { success: false, error: webauthnError }
      } finally {
        registrationInProgress.current = false
      }
    },
    [api, signer, identity, onError]
  )

  /**
   * Sign and post content
   */
  const signAndPost = useCallback(
    async (content: string, parentId?: bigint): Promise<PostResult> => {
      if (signingInProgress.current) {
        return { success: false, error: createWebAuthnError('AUTHENTICATOR_ERROR') as any }
      }

      signingInProgress.current = true
      setError(null)
      setSigningStatus('hashing')

      try {
        // Check prerequisites
        if (!api) {
          const err = createWebAuthnError('API_NOT_AVAILABLE')
          setError(err)
          setSigningStatus('error')
          onError?.(err)
          return { success: false, error: err as any }
        }

        if (!signer) {
          const err = createWebAuthnError('SIGNER_NOT_AVAILABLE')
          setError(err)
          setSigningStatus('error')
          onError?.(err)
          return { success: false, error: err as any }
        }

        if (!identity) {
          const err = createWebAuthnError('NO_IDENTITY')
          setError(err)
          setSigningStatus('error')
          onError?.(err)
          return { success: false, error: err as any }
        }

        // Generate WYSIWYS challenge
        const challenge = await generateWysiwysChallenge(content)

        setSigningStatus('authenticating')

        // Get WebAuthn assertion
        const credentialId = base64UrlDecode(identity.credentialId)
        const assertion = (await navigator.credentials.get({
          publicKey: {
            challenge: challenge as BufferSource,
            rpId: RP_ID,
            allowCredentials: [
              {
                id: credentialId as BufferSource,
                type: 'public-key',
                transports: ['internal'],
              },
            ],
            userVerification: 'preferred',
            timeout: 60000,
          },
        })) as PublicKeyCredential | null

        if (!assertion) {
          const err = createWebAuthnError('CREDENTIAL_NOT_FOUND')
          setError(err)
          setSigningStatus('error')
          onError?.(err)
          return { success: false, error: err as any }
        }

        setSigningStatus('submitting')

        const assertionResponse = assertion.response as AuthenticatorAssertionResponse

        // Build post transaction
        const tx = api.tx.Post.create_post_with_webauthn({
          identity_id: identity.identityId,
          passkey_id: Binary.fromBytes(identity.passkeyId),
          content: Binary.fromBytes(new TextEncoder().encode(content)),
          authenticator_data: Binary.fromBytes(new Uint8Array(assertionResponse.authenticatorData)),
          client_data_json: Binary.fromBytes(new Uint8Array(assertionResponse.clientDataJSON)),
          signature: Binary.fromBytes(new Uint8Array(assertionResponse.signature)),
          parent_id: parentId,
        })

        setSigningStatus('confirming')

        // Submit transaction
        const result = await tx.signAndSubmit(signer)

        // Check if transaction succeeded
        if (!result.ok) {
          console.error('Post transaction failed:', result)
          const dispatchError = (result as any).dispatchError
          const errorMsg = dispatchError ? JSON.stringify(dispatchError) : 'Unknown transaction error'
          throw new Error(`Transaction failed: ${errorMsg}`)
        }

        // Extract post ID from events
        const postId = extractPostIdFromEvents(result.events)
        const moralSpent = extractMoralSpentFromEvents(result.events)

        if (postId === undefined) {
          const err = createWebAuthnError('TRANSACTION_FAILED')
          setError(err)
          setSigningStatus('error')
          onError?.(err)
          return { success: false, error: err as any }
        }

        setSigningStatus('success')

        const postResult: PostResult = {
          success: true,
          postId,
          txHash: result.txHash,
          moralSpent,
        }

        onPostSuccess?.(postResult)

        return postResult
      } catch (err) {
        console.error('Signing failed:', err)
        const errorCode = mapWebAuthnError(err)
        const webauthnError = createWebAuthnError(errorCode, err)
        setError(webauthnError)
        setSigningStatus('error')
        onError?.(webauthnError)
        return { success: false, error: webauthnError as any }
      } finally {
        signingInProgress.current = false
      }
    },
    [api, signer, identity, onPostSuccess, onError]
  )

  /**
   * Load identity by ID from chain
   */
  const loadIdentityById = useCallback(
    async (identityId: bigint, credentialId: string): Promise<void> => {
      setError(null)

      if (!api) {
        const err = createWebAuthnError('API_NOT_AVAILABLE')
        setError(err)
        throw err
      }

      try {
        // Query identity from chain
        const chainIdentity = await api.query.Identity.Identities.getValue(identityId)

        if (!chainIdentity) {
          const err = createWebAuthnError('IDENTITY_NOT_FOUND')
          setError(err)
          throw err
        }

        // Find matching passkey (if credential matches) or use first one
        let passkeyId: Uint8Array | undefined
        let deviceName: string | undefined

        if (chainIdentity.passkeys && chainIdentity.passkeys.length > 0) {
          // Use first passkey by default
          // Note: チェーン側のPasskey構造体は { id, public_key, ... }
          passkeyId = chainIdentity.passkeys[0].id
          deviceName = chainIdentity.passkeys[0].device_name
        }

        // Update identity state
        const newIdentity: CurrentIdentity = {
          identityId,
          passkeyId: passkeyId ?? new Uint8Array(32),
          credentialId,
          deviceName,
        }
        setIdentity(newIdentity)
      } catch (err) {
        if ((err as WebAuthnError).code) {
          throw err
        }
        console.error('Failed to load identity:', err)
        const webauthnError = createWebAuthnError('NETWORK_ERROR', err)
        setError(webauthnError)
        throw webauthnError
      }
    },
    [api]
  )

  /**
   * Reset all states
   */
  const reset = useCallback(() => {
    setIdentity(null)
    setRegistrationStatus('idle')
    setSigningStatus('idle')
    setError(null)
    registrationInProgress.current = false
    signingInProgress.current = false
  }, [])

  /**
   * Login with existing passkey
   * Uses WebAuthn credentials.get() to authenticate, then queries chain for identity
   */
  const loginWithPasskey = useCallback(
    async (): Promise<{ success: boolean; error?: WebAuthnError }> => {
      setError(null)

      if (!api) {
        const err = createWebAuthnError('API_NOT_AVAILABLE')
        setError(err)
        return { success: false, error: err }
      }

      try {
        // Generate a simple challenge for authentication
        const challenge = new Uint8Array(32)
        crypto.getRandomValues(challenge)

        // Request passkey authentication without specifying allowCredentials
        // This lets the user pick from any passkey registered with this RP
        const credential = await navigator.credentials.get({
          publicKey: {
            challenge,
            rpId: window.location.hostname === 'localhost' ? 'localhost' : window.location.hostname,
            timeout: 60000,
            userVerification: 'preferred',
          },
        }) as PublicKeyCredential | null

        if (!credential) {
          const err = createWebAuthnError('AUTHENTICATOR_ERROR')
          setError(err)
          return { success: false, error: err }
        }

        // Get credential ID
        const credentialId = credential.id

        // Now we need to find the identity associated with this passkey
        // Query chain to find identity by iterating through stored credentials
        // For now, we'll use the raw credential ID bytes to derive the passkey ID
        const rawIdBytes = new Uint8Array(credential.rawId)
        
        // Try to find matching identity by querying all identities
        // This is a simplified approach - in production you'd have an index
        // For now, we'll store the association in localStorage during registration
        
        // Check if we have stored credentials that match
        const storedCredentials = localStorage.getItem('anarchy_webauthn_credentials')
        if (storedCredentials) {
          const credentials = JSON.parse(storedCredentials)
          const matchingCred = credentials.find((c: any) => c.credentialId === credentialId)
          
          if (matchingCred) {
            const identityId = BigInt(matchingCred.identityId)
            await loadIdentityById(identityId, credentialId)
            return { success: true }
          }
        }

        // If no stored credential found, we can't determine the identity
        // The user needs to provide their identity ID or re-register
        const err = createWebAuthnError('IDENTITY_NOT_FOUND')
        setError(err)
        return { success: false, error: err }
      } catch (err) {
        console.error('Login with passkey failed:', err)
        const errorCode = mapWebAuthnError(err)
        const webauthnError = createWebAuthnError(errorCode, err)
        setError(webauthnError)
        return { success: false, error: webauthnError }
      }
    },
    [api, loadIdentityById]
  )

  return {
    // Feature Detection
    isSupported,
    hasPlatformAuthenticator,

    // Identity State
    identity,

    // Registration
    registrationStatus,
    registerPasskey,

    // Signing
    signingStatus,
    signAndPost,

    // Multi-device
    addPasskey,

    // Utilities
    loadIdentityById,
    loginWithPasskey,
    reset,
    error,
  }
}
