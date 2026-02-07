/// <reference types="vitest/globals" />
import '@testing-library/jest-dom';

/**
 * WebAuthn API Mock
 * Mocks navigator.credentials for testing WebAuthn flows
 */

// Mock PublicKeyCredential
export class MockPublicKeyCredential implements PublicKeyCredential {
  readonly id: string;
  readonly rawId: ArrayBuffer;
  readonly type: 'public-key' = 'public-key';
  readonly authenticatorAttachment: AuthenticatorAttachment | null = 'platform';
  readonly response: AuthenticatorResponse;

  constructor(options: {
    id: string;
    rawId: ArrayBuffer;
    response: AuthenticatorResponse;
  }) {
    this.id = options.id;
    this.rawId = options.rawId;
    this.response = options.response;
  }

  // Extensions typically return empty object in tests
  getClientExtensionResults(): AuthenticationExtensionsClientOutputs {
    return {};
  }

  toJSON(): PublicKeyCredentialJSON {
    return {
      id: this.id,
      rawId: this.id,
      type: this.type,
      clientExtensionResults: {},
      authenticatorAttachment: this.authenticatorAttachment,
    } as unknown as PublicKeyCredentialJSON;
  }
}

// Mock attestation object with COSE public key
// This is a valid CBOR-encoded attestation object structure
export function createMockAttestationObject(): ArrayBuffer {
  // Minimal attestation object with authData containing COSE key
  // Format: fmt + attStmt + authData
  // authData: rpIdHash (32) + flags (1) + signCount (4) + attestedCredentialData
  // attestedCredentialData: aaguid (16) + credIdLen (2) + credId + credentialPublicKey (COSE)
  
  // This is a pre-computed valid attestation object for testing
  // COSE key format: EC2 key with P-256 curve (-7 / ES256)
  const attestationObjectHex = 
    'a363666d74646e6f6e6567617474537' +
    '4d74a068617574684461746158a4c9b' +
    '10454fca01c5f0f0f0f0f0a0a0a0a0a' +
    '0a0a0a0a0a0a0a0a0a0a0a0a0a04100' +
    '0001002000' +
    'f7e6e8e6e8e6e8e6e8e6e8e6e8e6e8e6e8e6e8e6e8e6e8e6e8e6e8e6e8e6e8e6' +
    'a501020326200121582065eda5a12577c2bad574' +
    '54c99a8f68a56da2e2eb0cbb4bfa6b9e3b7cdbaa' +
    '2258207f7e97e2aa8e9e8e8e9a8e8e8e8e8e8e' +
    '8e8e8e8e8e8e8e8e8e8e8e8e8e8e';
  
  // For simplicity, return a manually constructed valid attestation object
  const encoder = new TextEncoder();
  
  // Create a minimal valid COSE key (EC2, P-256)
  // COSE_Key = {1: 2, 3: -7, -1: 1, -2: x, -3: y}
  // Where: kty=2 (EC2), alg=-7 (ES256), crv=1 (P-256), x and y are 32-byte coords
  const coseKey = new Uint8Array([
    // CBOR map with 5 elements (0xa5)
    0xa5,
    // 1: 2 (kty: EC2)
    0x01, 0x02,
    // 3: -7 (alg: ES256)
    0x03, 0x26,
    // -1: 1 (crv: P-256)
    0x20, 0x01,
    // -2: bstr(32) x-coordinate
    0x21, 0x58, 0x20,
    ...new Array(32).fill(0x11),
    // -3: bstr(32) y-coordinate
    0x22, 0x58, 0x20,
    ...new Array(32).fill(0x22),
  ]);
  
  // Create authData
  // rpIdHash (32 bytes) + flags (1 byte) + signCount (4 bytes) + attestedCredData
  const rpIdHash = new Uint8Array(32).fill(0xc9);
  const flags = new Uint8Array([0x45]); // AT flag set (attested credential data present)
  const signCount = new Uint8Array([0x00, 0x00, 0x00, 0x01]);
  const aaguid = new Uint8Array(16).fill(0xaa);
  const credIdLength = new Uint8Array([0x00, 0x20]); // 32 bytes
  const credId = new Uint8Array(32).fill(0xcc);
  
  const authData = new Uint8Array([
    ...rpIdHash,
    ...flags,
    ...signCount,
    ...aaguid,
    ...credIdLength,
    ...credId,
    ...coseKey,
  ]);
  
  // Create attestation object CBOR manually
  // { "fmt": "none", "attStmt": {}, "authData": bstr }
  const fmtKey = new Uint8Array([0x63, 0x66, 0x6d, 0x74]); // "fmt" as CBOR text
  const fmtValue = new Uint8Array([0x64, 0x6e, 0x6f, 0x6e, 0x65]); // "none" as CBOR text
  const attStmtKey = new Uint8Array([0x67, 0x61, 0x74, 0x74, 0x53, 0x74, 0x6d, 0x74]); // "attStmt"
  const attStmtValue = new Uint8Array([0xa0]); // empty map
  const authDataKey = new Uint8Array([0x68, 0x61, 0x75, 0x74, 0x68, 0x44, 0x61, 0x74, 0x61]); // "authData"
  
  // authData as byte string with length prefix
  const authDataLen = authData.length;
  let authDataPrefix: Uint8Array;
  if (authDataLen <= 23) {
    authDataPrefix = new Uint8Array([0x40 + authDataLen]);
  } else if (authDataLen <= 255) {
    authDataPrefix = new Uint8Array([0x58, authDataLen]);
  } else {
    authDataPrefix = new Uint8Array([0x59, (authDataLen >> 8) & 0xff, authDataLen & 0xff]);
  }
  
  // Map with 3 elements
  const mapPrefix = new Uint8Array([0xa3]);
  
  const attestationObject = new Uint8Array([
    ...mapPrefix,
    ...fmtKey,
    ...fmtValue,
    ...attStmtKey,
    ...attStmtValue,
    ...authDataKey,
    ...authDataPrefix,
    ...authData,
  ]);
  
  return attestationObject.buffer;
}

// Mock client data JSON
export function createMockClientDataJSON(challenge: string): ArrayBuffer {
  const clientData = {
    type: 'webauthn.create',
    challenge: challenge,
    origin: 'http://localhost:3000',
    crossOrigin: false,
  };
  const encoder = new TextEncoder();
  return encoder.encode(JSON.stringify(clientData)).buffer;
}

// Mock assertion response
export function createMockAssertionResponse(challenge: Uint8Array): {
  authenticatorData: ArrayBuffer;
  clientDataJSON: ArrayBuffer;
  signature: ArrayBuffer;
  userHandle: ArrayBuffer | null;
} {
  // Minimal authenticator data for assertion
  const rpIdHash = new Uint8Array(32).fill(0xc9);
  const flags = new Uint8Array([0x05]); // UP + UV flags
  const signCount = new Uint8Array([0x00, 0x00, 0x00, 0x02]);
  
  const authenticatorData = new Uint8Array([
    ...rpIdHash,
    ...flags,
    ...signCount,
  ]);
  
  // Base64URL encode challenge for clientDataJSON
  const base64UrlChallenge = btoa(String.fromCharCode(...challenge))
    .replace(/\+/g, '-')
    .replace(/\//g, '_')
    .replace(/=/g, '');
  
  const clientData = {
    type: 'webauthn.get',
    challenge: base64UrlChallenge,
    origin: 'http://localhost:3000',
    crossOrigin: false,
  };
  const encoder = new TextEncoder();
  const clientDataJSON = encoder.encode(JSON.stringify(clientData));
  
  // Mock ECDSA signature (r || s, each 32 bytes)
  const signature = new Uint8Array(64).fill(0xaa);
  
  return {
    authenticatorData: authenticatorData.buffer,
    clientDataJSON: clientDataJSON.buffer,
    signature: signature.buffer,
    userHandle: null,
  };
}

// Mock navigator.credentials
export interface MockCredentialsContainer {
  create: ReturnType<typeof vi.fn>;
  get: ReturnType<typeof vi.fn>;
}

export function createMockCredentials(): MockCredentialsContainer {
  return {
    create: vi.fn(),
    get: vi.fn(),
  };
}

// Helper to setup successful registration mock
export function setupRegistrationMock(mockCredentials: MockCredentialsContainer, credentialId = 'test-credential-id') {
  const rawId = new TextEncoder().encode(credentialId);
  const attestationResponse = {
    attestationObject: createMockAttestationObject(),
    clientDataJSON: createMockClientDataJSON('test-challenge'),
    getTransports: () => ['internal'],
    getPublicKey: () => null,
    getPublicKeyAlgorithm: () => -7,
    getAuthenticatorData: () => createMockAttestationObject(),
  };
  
  const credential = new MockPublicKeyCredential({
    id: credentialId,
    rawId: rawId.buffer,
    response: attestationResponse as unknown as AuthenticatorAttestationResponse,
  });
  
  mockCredentials.create.mockResolvedValue(credential);
  return credential;
}

// Helper to setup successful assertion mock
export function setupAssertionMock(mockCredentials: MockCredentialsContainer, challenge: Uint8Array, credentialId = 'test-credential-id') {
  const rawId = new TextEncoder().encode(credentialId);
  const assertionResponse = createMockAssertionResponse(challenge);
  
  const credential = new MockPublicKeyCredential({
    id: credentialId,
    rawId: rawId.buffer,
    response: assertionResponse as unknown as AuthenticatorAssertionResponse,
  });
  
  mockCredentials.get.mockResolvedValue(credential);
  return credential;
}

// Helper to setup user cancellation
export function setupCancellationMock(mockCredentials: MockCredentialsContainer) {
  const error = new DOMException('The operation either timed out or was not allowed.', 'NotAllowedError');
  mockCredentials.create.mockRejectedValue(error);
  mockCredentials.get.mockRejectedValue(error);
}

// Setup global mocks
let mockCredentials: MockCredentialsContainer;

beforeAll(() => {
  mockCredentials = createMockCredentials();
  
  // Mock navigator.credentials
  Object.defineProperty(global.navigator, 'credentials', {
    value: mockCredentials,
    writable: true,
    configurable: true,
  });
  
  // Mock PublicKeyCredential
  (global as any).PublicKeyCredential = MockPublicKeyCredential;
  
  // Mock PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable
  MockPublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable = vi.fn().mockResolvedValue(true);
  
  // Mock PublicKeyCredential.isConditionalMediationAvailable
  (MockPublicKeyCredential as any).isConditionalMediationAvailable = vi.fn().mockResolvedValue(true);
});

afterEach(() => {
  vi.clearAllMocks();
});

// Export for use in tests
export { mockCredentials };
