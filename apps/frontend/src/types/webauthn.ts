/**
 * WebAuthn Types for Anarchy Frontend
 */

// ============================================
// Registration Types
// ============================================

export type RegistrationStatus =
  | 'idle'
  | 'authenticating'
  | 'extracting'
  | 'submitting'
  | 'confirming'
  | 'success'
  | 'error'

export type RegistrationErrorCode =
  | 'WEBAUTHN_NOT_SUPPORTED'
  | 'USER_CANCELLED'
  | 'AUTHENTICATOR_ERROR'
  | 'EXTRACTION_FAILED'
  | 'TRANSACTION_FAILED'
  | 'PASSKEY_ALREADY_REGISTERED'
  | 'NETWORK_ERROR'

export interface RegistrationError {
  code: RegistrationErrorCode
  message: string
  originalError?: unknown
}

export interface RegisterResult {
  success: boolean
  identityId?: bigint
  passkeyId?: Uint8Array
  error?: RegistrationError
}

// ============================================
// Signing Types
// ============================================

export type SigningStatus =
  | 'idle'
  | 'hashing'
  | 'authenticating'
  | 'submitting'
  | 'confirming'
  | 'success'
  | 'error'

export type SigningErrorCode =
  | 'WEBAUTHN_NOT_SUPPORTED'
  | 'USER_CANCELLED'
  | 'AUTHENTICATOR_ERROR'
  | 'CREDENTIAL_NOT_FOUND'
  | 'TRANSACTION_FAILED'
  | 'SIGNATURE_INVALID'
  | 'CHALLENGE_MISMATCH'
  | 'INSUFFICIENT_BALANCE'
  | 'CONTENT_TOO_LONG'
  | 'NETWORK_ERROR'

export interface SigningError {
  code: SigningErrorCode
  message: string
  originalError?: unknown
}

export interface PostResult {
  success: boolean
  postId?: bigint
  txHash?: string
  moralSpent?: bigint
  error?: SigningError
}

// ============================================
// Identity Types
// ============================================

export interface PasskeyInfo {
  passkeyId: Uint8Array
  deviceName?: string
  registeredAt?: Date
}

export interface IdentityState {
  identityId: bigint
  passkeys: PasskeyInfo[]
  createdAt?: Date
}

// ============================================
// useWebAuthn Integrated Hook Types
// ============================================

export type WebAuthnErrorCode =
  | RegistrationErrorCode
  | SigningErrorCode
  | 'NO_IDENTITY'
  | 'API_NOT_AVAILABLE'
  | 'SIGNER_NOT_AVAILABLE'
  | 'IDENTITY_NOT_FOUND'
  | 'TOO_MANY_PASSKEYS'

export interface WebAuthnError {
  code: WebAuthnErrorCode
  message: string
  originalError?: unknown
}

export interface CurrentIdentity {
  identityId: bigint
  passkeyId: Uint8Array
  credentialId: string
  deviceName?: string
}

export interface AddPasskeyResult {
  success: boolean
  passkeyId?: Uint8Array
  error?: WebAuthnError
}

export interface UseWebAuthnOptions {
  api: any | null
  signer: any | null
  initialIdentity?: CurrentIdentity
  onRegistrationSuccess?: (result: RegisterResult) => void
  onPostSuccess?: (result: PostResult) => void
  onError?: (error: WebAuthnError) => void
}

export const WEBAUTHN_ERROR_MESSAGES: Record<WebAuthnErrorCode, string> = {
  // Registration errors
  WEBAUTHN_NOT_SUPPORTED: 'このブラウザはパスキーに対応していません',
  USER_CANCELLED: '操作がキャンセルされました',
  AUTHENTICATOR_ERROR: '認証に失敗しました',
  EXTRACTION_FAILED: '公開鍵の取得に失敗しました',
  TRANSACTION_FAILED: 'トランザクションが失敗しました',
  PASSKEY_ALREADY_REGISTERED: 'このパスキーは既に登録されています',
  NETWORK_ERROR: 'ネットワークエラーが発生しました',
  // Signing errors
  CREDENTIAL_NOT_FOUND: 'パスキーが見つかりません',
  SIGNATURE_INVALID: '署名の検証に失敗しました',
  CHALLENGE_MISMATCH: '署名内容が一致しません',
  INSUFFICIENT_BALANCE: '$moral残高が不足しています',
  CONTENT_TOO_LONG: '投稿内容が長すぎます',
  // useWebAuthn specific errors
  NO_IDENTITY: 'Identityが設定されていません',
  API_NOT_AVAILABLE: 'APIが利用できません',
  SIGNER_NOT_AVAILABLE: '署名者が利用できません',
  IDENTITY_NOT_FOUND: '指定されたIdentityが見つかりません',
  TOO_MANY_PASSKEYS: 'パスキー数が上限に達しています',
}

// ============================================
// Hook Options
// ============================================

export interface UseWebAuthnRegistrationOptions {
  api: any | null
  signer: any | null
  onSuccess?: (result: RegisterResult) => void
  onError?: (error: RegistrationError) => void
}

export interface UseWebAuthnSigningOptions {
  api: any | null
  signer: any | null
  identityId: bigint
  passkeyId: Uint8Array
  credentialId: string
  onSuccess?: (result: PostResult) => void
  onError?: (error: SigningError) => void
}

// ============================================
// WebAuthn Event Types (for analytics)
// ============================================

export type WebAuthnEvent =
  | { type: 'registration_started'; deviceName?: string }
  | { type: 'registration_authenticating' }
  | { type: 'registration_submitting'; txHash: string }
  | { type: 'registration_success'; identityId: bigint; passkeyId: string }
  | { type: 'registration_error'; code: RegistrationErrorCode }
  | { type: 'signing_started'; contentLength: number }
  | { type: 'signing_authenticating' }
  | { type: 'signing_submitting'; txHash: string }
  | { type: 'signing_success'; postId: bigint; moralSpent: string }
  | { type: 'signing_error'; code: SigningErrorCode }

// ============================================
// Error Messages (Japanese)
// ============================================

export const REGISTRATION_ERROR_MESSAGES: Record<RegistrationErrorCode, string> = {
  WEBAUTHN_NOT_SUPPORTED: 'このブラウザはパスキーに対応していません',
  USER_CANCELLED: '操作がキャンセルされました',
  AUTHENTICATOR_ERROR: '認証に失敗しました',
  EXTRACTION_FAILED: '公開鍵の取得に失敗しました',
  TRANSACTION_FAILED: 'トランザクションが失敗しました',
  PASSKEY_ALREADY_REGISTERED: 'このパスキーは既に登録されています',
  NETWORK_ERROR: 'ネットワークエラーが発生しました',
}

export const SIGNING_ERROR_MESSAGES: Record<SigningErrorCode, string> = {
  WEBAUTHN_NOT_SUPPORTED: 'このブラウザはパスキーに対応していません',
  USER_CANCELLED: '操作がキャンセルされました',
  AUTHENTICATOR_ERROR: '認証に失敗しました',
  CREDENTIAL_NOT_FOUND: 'パスキーが見つかりません',
  TRANSACTION_FAILED: 'トランザクションが失敗しました',
  SIGNATURE_INVALID: '署名の検証に失敗しました',
  CHALLENGE_MISMATCH: '署名内容が一致しません',
  INSUFFICIENT_BALANCE: '$moral残高が不足しています',
  CONTENT_TOO_LONG: '投稿内容が長すぎます',
  NETWORK_ERROR: 'ネットワークエラーが発生しました',
}

// ============================================
// Utility Type Guards
// ============================================

export function isRegistrationError(error: unknown): error is RegistrationError {
  return (
    typeof error === 'object' &&
    error !== null &&
    'code' in error &&
    'message' in error &&
    typeof (error as RegistrationError).code === 'string' &&
    typeof (error as RegistrationError).message === 'string'
  )
}

export function isSigningError(error: unknown): error is SigningError {
  return (
    typeof error === 'object' &&
    error !== null &&
    'code' in error &&
    'message' in error &&
    typeof (error as SigningError).code === 'string' &&
    typeof (error as SigningError).message === 'string'
  )
}
