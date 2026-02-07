'use client'

import { useState, useCallback, useRef } from 'react'
import { Binary } from 'polkadot-api'
import {
  generateWysiwysChallenge,
  createCredentialRequestOptions,
  base64UrlDecode,
} from '../utils/webauthn'
import {
  SigningStatus,
  SigningErrorCode,
  SigningError,
  PostResult,
  UseWebAuthnSigningOptions,
  SIGNING_ERROR_MESSAGES,
} from '../types/webauthn'

/** Fallback cost constants (should match runtime) */
const FALLBACK_BASE_COST = 10 // 10 MORAL
const FALLBACK_BYTE_COST = 0.1 // 0.1 MORAL per byte

export interface UseWebAuthnSigningResult {
  status: SigningStatus
  sign: (content: string, parentId?: number) => Promise<PostResult>
  estimateCost: (content: string) => number
  reset: () => void
  error: SigningError | null
}

/**
 * Create a SigningError from various error types
 */
function createSigningError(code: SigningErrorCode, originalError?: unknown): SigningError {
  return {
    code,
    message: SIGNING_ERROR_MESSAGES[code],
    originalError,
  }
}

/**
 * Map WebAuthn error to our error code
 */
function mapWebAuthnError(error: unknown): SigningErrorCode {
  if (error instanceof Error) {
    const message = error.message.toLowerCase()
    const errorName = error.name

    // Check message first for better jsdom compatibility
    if (message.includes('not allowed') || message.includes('notallowed')) {
      return 'USER_CANCELLED'
    }
    if (message.includes('abort') || message.includes('timed out')) {
      return 'USER_CANCELLED'
    }
    if (message.includes('invalid state')) {
      return 'CREDENTIAL_NOT_FOUND'
    }
    if (message.includes('not supported')) {
      return 'WEBAUTHN_NOT_SUPPORTED'
    }
    if (message.includes('security')) {
      return 'WEBAUTHN_NOT_SUPPORTED'
    }

    // Fallback to error name
    if (errorName === 'NotAllowedError') {
      return 'USER_CANCELLED'
    }
    if (errorName === 'InvalidStateError') {
      return 'CREDENTIAL_NOT_FOUND'
    }
    if (errorName === 'NotSupportedError') {
      return 'WEBAUTHN_NOT_SUPPORTED'
    }
    if (errorName === 'SecurityError') {
      return 'WEBAUTHN_NOT_SUPPORTED'
    }
    if (errorName === 'AbortError') {
      return 'USER_CANCELLED'
    }
  }
  return 'AUTHENTICATOR_ERROR'
}

/**
 * Map blockchain transaction error to our error code
 */
function mapTransactionError(error: unknown): SigningErrorCode {
  if (error instanceof Error) {
    const msg = error.message.toLowerCase()
    if (msg.includes('insufficientbalance') || msg.includes('insufficient balance') || msg.includes('insufficient')) {
      return 'INSUFFICIENT_BALANCE'
    }
    if (msg.includes('signatureinvalid') || msg.includes('signature invalid') || msg.includes('invalid signature')) {
      return 'SIGNATURE_INVALID'
    }
    if (msg.includes('challengemismatch') || msg.includes('challenge mismatch') || msg.includes('mismatch')) {
      return 'CHALLENGE_MISMATCH'
    }
    if (msg.includes('network') || msg.includes('connection')) {
      return 'NETWORK_ERROR'
    }
    if (msg.includes('contenttolong') || msg.includes('content too long')) {
      return 'CONTENT_TOO_LONG'
    }
    if (msg.includes('credentialnotfound') || msg.includes('credential not found')) {
      return 'CREDENTIAL_NOT_FOUND'
    }
  }
  return 'TRANSACTION_FAILED'
}

/**
 * Calculate UTF-8 byte length of a string
 */
function getByteLength(str: string): number {
  const encoder = new TextEncoder()
  return encoder.encode(str).length
}

/**
 * Extract post ID from transaction events
 */
function extractPostIdFromEvents(events: any[]): bigint | undefined {
  for (const { event } of events) {
    if (
      event?.type === 'Post' &&
      event?.value?.type === 'PostCreated' &&
      event?.value?.value?.post_id !== undefined
    ) {
      return BigInt(event.value.value.post_id)
    }
  }
  return undefined
}

/**
 * Extract moral spent from transaction events
 */
function extractMoralSpentFromEvents(events: any[]): bigint | undefined {
  for (const { event } of events) {
    if (
      event?.type === 'Post' &&
      event?.value?.type === 'PostCreated' &&
      event?.value?.value?.moral_spent !== undefined
    ) {
      return BigInt(event.value.value.moral_spent)
    }
  }
  return undefined
}

/**
 * Hook for WebAuthn-signed post creation
 *
 * Handles the complete flow:
 * 1. Generate WYSIWYS challenge from content
 * 2. Request WebAuthn assertion
 * 3. Submit to blockchain with authenticator data
 *
 * @example
 * ```tsx
 * const { sign, status, error } = useWebAuthnSigning({
 *   api,
 *   signer,
 *   identityId: 42n,
 *   passkeyId: new Uint8Array([...]),
 *   credentialId: 'base64url-credential-id',
 * });
 *
 * const result = await sign('Hello, world!');
 * if (result.success) {
 *   console.log('Posted!', result.postId);
 * }
 * ```
 */
export function useWebAuthnSigning(options: UseWebAuthnSigningOptions): UseWebAuthnSigningResult {
  const { api, signer, identityId, passkeyId, credentialId, onSuccess, onError } = options

  const [status, setStatus] = useState<SigningStatus>('idle')
  const [error, setError] = useState<SigningError | null>(null)
  const abortControllerRef = useRef<AbortController | null>(null)

  /**
   * Estimate post cost based on content length
   * Uses fallback values (actual values should come from usePostCost)
   */
  const estimateCost = useCallback((content: string): number => {
    const byteLength = getByteLength(content)
    return FALLBACK_BASE_COST + FALLBACK_BYTE_COST * byteLength
  }, [])

  /**
   * Reset state to idle
   */
  const reset = useCallback(() => {
    if (abortControllerRef.current) {
      abortControllerRef.current.abort()
      abortControllerRef.current = null
    }
    setStatus('idle')
    setError(null)
  }, [])

  /**
   * Sign and post content using WebAuthn
   */
  const sign = useCallback(
    async (content: string, parentId?: number): Promise<PostResult> => {
      // Track current phase for error classification
      let currentPhase: 'hashing' | 'authenticating' | 'submitting' | 'confirming' = 'hashing'

      try {
        // Validate prerequisites
        if (!api || !signer) {
          const err = createSigningError('NETWORK_ERROR')
          setStatus('error')
          setError(err)
          onError?.(err)
          return { success: false, error: err }
        }

        // Check WebAuthn support
        if (!navigator.credentials) {
          const err = createSigningError('WEBAUTHN_NOT_SUPPORTED')
          setStatus('error')
          setError(err)
          onError?.(err)
          return { success: false, error: err }
        }

        // Reset previous state
        setError(null)
        abortControllerRef.current = new AbortController()

        // Phase 1: Generate WYSIWYS challenge
        setStatus('hashing')
        currentPhase = 'hashing'
        const challenge = await generateWysiwysChallenge(content)

        // Phase 2: Request WebAuthn assertion
        setStatus('authenticating')
        currentPhase = 'authenticating'

        // Decode credential ID from base64url
        const rawCredentialId = base64UrlDecode(credentialId)

        const requestOptions = createCredentialRequestOptions({
          challenge,
          allowCredentials: [{ id: rawCredentialId, type: 'public-key' }],
        })

        const credential = (await navigator.credentials.get({
          publicKey: requestOptions,
        })) as PublicKeyCredential | null

        if (!credential) {
          const err = createSigningError('CREDENTIAL_NOT_FOUND')
          setStatus('error')
          setError(err)
          onError?.(err)
          return { success: false, error: err }
        }

        const response = credential.response as AuthenticatorAssertionResponse

        // Extract assertion data
        const authenticatorData = new Uint8Array(response.authenticatorData)
        const clientDataJSON = new Uint8Array(response.clientDataJSON)
        const signature = new Uint8Array(response.signature)

        // Phase 3: Submit to blockchain
        setStatus('submitting')
        currentPhase = 'submitting'

        // Encode content to bytes
        const contentBytes = new TextEncoder().encode(content)

        // Build transaction
        const tx = api.tx.Post.create_post_with_webauthn(
          identityId,
          Binary.fromBytes(passkeyId),
          Binary.fromBytes(contentBytes),
          Binary.fromBytes(authenticatorData),
          Binary.fromBytes(clientDataJSON),
          Binary.fromBytes(signature)
        )

        // Phase 4: Wait for confirmation
        setStatus('confirming')
        currentPhase = 'confirming'

        const result = await tx.signAndSubmit(signer)

        // Extract post ID and moral spent from events
        const postId = extractPostIdFromEvents(result.events || [])
        const moralSpent = extractMoralSpentFromEvents(result.events || [])

        // Success
        setStatus('success')
        const successResult: PostResult = {
          success: true,
          postId,
          txHash: result.txHash || result.block?.hash,
          moralSpent,
        }
        onSuccess?.(successResult)
        return successResult
      } catch (err) {
        // Classify error based on current phase
        let errorCode: SigningErrorCode

        if (currentPhase === 'hashing') {
          errorCode = 'AUTHENTICATOR_ERROR'
        } else if (currentPhase === 'authenticating') {
          errorCode = mapWebAuthnError(err)
        } else {
          // submitting or confirming
          errorCode = mapTransactionError(err)
        }

        const signingError = createSigningError(errorCode, err)
        setStatus('error')
        setError(signingError)
        onError?.(signingError)
        return { success: false, error: signingError }
      }
    },
    [api, signer, identityId, passkeyId, credentialId, onSuccess, onError]
  )

  return {
    status,
    sign,
    estimateCost,
    reset,
    error,
  }
}
