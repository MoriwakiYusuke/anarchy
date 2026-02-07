import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { renderHook, act, waitFor } from '@testing-library/react'
import { useWebAuthn } from '../hooks/useWebAuthn'
import {
  createMockCredentials,
  setupRegistrationMock,
  setupAssertionMock,
  MockCredentialsContainer,
  MockPublicKeyCredential,
} from './setup'
import {
  RegistrationStatus,
  SigningStatus,
  RegistrationErrorCode,
  SigningErrorCode,
} from '../types/webauthn'
import { base64UrlEncode, generateWysiwysChallenge } from '../utils/webauthn'

// Mock PAPI API with all required methods
const createMockApi = (options?: {
  registerShouldFail?: boolean
  addPasskeyShouldFail?: boolean
  postShouldFail?: boolean
  errorType?: string
  identityId?: bigint
  passkeyId?: Uint8Array
  postId?: bigint
  moralSpent?: bigint
  identity?: {
    passkey_count: number
    passkeys: Array<{ passkey_id: Uint8Array; device_name?: string }>
  } | null
}) => {
  const {
    registerShouldFail = false,
    addPasskeyShouldFail = false,
    postShouldFail = false,
    errorType = 'TransactionFailed',
    identityId = 42n,
    passkeyId = new Uint8Array([1, 2, 3, 4]),
    postId = 1n,
    moralSpent = 15000000000000n,
    identity = {
      passkey_count: 1,
      passkeys: [{ passkey_id: passkeyId, device_name: 'Test Device' }],
    },
  } = options ?? {}

  const mockRegisterTx = {
    signAndSubmit: vi.fn().mockImplementation(async () => {
      if (registerShouldFail) {
        throw new Error(errorType)
      }
      return {
        ok: true,
        block: { hash: '0x1234567890' },
        txHash: '0xregister123',
        events: [
          {
            event: {
              type: 'Identity',
              value: {
                type: 'IdentityCreated',
                value: {
                  identity_id: identityId,
                  passkey_id: passkeyId,
                },
              },
            },
          },
        ],
      }
    }),
  }

  const mockAddPasskeyTx = {
    signAndSubmit: vi.fn().mockImplementation(async () => {
      if (addPasskeyShouldFail) {
        throw new Error(errorType)
      }
      return {
        ok: true,
        block: { hash: '0x9876543210' },
        txHash: '0xaddpasskey456',
        events: [
          {
            event: {
              type: 'Identity',
              value: {
                type: 'PasskeyAdded',
                value: {
                  identity_id: identityId,
                  passkey_id: new Uint8Array([5, 6, 7, 8]),
                },
              },
            },
          },
        ],
      }
    }),
  }

  const mockPostTx = {
    signAndSubmit: vi.fn().mockImplementation(async () => {
      if (postShouldFail) {
        throw new Error(errorType)
      }
      return {
        ok: true,
        block: { hash: '0xabc123' },
        txHash: '0xpost789',
        events: [
          {
            event: {
              type: 'Post',
              value: {
                type: 'PostCreated',
                value: {
                  post_id: postId,
                  identity_id: identityId,
                  moral_spent: moralSpent,
                },
              },
            },
          },
        ],
      }
    }),
  }

  return {
    tx: {
      Identity: {
        register_identity: vi.fn().mockReturnValue(mockRegisterTx),
        add_passkey: vi.fn().mockReturnValue(mockAddPasskeyTx),
      },
      Post: {
        create_post_with_webauthn: vi.fn().mockReturnValue(mockPostTx),
      },
    },
    query: {
      Identity: {
        Identities: {
          getValue: vi.fn().mockResolvedValue(identity),
        },
      },
    },
  }
}

const createMockSigner = () => ({
  publicKey: new Uint8Array(32).fill(1),
  sign: vi.fn().mockResolvedValue(new Uint8Array(64).fill(2)),
})

describe('useWebAuthn', () => {
  let originalCredentials: CredentialsContainer | undefined
  let originalPublicKeyCredential: typeof PublicKeyCredential | undefined
  let mockCreds: MockCredentialsContainer
  const testIdentityId = 42n
  const testPasskeyId = new Uint8Array([1, 2, 3, 4])
  const testCredentialId = base64UrlEncode(new TextEncoder().encode('test-credential-id'))

  beforeEach(() => {
    originalCredentials = navigator.credentials
    originalPublicKeyCredential = window.PublicKeyCredential
    vi.clearAllMocks()

    mockCreds = createMockCredentials()

    Object.defineProperty(navigator, 'credentials', {
      value: mockCreds,
      configurable: true,
      writable: true,
    })

    Object.defineProperty(window, 'PublicKeyCredential', {
      value: MockPublicKeyCredential,
      configurable: true,
      writable: true,
    })

    ;(MockPublicKeyCredential as any).isUserVerifyingPlatformAuthenticatorAvailable = vi
      .fn()
      .mockResolvedValue(true)
    ;(MockPublicKeyCredential as any).isConditionalMediationAvailable = vi
      .fn()
      .mockResolvedValue(true)
  })

  afterEach(() => {
    if (originalCredentials) {
      Object.defineProperty(navigator, 'credentials', {
        value: originalCredentials,
        configurable: true,
        writable: true,
      })
    }

    if (originalPublicKeyCredential) {
      Object.defineProperty(window, 'PublicKeyCredential', {
        value: originalPublicKeyCredential,
        configurable: true,
        writable: true,
      })
    }

    vi.restoreAllMocks()
  })

  describe('Feature Detection (useWebAuthnSupport integration)', () => {
    it('should report WebAuthn as supported when available', async () => {
      const api = createMockApi()
      const signer = createMockSigner()

      const { result } = renderHook(() => useWebAuthn({ api, signer }))

      await waitFor(() => {
        expect(result.current.isSupported).toBe(true)
      })
    })

    it('should report platform authenticator availability', async () => {
      const api = createMockApi()
      const signer = createMockSigner()

      const { result } = renderHook(() => useWebAuthn({ api, signer }))

      await waitFor(() => {
        expect(result.current.hasPlatformAuthenticator).toBe(true)
      })
    })

    it('should handle missing WebAuthn API', async () => {
      Object.defineProperty(window, 'PublicKeyCredential', {
        value: undefined,
        configurable: true,
        writable: true,
      })

      const api = createMockApi()
      const signer = createMockSigner()

      const { result } = renderHook(() => useWebAuthn({ api, signer }))

      await waitFor(() => {
        expect(result.current.isSupported).toBe(false)
      })
    })
  })

  describe('Initial Registration (useWebAuthnRegistration integration)', () => {
    it('should have idle registration status initially', () => {
      const api = createMockApi()
      const signer = createMockSigner()

      const { result } = renderHook(() => useWebAuthn({ api, signer }))

      expect(result.current.registrationStatus).toBe('idle')
    })

    it('should register new identity with passkey', async () => {
      const api = createMockApi()
      const signer = createMockSigner()
      setupRegistrationMock(mockCreds, 'new-credential-id')

      const { result } = renderHook(() => useWebAuthn({ api, signer }))

      let registerResult: any
      await act(async () => {
        registerResult = await result.current.registerPasskey('MacBook Pro')
      })

      expect(registerResult.success).toBe(true)
      expect(registerResult.identityId).toBe(testIdentityId)
      expect(api.tx.Identity.register_identity).toHaveBeenCalled()
    })

    it('should update identity state after successful registration', async () => {
      const api = createMockApi()
      const signer = createMockSigner()
      setupRegistrationMock(mockCreds, 'new-credential-id')

      const { result } = renderHook(() => useWebAuthn({ api, signer }))

      expect(result.current.identity).toBeNull()

      await act(async () => {
        await result.current.registerPasskey('MacBook Pro')
      })

      await waitFor(() => {
        expect(result.current.identity).not.toBeNull()
        expect(result.current.identity?.identityId).toBe(testIdentityId)
      })
    })

    it('should handle registration failure', async () => {
      const api = createMockApi({ registerShouldFail: true })
      const signer = createMockSigner()
      setupRegistrationMock(mockCreds, 'new-credential-id')

      const { result } = renderHook(() => useWebAuthn({ api, signer }))

      let registerResult: any
      await act(async () => {
        registerResult = await result.current.registerPasskey('MacBook Pro')
      })

      expect(registerResult.success).toBe(false)
      expect(registerResult.error).toBeDefined()
    })

    it('should handle user cancellation during registration', async () => {
      const api = createMockApi()
      const signer = createMockSigner()

      // Simulate user cancellation
      const error = Object.assign(new Error('User cancelled'), { name: 'NotAllowedError' })
      mockCreds.create.mockRejectedValue(error)

      const { result } = renderHook(() => useWebAuthn({ api, signer }))

      let registerResult: any
      await act(async () => {
        registerResult = await result.current.registerPasskey()
      })

      expect(registerResult.success).toBe(false)
      expect(registerResult.error?.code).toBe('USER_CANCELLED')
    })
  })

  describe('addPasskey (New Feature)', () => {
    it('should add passkey to existing identity', async () => {
      const api = createMockApi()
      const signer = createMockSigner()
      setupRegistrationMock(mockCreds, 'new-device-credential')

      const { result } = renderHook(() =>
        useWebAuthn({
          api,
          signer,
          initialIdentity: {
            identityId: testIdentityId,
            passkeyId: testPasskeyId,
            credentialId: testCredentialId,
            deviceName: 'MacBook Pro',
          },
        })
      )

      let addResult: any
      await act(async () => {
        addResult = await result.current.addPasskey('iPhone 15')
      })

      expect(addResult.success).toBe(true)
      expect(addResult.passkeyId).toBeDefined()
      expect(api.tx.Identity.add_passkey).toHaveBeenCalled()

      // Should be called with identity_id as first argument
      const callArgs = api.tx.Identity.add_passkey.mock.calls[0][0]
      expect(callArgs.identity_id).toBe(testIdentityId)
      expect(callArgs.device_name).toBeDefined()
    })

    it('should fail when no identity is set', async () => {
      const api = createMockApi()
      const signer = createMockSigner()
      setupRegistrationMock(mockCreds, 'new-device-credential')

      const { result } = renderHook(() => useWebAuthn({ api, signer }))

      let addResult: any
      await act(async () => {
        addResult = await result.current.addPasskey('iPhone 15')
      })

      expect(addResult.success).toBe(false)
      expect(addResult.error?.code).toBe('NO_IDENTITY')
    })

    it('should fail when api is not available', async () => {
      const signer = createMockSigner()
      setupRegistrationMock(mockCreds, 'new-device-credential')

      const { result } = renderHook(() =>
        useWebAuthn({
          api: null,
          signer,
          initialIdentity: {
            identityId: testIdentityId,
            passkeyId: testPasskeyId,
            credentialId: testCredentialId,
          },
        })
      )

      let addResult: any
      await act(async () => {
        addResult = await result.current.addPasskey('iPhone 15')
      })

      expect(addResult.success).toBe(false)
      expect(addResult.error?.code).toBe('API_NOT_AVAILABLE')
    })

    it('should fail when signer is not available', async () => {
      const api = createMockApi()
      setupRegistrationMock(mockCreds, 'new-device-credential')

      const { result } = renderHook(() =>
        useWebAuthn({
          api,
          signer: null,
          initialIdentity: {
            identityId: testIdentityId,
            passkeyId: testPasskeyId,
            credentialId: testCredentialId,
          },
        })
      )

      let addResult: any
      await act(async () => {
        addResult = await result.current.addPasskey('iPhone 15')
      })

      expect(addResult.success).toBe(false)
      expect(addResult.error?.code).toBe('SIGNER_NOT_AVAILABLE')
    })

    it('should handle transaction failure during add_passkey', async () => {
      const api = createMockApi({ addPasskeyShouldFail: true, errorType: 'GenericTransactionError' })
      const signer = createMockSigner()
      setupRegistrationMock(mockCreds, 'new-device-credential')

      const { result } = renderHook(() =>
        useWebAuthn({
          api,
          signer,
          initialIdentity: {
            identityId: testIdentityId,
            passkeyId: testPasskeyId,
            credentialId: testCredentialId,
          },
        })
      )

      let addResult: any
      await act(async () => {
        addResult = await result.current.addPasskey('iPhone 15')
      })

      expect(addResult.success).toBe(false)
      expect(addResult.error?.code).toBe('TRANSACTION_FAILED')
    })

    it('should handle passkey already registered error', async () => {
      const api = createMockApi({
        addPasskeyShouldFail: true,
        errorType: 'PasskeyAlreadyRegistered',
      })
      const signer = createMockSigner()
      setupRegistrationMock(mockCreds, 'new-device-credential')

      const { result } = renderHook(() =>
        useWebAuthn({
          api,
          signer,
          initialIdentity: {
            identityId: testIdentityId,
            passkeyId: testPasskeyId,
            credentialId: testCredentialId,
          },
        })
      )

      let addResult: any
      await act(async () => {
        addResult = await result.current.addPasskey('iPhone 15')
      })

      expect(addResult.success).toBe(false)
      expect(addResult.error?.code).toBe('PASSKEY_ALREADY_REGISTERED')
    })

    it('should handle too many passkeys error', async () => {
      const api = createMockApi({
        addPasskeyShouldFail: true,
        errorType: 'TooManyPasskeys',
      })
      const signer = createMockSigner()
      setupRegistrationMock(mockCreds, 'new-device-credential')

      const { result } = renderHook(() =>
        useWebAuthn({
          api,
          signer,
          initialIdentity: {
            identityId: testIdentityId,
            passkeyId: testPasskeyId,
            credentialId: testCredentialId,
          },
        })
      )

      let addResult: any
      await act(async () => {
        addResult = await result.current.addPasskey('iPhone 15')
      })

      expect(addResult.success).toBe(false)
      expect(addResult.error?.code).toBe('TOO_MANY_PASSKEYS')
    })

    it('should handle user cancellation during add passkey', async () => {
      const api = createMockApi()
      const signer = createMockSigner()

      const error = Object.assign(new Error('User cancelled'), { name: 'NotAllowedError' })
      mockCreds.create.mockRejectedValue(error)

      const { result } = renderHook(() =>
        useWebAuthn({
          api,
          signer,
          initialIdentity: {
            identityId: testIdentityId,
            passkeyId: testPasskeyId,
            credentialId: testCredentialId,
          },
        })
      )

      let addResult: any
      await act(async () => {
        addResult = await result.current.addPasskey('iPhone 15')
      })

      expect(addResult.success).toBe(false)
      expect(addResult.error?.code).toBe('USER_CANCELLED')
    })
  })

  describe('loadIdentityById (New Feature)', () => {
    it('should load identity from chain', async () => {
      const mockIdentity = {
        passkey_count: 2,
        passkeys: [
          { passkey_id: new Uint8Array([1, 2, 3, 4]), device_name: 'MacBook Pro' },
          { passkey_id: new Uint8Array([5, 6, 7, 8]), device_name: 'iPhone 15' },
        ],
      }
      const api = createMockApi({ identity: mockIdentity })
      const signer = createMockSigner()

      const { result } = renderHook(() => useWebAuthn({ api, signer }))

      await act(async () => {
        await result.current.loadIdentityById(testIdentityId, testCredentialId)
      })

      await waitFor(() => {
        expect(result.current.identity).not.toBeNull()
        expect(result.current.identity?.identityId).toBe(testIdentityId)
      })

      expect(api.query.Identity.Identities.getValue).toHaveBeenCalledWith(testIdentityId)
    })

    it('should handle identity not found', async () => {
      const api = createMockApi({ identity: null })
      const signer = createMockSigner()

      const { result } = renderHook(() => useWebAuthn({ api, signer }))

      await act(async () => {
        try {
          await result.current.loadIdentityById(999n, testCredentialId)
        } catch (e) {
          // Expected to throw
        }
      })

      await waitFor(() => {
        expect(result.current.error).toBeDefined()
        expect(result.current.error?.code).toBe('IDENTITY_NOT_FOUND')
      })
    })

    it('should fail when api is not available', async () => {
      const signer = createMockSigner()

      const { result } = renderHook(() => useWebAuthn({ api: null, signer }))

      await act(async () => {
        try {
          await result.current.loadIdentityById(testIdentityId, testCredentialId)
        } catch (e) {
          // Expected to throw
        }
      })

      await waitFor(() => {
        expect(result.current.error).toBeDefined()
        expect(result.current.error?.code).toBe('API_NOT_AVAILABLE')
      })
    })

    it('should update passkey ID from loaded identity when matching credential found', async () => {
      const mockPasskeyId = new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8])
      const mockIdentity = {
        passkey_count: 1,
        passkeys: [
          { passkey_id: mockPasskeyId, device_name: 'MacBook Pro', cose_public_key: new Uint8Array(77) },
        ],
      }
      const api = createMockApi({ identity: mockIdentity })
      const signer = createMockSigner()

      const { result } = renderHook(() => useWebAuthn({ api, signer }))

      await act(async () => {
        await result.current.loadIdentityById(testIdentityId, testCredentialId)
      })

      await waitFor(() => {
        expect(result.current.identity).not.toBeNull()
        expect(result.current.identity?.passkeyId).toBeDefined()
      })
    })
  })

  describe('Signing (useWebAuthnSigning integration)', () => {
    it('should have idle signing status initially', () => {
      const api = createMockApi()
      const signer = createMockSigner()

      const { result } = renderHook(() =>
        useWebAuthn({
          api,
          signer,
          initialIdentity: {
            identityId: testIdentityId,
            passkeyId: testPasskeyId,
            credentialId: testCredentialId,
          },
        })
      )

      expect(result.current.signingStatus).toBe('idle')
    })

    it('should sign and post content', async () => {
      const api = createMockApi()
      const signer = createMockSigner()

      // Setup assertion mock for signing
      const challenge = await generateWysiwysChallenge('Hello, World!')
      setupAssertionMock(mockCreds, challenge, 'test-credential-id')

      const { result } = renderHook(() =>
        useWebAuthn({
          api,
          signer,
          initialIdentity: {
            identityId: testIdentityId,
            passkeyId: testPasskeyId,
            credentialId: testCredentialId,
          },
        })
      )

      let postResult: any
      await act(async () => {
        postResult = await result.current.signAndPost('Hello, World!')
      })

      expect(postResult.success).toBe(true)
      expect(postResult.postId).toBe(1n)
      expect(api.tx.Post.create_post_with_webauthn).toHaveBeenCalled()
    })

    it('should fail signing when no identity is set', async () => {
      const api = createMockApi()
      const signer = createMockSigner()

      const { result } = renderHook(() => useWebAuthn({ api, signer }))

      let postResult: any
      await act(async () => {
        postResult = await result.current.signAndPost('Hello, World!')
      })

      expect(postResult.success).toBe(false)
      expect(postResult.error?.code).toBe('NO_IDENTITY')
    })

    it('should handle signing failure', async () => {
      const api = createMockApi({ postShouldFail: true })
      const signer = createMockSigner()

      const challenge = await generateWysiwysChallenge('Hello, World!')
      setupAssertionMock(mockCreds, challenge, 'test-credential-id')

      const { result } = renderHook(() =>
        useWebAuthn({
          api,
          signer,
          initialIdentity: {
            identityId: testIdentityId,
            passkeyId: testPasskeyId,
            credentialId: testCredentialId,
          },
        })
      )

      let postResult: any
      await act(async () => {
        postResult = await result.current.signAndPost('Hello, World!')
      })

      expect(postResult.success).toBe(false)
      expect(postResult.error).toBeDefined()
    })
  })

  describe('reset', () => {
    it('should reset all states', async () => {
      const api = createMockApi()
      const signer = createMockSigner()
      setupRegistrationMock(mockCreds, 'new-credential-id')

      const { result } = renderHook(() => useWebAuthn({ api, signer }))

      // First register
      await act(async () => {
        await result.current.registerPasskey('MacBook Pro')
      })

      await waitFor(() => {
        expect(result.current.identity).not.toBeNull()
      })

      // Then reset
      act(() => {
        result.current.reset()
      })

      expect(result.current.identity).toBeNull()
      expect(result.current.registrationStatus).toBe('idle')
      expect(result.current.signingStatus).toBe('idle')
      expect(result.current.error).toBeNull()
    })
  })

  describe('Error state', () => {
    it('should clear error on successful operation', async () => {
      const api = createMockApi()
      const signer = createMockSigner()

      // First fail
      const error = Object.assign(new Error('User cancelled'), { name: 'NotAllowedError' })
      mockCreds.create.mockRejectedValueOnce(error)

      const { result } = renderHook(() => useWebAuthn({ api, signer }))

      await act(async () => {
        await result.current.registerPasskey()
      })

      expect(result.current.error).not.toBeNull()

      // Then succeed
      setupRegistrationMock(mockCreds, 'new-credential-id')

      await act(async () => {
        await result.current.registerPasskey()
      })

      expect(result.current.error).toBeNull()
    })
  })

  describe('initialIdentity option', () => {
    it('should initialize with provided identity', () => {
      const api = createMockApi()
      const signer = createMockSigner()

      const { result } = renderHook(() =>
        useWebAuthn({
          api,
          signer,
          initialIdentity: {
            identityId: testIdentityId,
            passkeyId: testPasskeyId,
            credentialId: testCredentialId,
            deviceName: 'MacBook Pro',
          },
        })
      )

      expect(result.current.identity).not.toBeNull()
      expect(result.current.identity?.identityId).toBe(testIdentityId)
      expect(result.current.identity?.passkeyId).toEqual(testPasskeyId)
      expect(result.current.identity?.credentialId).toBe(testCredentialId)
      expect(result.current.identity?.deviceName).toBe('MacBook Pro')
    })

    it('should allow registering new identity even with initialIdentity', async () => {
      const api = createMockApi({ identityId: 100n })
      const signer = createMockSigner()
      setupRegistrationMock(mockCreds, 'brand-new-credential')

      const { result } = renderHook(() =>
        useWebAuthn({
          api,
          signer,
          initialIdentity: {
            identityId: testIdentityId,
            passkeyId: testPasskeyId,
            credentialId: testCredentialId,
          },
        })
      )

      expect(result.current.identity?.identityId).toBe(testIdentityId)

      // Register creates a NEW identity (not add passkey)
      let registerResult: any
      await act(async () => {
        registerResult = await result.current.registerPasskey('New Device')
      })

      expect(registerResult.success).toBe(true)
      expect(registerResult.identityId).toBe(100n)
    })
  })

  describe('callbacks', () => {
    it('should call onRegistrationSuccess callback', async () => {
      const api = createMockApi()
      const signer = createMockSigner()
      setupRegistrationMock(mockCreds, 'new-credential-id')

      const onRegistrationSuccess = vi.fn()

      const { result } = renderHook(() =>
        useWebAuthn({
          api,
          signer,
          onRegistrationSuccess,
        })
      )

      await act(async () => {
        await result.current.registerPasskey()
      })

      expect(onRegistrationSuccess).toHaveBeenCalled()
      expect(onRegistrationSuccess).toHaveBeenCalledWith(expect.objectContaining({
        success: true,
        identityId: testIdentityId,
      }))
    })

    it('should call onPostSuccess callback', async () => {
      const api = createMockApi()
      const signer = createMockSigner()

      const challenge = await generateWysiwysChallenge('Hello!')
      setupAssertionMock(mockCreds, challenge, 'test-credential-id')

      const onPostSuccess = vi.fn()

      const { result } = renderHook(() =>
        useWebAuthn({
          api,
          signer,
          initialIdentity: {
            identityId: testIdentityId,
            passkeyId: testPasskeyId,
            credentialId: testCredentialId,
          },
          onPostSuccess,
        })
      )

      await act(async () => {
        await result.current.signAndPost('Hello!')
      })

      expect(onPostSuccess).toHaveBeenCalled()
      expect(onPostSuccess).toHaveBeenCalledWith(expect.objectContaining({
        success: true,
        postId: 1n,
      }))
    })

    it('should call onError callback on failure', async () => {
      const api = createMockApi()
      const signer = createMockSigner()

      const error = Object.assign(new Error('User cancelled'), { name: 'NotAllowedError' })
      mockCreds.create.mockRejectedValue(error)

      const onError = vi.fn()

      const { result } = renderHook(() =>
        useWebAuthn({
          api,
          signer,
          onError,
        })
      )

      await act(async () => {
        await result.current.registerPasskey()
      })

      expect(onError).toHaveBeenCalled()
      expect(onError).toHaveBeenCalledWith(expect.objectContaining({
        code: 'USER_CANCELLED',
      }))
    })
  })
})
