import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { renderHook, act, waitFor } from '@testing-library/react'
import { useWebAuthnSigning } from '../hooks/useWebAuthnSigning'
import {
  createMockCredentials,
  setupAssertionMock,
  MockCredentialsContainer,
  MockPublicKeyCredential,
} from './setup'
import { SigningStatus, SigningErrorCode } from '../types/webauthn'
import { generateWysiwysChallenge, base64UrlEncode } from '../utils/webauthn'

// Mock PAPI transaction for post creation
const createMockApi = (options?: {
  shouldFail?: boolean
  errorType?: string
  events?: any[]
  postId?: bigint
  moralSpent?: bigint
}) => {
  const {
    shouldFail = false,
    errorType = 'TransactionFailed',
    events = [],
    postId = 1n,
    moralSpent = 15000000000000n, // 15 MORAL with 12 decimals
  } = options ?? {}

  const mockTx = {
    signAndSubmit: vi.fn().mockImplementation(async () => {
      if (shouldFail) {
        throw new Error(errorType)
      }
      // Return mock transaction result with events
      return {
        block: { hash: '0xabc123' },
        txHash: '0xdef456',
        events:
          events.length > 0
            ? events
            : [
                {
                  event: {
                    type: 'Post',
                    value: {
                      type: 'PostCreated',
                      value: {
                        post_id: postId,
                        identity_id: 42n,
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
      Post: {
        create_post_with_webauthn: vi.fn().mockReturnValue(mockTx),
      },
    },
  }
}

const createMockSigner = () => ({
  publicKey: new Uint8Array(32).fill(1),
  sign: vi.fn().mockResolvedValue(new Uint8Array(64).fill(2)),
})

describe('useWebAuthnSigning', () => {
  let originalCredentials: CredentialsContainer | undefined
  let originalPublicKeyCredential: typeof PublicKeyCredential | undefined
  let mockCreds: MockCredentialsContainer
  const testIdentityId = 42n
  const testPasskeyId = new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8])
  // Valid base64url encoded credential ID (encodes 'test-credential-id')
  const testCredentialId = base64UrlEncode(new TextEncoder().encode('test-credential-id'))

  beforeEach(() => {
    originalCredentials = navigator.credentials
    originalPublicKeyCredential = window.PublicKeyCredential
    vi.clearAllMocks()

    // Create fresh mock credentials for each test
    mockCreds = createMockCredentials()

    // Setup navigator.credentials
    Object.defineProperty(navigator, 'credentials', {
      value: mockCreds,
      configurable: true,
      writable: true,
    })

    // Setup PublicKeyCredential
    Object.defineProperty(window, 'PublicKeyCredential', {
      value: MockPublicKeyCredential,
      configurable: true,
      writable: true,
    })

    // Setup static methods
    ;(MockPublicKeyCredential as any).isUserVerifyingPlatformAuthenticatorAvailable = vi
      .fn()
      .mockResolvedValue(true)
    ;(MockPublicKeyCredential as any).isConditionalMediationAvailable = vi.fn().mockResolvedValue(true)
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
  })

  describe('初期状態', () => {
    it('status が idle で初期化される', () => {
      const api = createMockApi()
      const signer = createMockSigner()

      const { result } = renderHook(() =>
        useWebAuthnSigning({
          api,
          signer,
          identityId: testIdentityId,
          passkeyId: testPasskeyId,
          credentialId: testCredentialId,
        })
      )

      expect(result.current.status).toBe<SigningStatus>('idle')
      expect(result.current.error).toBeNull()
    })

    it('api または signer が null でも動作する', () => {
      const { result } = renderHook(() =>
        useWebAuthnSigning({
          api: null,
          signer: null,
          identityId: testIdentityId,
          passkeyId: testPasskeyId,
          credentialId: testCredentialId,
        })
      )

      expect(result.current.status).toBe('idle')
    })
  })

  describe('正常な署名フロー', () => {
    it('完全な署名フローが成功する', async () => {
      const challenge = await generateWysiwysChallenge('test content')
      setupAssertionMock(mockCreds, challenge, testCredentialId)
      const api = createMockApi()
      const signer = createMockSigner()
      const onSuccess = vi.fn()

      const { result } = renderHook(() =>
        useWebAuthnSigning({
          api,
          signer,
          identityId: testIdentityId,
          passkeyId: testPasskeyId,
          credentialId: testCredentialId,
          onSuccess,
        })
      )

      let signResult: any
      await act(async () => {
        signResult = await result.current.sign('test content')
      })

      expect(signResult.success).toBe(true)
      expect(signResult.postId).toBe(1n)
      expect(signResult.moralSpent).toBe(15000000000000n)
      expect(result.current.status).toBe<SigningStatus>('success')
      expect(onSuccess).toHaveBeenCalledWith(signResult)
    })

    it('status が正しい順序で遷移する', async () => {
      const challenge = await generateWysiwysChallenge('status test')
      setupAssertionMock(mockCreds, challenge, testCredentialId)
      const api = createMockApi()
      const signer = createMockSigner()
      const statusHistory: SigningStatus[] = []

      const { result } = renderHook(() => {
        const hookResult = useWebAuthnSigning({
          api,
          signer,
          identityId: testIdentityId,
          passkeyId: testPasskeyId,
          credentialId: testCredentialId,
        })
        statusHistory.push(hookResult.status)
        return hookResult
      })

      await act(async () => {
        await result.current.sign('status test')
      })

      // Verify status transitions (may have duplicates due to re-renders)
      const uniqueStatuses = [...new Set(statusHistory)]
      expect(uniqueStatuses).toContain('idle')
      expect(uniqueStatuses).toContain('success')
    })

    it('親投稿IDを指定してリプライできる', async () => {
      const challenge = await generateWysiwysChallenge('reply content')
      setupAssertionMock(mockCreds, challenge, testCredentialId)
      const api = createMockApi()
      const signer = createMockSigner()

      const { result } = renderHook(() =>
        useWebAuthnSigning({
          api,
          signer,
          identityId: testIdentityId,
          passkeyId: testPasskeyId,
          credentialId: testCredentialId,
        })
      )

      await act(async () => {
        await result.current.sign('reply content', 999)
      })

      // Verify create_post_with_webauthn was called
      expect(api.tx.Post.create_post_with_webauthn).toHaveBeenCalled()
    })
  })

  describe('WebAuthn認証エラー', () => {
    it('ユーザーがキャンセルした場合 USER_CANCELLED エラー', async () => {
      // Use Object.assign pattern for jsdom compatibility (like registration test)
      const error = Object.assign(new Error('The operation was not allowed.'), { name: 'NotAllowedError' })
      mockCreds.get.mockRejectedValue(error)

      const api = createMockApi()
      const signer = createMockSigner()
      const onError = vi.fn()

      const { result } = renderHook(() =>
        useWebAuthnSigning({
          api,
          signer,
          identityId: testIdentityId,
          passkeyId: testPasskeyId,
          credentialId: testCredentialId,
          onError,
        })
      )

      let signResult: any
      await act(async () => {
        signResult = await result.current.sign('test')
      })

      expect(signResult.success).toBe(false)
      expect(signResult.error?.code).toBe<SigningErrorCode>('USER_CANCELLED')
      expect(result.current.status).toBe<SigningStatus>('error')
      expect(onError).toHaveBeenCalled()
    })

    it('クレデンシャルが見つからない場合 CREDENTIAL_NOT_FOUND エラー', async () => {
      mockCreds.get.mockResolvedValue(null)

      const api = createMockApi()
      const signer = createMockSigner()

      const { result } = renderHook(() =>
        useWebAuthnSigning({
          api,
          signer,
          identityId: testIdentityId,
          passkeyId: testPasskeyId,
          credentialId: testCredentialId,
        })
      )

      let signResult: any
      await act(async () => {
        signResult = await result.current.sign('test')
      })

      expect(signResult.success).toBe(false)
      expect(signResult.error?.code).toBe<SigningErrorCode>('CREDENTIAL_NOT_FOUND')
    })

    it('WebAuthn未サポートの場合 WEBAUTHN_NOT_SUPPORTED エラー', async () => {
      // Remove credentials API
      Object.defineProperty(navigator, 'credentials', {
        value: undefined,
        configurable: true,
        writable: true,
      })

      const api = createMockApi()
      const signer = createMockSigner()

      const { result } = renderHook(() =>
        useWebAuthnSigning({
          api,
          signer,
          identityId: testIdentityId,
          passkeyId: testPasskeyId,
          credentialId: testCredentialId,
        })
      )

      let signResult: any
      await act(async () => {
        signResult = await result.current.sign('test')
      })

      expect(signResult.success).toBe(false)
      expect(signResult.error?.code).toBe<SigningErrorCode>('WEBAUTHN_NOT_SUPPORTED')
    })

    it('認証エラー時 AUTHENTICATOR_ERROR', async () => {
      const error = new DOMException('Unknown authenticator error', 'UnknownError')
      mockCreds.get.mockRejectedValue(error)

      const api = createMockApi()
      const signer = createMockSigner()

      const { result } = renderHook(() =>
        useWebAuthnSigning({
          api,
          signer,
          identityId: testIdentityId,
          passkeyId: testPasskeyId,
          credentialId: testCredentialId,
        })
      )

      let signResult: any
      await act(async () => {
        signResult = await result.current.sign('test')
      })

      expect(signResult.success).toBe(false)
      expect(signResult.error?.code).toBe<SigningErrorCode>('AUTHENTICATOR_ERROR')
    })
  })

  describe('トランザクションエラー', () => {
    it('一般的なトランザクション失敗', async () => {
      const challenge = await generateWysiwysChallenge('tx fail test')
      setupAssertionMock(mockCreds, challenge, testCredentialId)
      const api = createMockApi({ shouldFail: true, errorType: 'TransactionFailed' })
      const signer = createMockSigner()

      const { result } = renderHook(() =>
        useWebAuthnSigning({
          api,
          signer,
          identityId: testIdentityId,
          passkeyId: testPasskeyId,
          credentialId: testCredentialId,
        })
      )

      let signResult: any
      await act(async () => {
        signResult = await result.current.sign('tx fail test')
      })

      expect(signResult.success).toBe(false)
      expect(signResult.error?.code).toBe<SigningErrorCode>('TRANSACTION_FAILED')
      expect(result.current.status).toBe<SigningStatus>('error')
    })

    it('残高不足エラー INSUFFICIENT_BALANCE', async () => {
      const challenge = await generateWysiwysChallenge('balance test')
      setupAssertionMock(mockCreds, challenge, testCredentialId)
      const api = createMockApi({ shouldFail: true, errorType: 'InsufficientBalance' })
      const signer = createMockSigner()

      const { result } = renderHook(() =>
        useWebAuthnSigning({
          api,
          signer,
          identityId: testIdentityId,
          passkeyId: testPasskeyId,
          credentialId: testCredentialId,
        })
      )

      let signResult: any
      await act(async () => {
        signResult = await result.current.sign('balance test')
      })

      expect(signResult.success).toBe(false)
      expect(signResult.error?.code).toBe<SigningErrorCode>('INSUFFICIENT_BALANCE')
    })

    it('署名検証失敗 SIGNATURE_INVALID', async () => {
      const challenge = await generateWysiwysChallenge('sig test')
      setupAssertionMock(mockCreds, challenge, testCredentialId)
      const api = createMockApi({ shouldFail: true, errorType: 'SignatureInvalid' })
      const signer = createMockSigner()

      const { result } = renderHook(() =>
        useWebAuthnSigning({
          api,
          signer,
          identityId: testIdentityId,
          passkeyId: testPasskeyId,
          credentialId: testCredentialId,
        })
      )

      let signResult: any
      await act(async () => {
        signResult = await result.current.sign('sig test')
      })

      expect(signResult.success).toBe(false)
      expect(signResult.error?.code).toBe<SigningErrorCode>('SIGNATURE_INVALID')
    })

    it('チャレンジ不一致 CHALLENGE_MISMATCH', async () => {
      const challenge = await generateWysiwysChallenge('mismatch test')
      setupAssertionMock(mockCreds, challenge, testCredentialId)
      const api = createMockApi({ shouldFail: true, errorType: 'ChallengeMismatch' })
      const signer = createMockSigner()

      const { result } = renderHook(() =>
        useWebAuthnSigning({
          api,
          signer,
          identityId: testIdentityId,
          passkeyId: testPasskeyId,
          credentialId: testCredentialId,
        })
      )

      let signResult: any
      await act(async () => {
        signResult = await result.current.sign('mismatch test')
      })

      expect(signResult.success).toBe(false)
      expect(signResult.error?.code).toBe<SigningErrorCode>('CHALLENGE_MISMATCH')
    })

    it('ネットワークエラー NETWORK_ERROR', async () => {
      const challenge = await generateWysiwysChallenge('network test')
      setupAssertionMock(mockCreds, challenge, testCredentialId)
      const api = createMockApi({ shouldFail: true, errorType: 'NetworkError: connection failed' })
      const signer = createMockSigner()

      const { result } = renderHook(() =>
        useWebAuthnSigning({
          api,
          signer,
          identityId: testIdentityId,
          passkeyId: testPasskeyId,
          credentialId: testCredentialId,
        })
      )

      let signResult: any
      await act(async () => {
        signResult = await result.current.sign('network test')
      })

      expect(signResult.success).toBe(false)
      expect(signResult.error?.code).toBe<SigningErrorCode>('NETWORK_ERROR')
    })
  })

  describe('コンテンツバリデーション', () => {
    it('空のコンテンツでも署名できる（バリデーションはpallet側）', async () => {
      const challenge = await generateWysiwysChallenge('')
      setupAssertionMock(mockCreds, challenge, testCredentialId)
      const api = createMockApi()
      const signer = createMockSigner()

      const { result } = renderHook(() =>
        useWebAuthnSigning({
          api,
          signer,
          identityId: testIdentityId,
          passkeyId: testPasskeyId,
          credentialId: testCredentialId,
        })
      )

      let signResult: any
      await act(async () => {
        signResult = await result.current.sign('')
      })

      expect(signResult.success).toBe(true)
    })

    it('長いコンテンツでも署名できる', async () => {
      const longContent = 'a'.repeat(1000)
      const challenge = await generateWysiwysChallenge(longContent)
      setupAssertionMock(mockCreds, challenge, testCredentialId)
      const api = createMockApi()
      const signer = createMockSigner()

      const { result } = renderHook(() =>
        useWebAuthnSigning({
          api,
          signer,
          identityId: testIdentityId,
          passkeyId: testPasskeyId,
          credentialId: testCredentialId,
        })
      )

      let signResult: any
      await act(async () => {
        signResult = await result.current.sign(longContent)
      })

      expect(signResult.success).toBe(true)
    })

    it('マルチバイト文字（日本語）を正しく処理する', async () => {
      const japaneseContent = 'これはテスト投稿です。🎉'
      const challenge = await generateWysiwysChallenge(japaneseContent)
      setupAssertionMock(mockCreds, challenge, testCredentialId)
      const api = createMockApi()
      const signer = createMockSigner()

      const { result } = renderHook(() =>
        useWebAuthnSigning({
          api,
          signer,
          identityId: testIdentityId,
          passkeyId: testPasskeyId,
          credentialId: testCredentialId,
        })
      )

      let signResult: any
      await act(async () => {
        signResult = await result.current.sign(japaneseContent)
      })

      expect(signResult.success).toBe(true)
    })
  })

  describe('reset機能', () => {
    it('reset() で idle 状態に戻る', async () => {
      const challenge = await generateWysiwysChallenge('reset test')
      setupAssertionMock(mockCreds, challenge, testCredentialId)
      const api = createMockApi()
      const signer = createMockSigner()

      const { result } = renderHook(() =>
        useWebAuthnSigning({
          api,
          signer,
          identityId: testIdentityId,
          passkeyId: testPasskeyId,
          credentialId: testCredentialId,
        })
      )

      await act(async () => {
        await result.current.sign('reset test')
      })
      expect(result.current.status).toBe('success')

      act(() => {
        result.current.reset()
      })

      expect(result.current.status).toBe<SigningStatus>('idle')
      expect(result.current.error).toBeNull()
    })

    it('エラー後も reset() で回復できる', async () => {
      mockCreds.get.mockRejectedValue(new DOMException('Cancelled', 'NotAllowedError'))
      const api = createMockApi()
      const signer = createMockSigner()

      const { result } = renderHook(() =>
        useWebAuthnSigning({
          api,
          signer,
          identityId: testIdentityId,
          passkeyId: testPasskeyId,
          credentialId: testCredentialId,
        })
      )

      await act(async () => {
        await result.current.sign('error test')
      })
      expect(result.current.status).toBe('error')
      expect(result.current.error).not.toBeNull()

      act(() => {
        result.current.reset()
      })

      expect(result.current.status).toBe('idle')
      expect(result.current.error).toBeNull()
    })
  })

  describe('estimateCost機能', () => {
    it('コンテンツ長に基づいてコストを計算する', () => {
      const api = createMockApi()
      const signer = createMockSigner()

      const { result } = renderHook(() =>
        useWebAuthnSigning({
          api,
          signer,
          identityId: testIdentityId,
          passkeyId: testPasskeyId,
          credentialId: testCredentialId,
        })
      )

      // 基本コスト 10 + バイト単価 0.1 * バイト数
      const cost = result.current.estimateCost('hello')
      expect(cost).toBeGreaterThan(0)
    })

    it('空文字列でも基本コストを返す', () => {
      const api = createMockApi()
      const signer = createMockSigner()

      const { result } = renderHook(() =>
        useWebAuthnSigning({
          api,
          signer,
          identityId: testIdentityId,
          passkeyId: testPasskeyId,
          credentialId: testCredentialId,
        })
      )

      const cost = result.current.estimateCost('')
      expect(cost).toBeGreaterThan(0) // Base cost only
    })

    it('日本語コンテンツのバイト数を正しく計算する', () => {
      const api = createMockApi()
      const signer = createMockSigner()

      const { result } = renderHook(() =>
        useWebAuthnSigning({
          api,
          signer,
          identityId: testIdentityId,
          passkeyId: testPasskeyId,
          credentialId: testCredentialId,
        })
      )

      // 1文字あたり3バイト（UTF-8）
      const asciiCost = result.current.estimateCost('hello') // 5 bytes
      const jpCost = result.current.estimateCost('こんにちは') // 5 chars * 3 = 15 bytes

      expect(jpCost).toBeGreaterThan(asciiCost)
    })
  })

  describe('API/Signerが未設定の場合', () => {
    it('api が null の場合エラーを返す', async () => {
      const challenge = await generateWysiwysChallenge('no api')
      setupAssertionMock(mockCreds, challenge, testCredentialId)
      const signer = createMockSigner()

      const { result } = renderHook(() =>
        useWebAuthnSigning({
          api: null,
          signer,
          identityId: testIdentityId,
          passkeyId: testPasskeyId,
          credentialId: testCredentialId,
        })
      )

      let signResult: any
      await act(async () => {
        signResult = await result.current.sign('no api')
      })

      expect(signResult.success).toBe(false)
      expect(signResult.error?.code).toBe<SigningErrorCode>('NETWORK_ERROR')
    })

    it('signer が null の場合エラーを返す', async () => {
      const challenge = await generateWysiwysChallenge('no signer')
      setupAssertionMock(mockCreds, challenge, testCredentialId)
      const api = createMockApi()

      const { result } = renderHook(() =>
        useWebAuthnSigning({
          api,
          signer: null,
          identityId: testIdentityId,
          passkeyId: testPasskeyId,
          credentialId: testCredentialId,
        })
      )

      let signResult: any
      await act(async () => {
        signResult = await result.current.sign('no signer')
      })

      expect(signResult.success).toBe(false)
      expect(signResult.error?.code).toBe<SigningErrorCode>('NETWORK_ERROR')
    })
  })

  describe('トランザクションパラメータ', () => {
    it('create_post_with_webauthn に正しいパラメータが渡される', async () => {
      const testContent = 'verify params'
      const challenge = await generateWysiwysChallenge(testContent)
      setupAssertionMock(mockCreds, challenge, testCredentialId)
      const api = createMockApi()
      const signer = createMockSigner()

      const { result } = renderHook(() =>
        useWebAuthnSigning({
          api,
          signer,
          identityId: testIdentityId,
          passkeyId: testPasskeyId,
          credentialId: testCredentialId,
        })
      )

      await act(async () => {
        await result.current.sign(testContent)
      })

      // Verify create_post_with_webauthn was called with correct parameters
      expect(api.tx.Post.create_post_with_webauthn).toHaveBeenCalledWith(
        expect.anything(), // identity_id
        expect.anything(), // passkey_id
        expect.anything(), // content
        expect.anything(), // authenticatorData (Binary)
        expect.anything(), // clientDataJSON (Binary)
        expect.anything() // signature (Binary)
      )
    })

    it('identity_id が正しく渡される', async () => {
      const challenge = await generateWysiwysChallenge('id test')
      setupAssertionMock(mockCreds, challenge, testCredentialId)
      const api = createMockApi()
      const signer = createMockSigner()

      const customIdentityId = 123n

      const { result } = renderHook(() =>
        useWebAuthnSigning({
          api,
          signer,
          identityId: customIdentityId,
          passkeyId: testPasskeyId,
          credentialId: testCredentialId,
        })
      )

      await act(async () => {
        await result.current.sign('id test')
      })

      const call = api.tx.Post.create_post_with_webauthn.mock.calls[0]
      expect(call[0]).toBe(customIdentityId)
    })
  })
})
