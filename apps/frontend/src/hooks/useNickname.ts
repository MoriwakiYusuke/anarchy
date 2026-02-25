'use client'

/**
 * useNickname Hook
 * 
 * T-040: useNickname hook for querying and setting nicknames
 * 
 * Features:
 * - Query current nickname for an account
 * - Set new nickname (with validation)
 * - Clear nickname
 * - State machine: idle → pending → success | error
 */

import { useState, useCallback, useEffect } from 'react'
import type { PolkadotSigner } from 'polkadot-api'

/** Maximum nickname length in bytes (UTF-8) */
const MAX_NICKNAME_BYTES = 128

export interface NicknameState {
  status: 'idle' | 'pending' | 'success' | 'error'
  txHash?: string
}

export interface UseNicknameOptions {
  /** PAPI client instance */
  client: unknown
  /** Unsafe API with tx/query access */
  unsafeApi: unknown
  /** Account to query/set nickname for */
  accountId: string | null
  /** Signer for transactions */
  signer?: unknown
  /** Callback on successful nickname set */
  onSuccess?: (nickname: string | null) => void
  /** Callback on error */
  onError?: (error: string) => void
}

export interface UseNicknameResult {
  /** Current nickname (null if not set) */
  nickname: string | null
  /** Whether a query/transaction is in progress */
  isLoading: boolean
  /** Error message (i18n key) */
  error: string | null
  /** Current state */
  state: NicknameState
  /** Set a new nickname */
  setNickname: (nickname: string) => Promise<void>
  /** Clear the current nickname */
  clearNickname: () => Promise<void>
  /** Refetch the current nickname */
  refetch: () => Promise<void>
}

/**
 * Get UTF-8 byte length of a string
 */
function getByteLength(str: string): number {
  return new TextEncoder().encode(str).length
}

/**
 * Hook for managing account nicknames
 */
export function useNickname({
  client,
  unsafeApi,
  accountId,
  signer,
  onSuccess,
  onError,
}: UseNicknameOptions): UseNicknameResult {
  const [nickname, setNicknameState] = useState<string | null>(null)
  const [isLoading, setIsLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [state, setState] = useState<NicknameState>({ status: 'idle' })

  // Fetch nickname on mount and when accountId changes
  const refetch = useCallback(async () => {
    if (!unsafeApi || !accountId) {
      setNicknameState(null)
      return
    }

    setIsLoading(true)
    setError(null)

    try {
      const api = unsafeApi as {
        query: {
          Nickname: {
            Nicknames: {
              getValue: (accountId: string) => Promise<Uint8Array | null>
            }
          }
        }
      }

      const result = await api.query.Nickname.Nicknames.getValue(accountId)
      
      if (result) {
        // Handle PAPI Binary type or raw Uint8Array
        let bytes: Uint8Array
        if (result && typeof result === 'object' && 'asBytes' in result && typeof (result as { asBytes: () => Uint8Array }).asBytes === 'function') {
          bytes = (result as { asBytes: () => Uint8Array }).asBytes()
        } else if (result instanceof Uint8Array) {
          bytes = result
        } else if (Array.isArray(result)) {
          bytes = new Uint8Array(result as number[])
        } else {
          bytes = new Uint8Array(result as ArrayLike<number>)
        }
        
        const decoded = new TextDecoder().decode(bytes)
        setNicknameState(decoded)
      } else {
        setNicknameState(null)
      }
    } catch (err) {
      console.error('[useNickname] Query failed:', err)
      setError('error.nicknameQueryFailed')
    } finally {
      setIsLoading(false)
    }
  }, [unsafeApi, accountId])

  // Initial fetch
  useEffect(() => {
    refetch()
  }, [refetch])

  // Set nickname
  const setNickname = useCallback(async (newNickname: string) => {
    // Validation
    if (!newNickname || newNickname.trim() === '') {
      setError('error.nicknameEmpty')
      setState({ status: 'error' })
      return
    }

    if (getByteLength(newNickname) > MAX_NICKNAME_BYTES) {
      setError('error.nicknameTooLong')
      setState({ status: 'error' })
      return
    }

    if (!signer) {
      setError('error.signerRequired')
      setState({ status: 'error' })
      return
    }

    if (!unsafeApi) {
      setError('error.apiNotReady')
      setState({ status: 'error' })
      return
    }

    setError(null)
    setState({ status: 'pending' })

    try {
      const api = unsafeApi as {
        tx: {
          Nickname: {
            set_nickname: (params: { nickname: Uint8Array }) => {
              signAndSubmit: (signer: unknown) => Promise<{ txHash: string }>
            }
          }
        }
      }

      const nicknameBytes = new TextEncoder().encode(newNickname)
      const tx = api.tx.Nickname.set_nickname({ nickname: nicknameBytes })
      const result = await tx.signAndSubmit(signer)

      setState({ status: 'success', txHash: (result as { txHash?: string }).txHash })
      setNicknameState(newNickname)
      onSuccess?.(newNickname)

      // Refetch to sync with chain
      await refetch()
    } catch (err) {
      console.error('[useNickname] Set failed:', err)
      setError('error.nicknameSetFailed')
      setState({ status: 'error' })
      onError?.('error.nicknameSetFailed')
    }
  }, [unsafeApi, signer, onSuccess, onError, refetch])

  // Clear nickname
  const clearNickname = useCallback(async () => {
    if (!signer) {
      setError('error.signerRequired')
      setState({ status: 'error' })
      return
    }

    if (!unsafeApi) {
      setError('error.apiNotReady')
      setState({ status: 'error' })
      return
    }

    setError(null)
    setState({ status: 'pending' })

    try {
      const api = unsafeApi as {
        tx: {
          Nickname: {
            clear_nickname: () => {
              signAndSubmit: (signer: unknown) => Promise<{ txHash: string }>
            }
          }
        }
      }

      const tx = api.tx.Nickname.clear_nickname()
      const result = await tx.signAndSubmit(signer)

      setState({ status: 'success', txHash: (result as { txHash?: string }).txHash })
      setNicknameState(null)
      onSuccess?.(null)

      // Refetch to sync with chain
      await refetch()
    } catch (err) {
      console.error('[useNickname] Clear failed:', err)
      setError('error.nicknameClearFailed')
      setState({ status: 'error' })
      onError?.('error.nicknameClearFailed')
    }
  }, [unsafeApi, signer, onSuccess, onError, refetch])

  return {
    nickname,
    isLoading,
    error,
    state,
    setNickname,
    clearNickname,
    refetch,
  }
}
