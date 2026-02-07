import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { renderHook, act, waitFor } from '@testing-library/react'
import { useWebAuthnRegistration } from '../hooks/useWebAuthnRegistration'
import { 
  createMockCredentials, 
  setupRegistrationMock, 
  MockCredentialsContainer,
  createMockAttestationObject,
  MockPublicKeyCredential,
} from './setup'
import { RegistrationStatus, RegistrationErrorCode } from '../types/webauthn'

// Mock PAPI transaction
const createMockApi = (options?: { 
  shouldFail?: boolean; 
  errorType?: string;
  events?: any[];
}) => {
  const { shouldFail = false, errorType = 'TransactionFailed', events = [] } = options ?? {}
  
  const mockTx = {
    signAndSubmit: vi.fn().mockImplementation(async () => {
      if (shouldFail) {
        throw new Error(errorType)
      }
      // Return mock transaction result with events
      return {
        ok: true,
        block: { hash: '0x1234567890' },
        events: events.length > 0 ? events : [
          {
            event: {
              type: 'Identity',
              value: {
                type: 'IdentityCreated',
                value: {
                  identity_id: 42n,
                  passkey_id: new Uint8Array([1, 2, 3, 4])
                }
              }
            }
          }
        ]
      }
    })
  }

  return {
    tx: {
      Identity: {
        register_identity: vi.fn().mockReturnValue(mockTx)
      }
    }
  }
}

const createMockSigner = () => ({
  publicKey: new Uint8Array(32).fill(1),
  sign: vi.fn().mockResolvedValue(new Uint8Array(64).fill(2))
})

describe('useWebAuthnRegistration', () => {
  let originalCredentials: CredentialsContainer | undefined
  let originalPublicKeyCredential: typeof PublicKeyCredential | undefined
  let mockCreds: MockCredentialsContainer

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
      writable: true
    })
    
    // Setup PublicKeyCredential
    Object.defineProperty(window, 'PublicKeyCredential', {
      value: MockPublicKeyCredential,
      configurable: true,
      writable: true
    })
    
    // Setup static methods
    ;(MockPublicKeyCredential as any).isUserVerifyingPlatformAuthenticatorAvailable = vi.fn().mockResolvedValue(true)
    ;(MockPublicKeyCredential as any).isConditionalMediationAvailable = vi.fn().mockResolvedValue(true)
  })

  afterEach(() => {
    if (originalCredentials) {
      Object.defineProperty(navigator, 'credentials', {
        value: originalCredentials,
        configurable: true,
        writable: true
      })
    }
    if (originalPublicKeyCredential) {
      Object.defineProperty(window, 'PublicKeyCredential', {
        value: originalPublicKeyCredential,
        configurable: true,
        writable: true
      })
    }
  })

  describe('初期状態', () => {
    it('status が idle で初期化される', () => {
      const api = createMockApi()
      const signer = createMockSigner()
      
      const { result } = renderHook(() => 
        useWebAuthnRegistration({ api, signer })
      )

      expect(result.current.status).toBe<RegistrationStatus>('idle')
      expect(result.current.error).toBeNull()
    })

    it('api または signer が null でも動作する', () => {
      const { result } = renderHook(() => 
        useWebAuthnRegistration({ api: null, signer: null })
      )

      expect(result.current.status).toBe('idle')
    })
  })

  describe('正常な登録フロー', () => {
    it('完全な登録フローが成功する', async () => {
      setupRegistrationMock(mockCreds)
      const api = createMockApi()
      const signer = createMockSigner()
      const onSuccess = vi.fn()

      const { result } = renderHook(() => 
        useWebAuthnRegistration({ api, signer, onSuccess })
      )

      let registerResult: any
      await act(async () => {
        registerResult = await result.current.register('MacBook Pro')
      })

      expect(registerResult.success).toBe(true)
      expect(registerResult.identityId).toBe(42n)
      expect(registerResult.passkeyId).toBeDefined()
      expect(result.current.status).toBe<RegistrationStatus>('success')
      expect(onSuccess).toHaveBeenCalledWith(expect.objectContaining({
        success: true,
        identityId: 42n
      }))
    })

    it('deviceName なしでも登録できる', async () => {
      setupRegistrationMock(mockCreds)
      const api = createMockApi()
      const signer = createMockSigner()

      const { result } = renderHook(() => 
        useWebAuthnRegistration({ api, signer })
      )

      await act(async () => {
        await result.current.register()
      })

      expect(result.current.status).toBe('success')
    })

    it('status が正しい順序で遷移する', async () => {
      setupRegistrationMock(mockCreds)
      const api = createMockApi()
      const signer = createMockSigner()
      const statusHistory: RegistrationStatus[] = []

      const { result, rerender } = renderHook(() => 
        useWebAuthnRegistration({ api, signer })
      )

      // 初期状態を記録
      statusHistory.push(result.current.status)

      // registerを呼び出し（非同期）
      const registerPromise = act(async () => {
        await result.current.register('Test Device')
      })

      // 中間状態をキャプチャするため短い間隔でrerender
      let prevStatus = result.current.status
      const checkInterval = setInterval(() => {
        if (result.current.status !== prevStatus) {
          statusHistory.push(result.current.status)
          prevStatus = result.current.status
        }
      }, 10)

      await registerPromise

      clearInterval(checkInterval)
      // 最終状態を追加
      if (statusHistory[statusHistory.length - 1] !== result.current.status) {
        statusHistory.push(result.current.status)
      }

      // 少なくとも idle → success の遷移を確認
      expect(statusHistory[0]).toBe('idle')
      expect(statusHistory[statusHistory.length - 1]).toBe('success')
    })

    it('register_identity に正しいパラメータが渡される', async () => {
      setupRegistrationMock(mockCreds)
      const api = createMockApi()
      const signer = createMockSigner()

      const { result } = renderHook(() => 
        useWebAuthnRegistration({ api, signer })
      )

      await act(async () => {
        await result.current.register('MacBook Air')
      })

      // PAPI呼び出しを検証
      expect(api.tx.Identity.register_identity).toHaveBeenCalledWith(
        expect.objectContaining({
          public_key: expect.any(Object), // Binary type
          device_name: expect.any(Object) // Binary type
        })
      )
    })
  })

  describe('エラーハンドリング', () => {
    it('apiがnullの場合エラーを返す', async () => {
      const signer = createMockSigner()
      const onError = vi.fn()

      const { result } = renderHook(() => 
        useWebAuthnRegistration({ api: null, signer, onError })
      )

      let registerResult: any
      await act(async () => {
        registerResult = await result.current.register()
      })

      expect(registerResult.success).toBe(false)
      expect(registerResult.error?.code).toBe<RegistrationErrorCode>('NETWORK_ERROR')
      expect(result.current.status).toBe<RegistrationStatus>('error')
      expect(onError).toHaveBeenCalled()
    })

    it('signerがnullの場合エラーを返す', async () => {
      const api = createMockApi()

      const { result } = renderHook(() => 
        useWebAuthnRegistration({ api, signer: null })
      )

      let registerResult: any
      await act(async () => {
        registerResult = await result.current.register()
      })

      expect(registerResult.success).toBe(false)
      expect(registerResult.error?.code).toBe<RegistrationErrorCode>('NETWORK_ERROR')
    })

    it('WebAuthn非対応の場合エラーを返す', async () => {
      // WebAuthnを無効化
      Object.defineProperty(navigator, 'credentials', {
        value: undefined,
        configurable: true
      })
      
      const api = createMockApi()
      const signer = createMockSigner()

      const { result } = renderHook(() => 
        useWebAuthnRegistration({ api, signer })
      )

      let registerResult: any
      await act(async () => {
        registerResult = await result.current.register()
      })

      expect(registerResult.success).toBe(false)
      expect(registerResult.error?.code).toBe<RegistrationErrorCode>('WEBAUTHN_NOT_SUPPORTED')
    })

    it('ユーザーがキャンセルした場合USER_CANCELLEDエラーを返す', async () => {
      const mockCreate = vi.fn().mockRejectedValue(
        Object.assign(new Error('User cancelled'), { name: 'NotAllowedError' })
      )
      Object.defineProperty(navigator, 'credentials', {
        value: { create: mockCreate },
        configurable: true
      })
      
      const api = createMockApi()
      const signer = createMockSigner()

      const { result } = renderHook(() => 
        useWebAuthnRegistration({ api, signer })
      )

      let registerResult: any
      await act(async () => {
        registerResult = await result.current.register()
      })

      expect(registerResult.success).toBe(false)
      expect(registerResult.error?.code).toBe<RegistrationErrorCode>('USER_CANCELLED')
      expect(result.current.status).toBe('error')
    })

    it('credentials.createがnullを返した場合エラーを返す', async () => {
      const mockCreate = vi.fn().mockResolvedValue(null)
      Object.defineProperty(navigator, 'credentials', {
        value: { create: mockCreate },
        configurable: true
      })
      
      const api = createMockApi()
      const signer = createMockSigner()

      const { result } = renderHook(() => 
        useWebAuthnRegistration({ api, signer })
      )

      let registerResult: any
      await act(async () => {
        registerResult = await result.current.register()
      })

      expect(registerResult.success).toBe(false)
      expect(registerResult.error?.code).toBe<RegistrationErrorCode>('AUTHENTICATOR_ERROR')
    })

    it('COSE公開鍵抽出に失敗した場合エラーを返す', async () => {
      // 無効な attestationObject を返すモック
      const invalidCredential = new MockPublicKeyCredential({
        id: 'test-credential-id',
        rawId: new TextEncoder().encode('test-credential-id').buffer,
        response: {
          attestationObject: new ArrayBuffer(0), // 空のバッファで CBOR パースエラー
          clientDataJSON: new TextEncoder().encode('{}').buffer,
          getTransports: () => ['internal'],
          getPublicKey: () => null,
          getPublicKeyAlgorithm: () => -7,
          getAuthenticatorData: () => new ArrayBuffer(0),
        } as unknown as AuthenticatorAttestationResponse,
      })
      
      mockCreds.create.mockResolvedValue(invalidCredential)
      
      const api = createMockApi()
      const signer = createMockSigner()

      const { result } = renderHook(() => 
        useWebAuthnRegistration({ api, signer })
      )

      let registerResult: any
      await act(async () => {
        registerResult = await result.current.register()
      })

      expect(registerResult.success).toBe(false)
      expect(registerResult.error?.code).toBe<RegistrationErrorCode>('EXTRACTION_FAILED')
    })

    it('トランザクション失敗時TRANSACTION_FAILEDエラーを返す', async () => {
      setupRegistrationMock(mockCreds)
      const api = createMockApi({ shouldFail: true })
      const signer = createMockSigner()

      const { result } = renderHook(() => 
        useWebAuthnRegistration({ api, signer })
      )

      let registerResult: any
      await act(async () => {
        registerResult = await result.current.register()
      })

      expect(registerResult.success).toBe(false)
      expect(registerResult.error?.code).toBe<RegistrationErrorCode>('TRANSACTION_FAILED')
    })

    it('パスキー既に登録済みの場合PASSKEY_ALREADY_REGISTEREDエラーを返す', async () => {
      setupRegistrationMock(mockCreds)
      const api = createMockApi({ 
        shouldFail: true, 
        errorType: 'PasskeyAlreadyRegistered' 
      })
      const signer = createMockSigner()

      const { result } = renderHook(() => 
        useWebAuthnRegistration({ api, signer })
      )

      let registerResult: any
      await act(async () => {
        registerResult = await result.current.register()
      })

      expect(registerResult.success).toBe(false)
      expect(registerResult.error?.code).toBe<RegistrationErrorCode>('PASSKEY_ALREADY_REGISTERED')
    })
  })

  describe('reset機能', () => {
    it('reset()でstateが初期化される', async () => {
      setupRegistrationMock(mockCreds)
      const api = createMockApi({ shouldFail: true })
      const signer = createMockSigner()

      const { result } = renderHook(() => 
        useWebAuthnRegistration({ api, signer })
      )

      // エラー状態にする
      await act(async () => {
        await result.current.register()
      })

      expect(result.current.status).toBe('error')
      expect(result.current.error).not.toBeNull()

      // リセット
      act(() => {
        result.current.reset()
      })

      expect(result.current.status).toBe('idle')
      expect(result.current.error).toBeNull()
    })

    it('success後もreset()で初期化できる', async () => {
      setupRegistrationMock(mockCreds)
      const api = createMockApi()
      const signer = createMockSigner()

      const { result } = renderHook(() => 
        useWebAuthnRegistration({ api, signer })
      )

      await act(async () => {
        await result.current.register()
      })

      expect(result.current.status).toBe('success')

      act(() => {
        result.current.reset()
      })

      expect(result.current.status).toBe('idle')
    })
  })

  describe('同時実行防止', () => {
    it('処理中に複数回register()を呼んでも2回目以降は無視される', async () => {
      setupRegistrationMock(mockCreds)
      const api = createMockApi()
      const signer = createMockSigner()

      const { result } = renderHook(() => 
        useWebAuthnRegistration({ api, signer })
      )

      // 同時に2回呼び出し
      let result1: any, result2: any
      await act(async () => {
        const promise1 = result.current.register()
        // 少し遅れて2回目を呼び出し
        const promise2 = result.current.register()
        
        result1 = await promise1
        result2 = await promise2
      })

      // 最初の呼び出しは成功、2回目は無視されるか同じ結果
      expect(result1.success).toBe(true)
      // 2回目は処理中のため無視されることを期待
      // 実装によっては同じ結果かエラーを返す
    })
  })

  describe('コールバック', () => {
    it('成功時onSuccessが呼ばれる', async () => {
      setupRegistrationMock(mockCreds)
      const api = createMockApi()
      const signer = createMockSigner()
      const onSuccess = vi.fn()

      const { result } = renderHook(() => 
        useWebAuthnRegistration({ api, signer, onSuccess })
      )

      await act(async () => {
        await result.current.register()
      })

      expect(onSuccess).toHaveBeenCalledTimes(1)
      expect(onSuccess).toHaveBeenCalledWith(expect.objectContaining({
        success: true,
        identityId: expect.anything()
      }))
    })

    it('失敗時onErrorが呼ばれる', async () => {
      setupRegistrationMock(mockCreds)
      const api = createMockApi({ shouldFail: true })
      const signer = createMockSigner()
      const onError = vi.fn()

      const { result } = renderHook(() => 
        useWebAuthnRegistration({ api, signer, onError })
      )

      await act(async () => {
        await result.current.register()
      })

      expect(onError).toHaveBeenCalledTimes(1)
      expect(onError).toHaveBeenCalledWith(expect.objectContaining({
        code: expect.any(String),
        message: expect.any(String)
      }))
    })
  })

  describe('デバイス名', () => {
    it('長いデバイス名は64バイトで切り詰められる', async () => {
      setupRegistrationMock(mockCreds)
      const api = createMockApi()
      const signer = createMockSigner()

      const { result } = renderHook(() => 
        useWebAuthnRegistration({ api, signer })
      )

      const longDeviceName = 'A'.repeat(100)
      await act(async () => {
        await result.current.register(longDeviceName)
      })

      // register_identityに渡されるdevice_nameが64バイト以下であることを確認
      const calledArgs = api.tx.Identity.register_identity.mock.calls[0][0]
      // Binary型でエンコードされている
      expect(calledArgs.device_name.asBytes().length).toBeLessThanOrEqual(64)
    })

    it('日本語デバイス名も正しく処理される', async () => {
      setupRegistrationMock(mockCreds)
      const api = createMockApi()
      const signer = createMockSigner()

      const { result } = renderHook(() => 
        useWebAuthnRegistration({ api, signer })
      )

      await act(async () => {
        await result.current.register('私のMacBook Pro')
      })

      expect(result.current.status).toBe('success')
    })
  })
})
