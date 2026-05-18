# WebAuthn Signature Verification Research

## Overview

This document provides research findings for implementing WebAuthn signature verification in a Substrate no_std runtime environment. The focus is on ECDSA P-256 (ES256) signature verification, COSE key parsing, and Substrate integration.

---

## 1. p256/ecdsa Crates no_std Compatibility

### Recommended Crate Configuration

```toml
[dependencies]
p256 = { version = "0.13", default-features = false, features = ["ecdsa-core", "alloc"] }
ecdsa = { version = "0.16", default-features = false, features = ["verifying"] }
sha2 = { version = "0.10", default-features = false }
```

### Feature Analysis

#### p256 v0.13.2
- **Default features**: `arithmetic`, `ecdsa`, `pem`, `std`
- **Optional features**:
  - `std` - Standard library support (disable for no_std)
  - `alloc` - Heap allocations without std
  - `arithmetic` - Elliptic curve arithmetic operations
  - `ecdsa` - Full ECDSA implementation
  - `ecdsa-core` - Core ECDSA types without std dependencies
  - `pkcs8` - PKCS#8 key format support
  - `pem` - PEM encoding (requires std)
  - `sha256` - SHA-256 digest integration
  - `jwk` - JSON Web Key support
  - `serde` - Serialization support

For no_std runtime:
```toml
p256 = { version = "0.13", default-features = false, features = ["ecdsa-core", "alloc"] }
```

#### ecdsa v0.16.9
- **Default features**: `digest` only
- **Optional features**:
  - `std` - Standard library support
  - `alloc` - Heap allocations
  - `verifying` - Signature verification support (**required**)
  - `signing` - Signature creation support
  - `der` - DER encoding/decoding for signatures
  - `hazmat` - Low-level cryptographic operations
  - `pem` - PEM format support
  - `pkcs8` - PKCS#8 format support

For no_std runtime:
```toml
ecdsa = { version = "0.16", default-features = false, features = ["verifying"] }
```

#### sha2 v0.10.9
- **Default features**: `std`
- **Optional features**:
  - `std` - Standard library support (disable for no_std)
  - `oid` - OID support for algorithms
  - `asm` - Assembly optimizations (x86/x86_64)
  - `compress` - Low-level compression functions
  - `force-soft` - Force software implementation

For no_std runtime:
```toml
sha2 = { version = "0.10", default-features = false }
```

### no_std Compatibility Notes

All RustCrypto crates (p256, ecdsa, sha2) are designed for no_std environments:

1. **No external C dependencies** - Pure Rust implementations
2. **Optional alloc feature** - Uses `alloc` crate for heap allocations when needed
3. **Core-only operation** - Can operate with only `core` library

When using in Substrate runtime:
- All crates compile to WASM without std
- Memory allocations use WASM linear memory via `sp-std`
- No filesystem or network dependencies

---

## 2. COSE Public Key Parsing

### COSE Key Format for ES256

```
COSE_Key = {
  1: kty,      ; Key Type (EC2 = 2)
  3: alg,      ; Algorithm (ES256 = -7)
 -1: crv,      ; Curve (P-256 = 1)
 -2: x,        ; X coordinate (32 bytes)
 -3: y,        ; Y coordinate (32 bytes)
}
```

#### CBOR Label Values
| Label | Name | ES256 Value | Description |
|-------|------|-------------|-------------|
| 1 | kty | 2 | Key Type: EC2 (Elliptic Curve with x,y) |
| 3 | alg | -7 | Algorithm: ES256 (ECDSA w/ SHA-256) |
| -1 | crv | 1 | Curve: P-256 (secp256r1) |
| -2 | x | bytes(32) | X coordinate of public key |
| -3 | y | bytes(32) | Y coordinate of public key |

### Parsing Options

#### Option A: coset Crate (Recommended)

```toml
[dependencies]
coset = { version = "0.4", default-features = false }
ciborium = { version = "0.2", default-features = false }
ciborium-io = { version = "0.2", default-features = false, features = ["alloc"] }
```

**no_std Support**: coset v0.4.1 is configured for no_std:
- Uses `ciborium` with `default-features = false`
- Uses `ciborium-io` with `alloc` feature
- Uses `core::error::Error` (not `std::error::Error`)

```rust
use coset::{CoseKey, Label, KeyType, Algorithm};

fn parse_cose_key(bytes: &[u8]) -> Result<(Vec<u8>, Vec<u8>), Error> {
    let cose_key = CoseKey::from_slice(bytes)?;
    
    // Validate key type and algorithm
    if cose_key.kty != KeyType::Assigned(iana::KeyType::EC2) {
        return Err(Error::InvalidKeyType);
    }
    
    if cose_key.alg != Some(Algorithm::Assigned(iana::Algorithm::ES256)) {
        return Err(Error::InvalidAlgorithm);
    }
    
    // Extract x and y coordinates
    let x = cose_key.params.iter()
        .find(|(label, _)| *label == Label::Int(-2))
        .and_then(|(_, value)| value.as_bytes())
        .ok_or(Error::MissingX)?;
    
    let y = cose_key.params.iter()
        .find(|(label, _)| *label == Label::Int(-3))
        .and_then(|(_, value)| value.as_bytes())
        .ok_or(Error::MissingY)?;
    
    Ok((x.to_vec(), y.to_vec()))
}
```

#### Option B: Manual CBOR Parsing

For minimal dependencies, implement manual CBOR parsing:

```rust
/// CBOR major types
const CBOR_UINT: u8 = 0;
const CBOR_NEGINT: u8 = 1;
const CBOR_BYTES: u8 = 2;
const CBOR_MAP: u8 = 5;

/// Parse COSE_Key for ES256
fn parse_cose_key_manual(data: &[u8]) -> Result<Ec2PublicKey, Error> {
    let mut cursor = 0;
    
    // Expect map
    let (major, arg) = decode_cbor_head(data, &mut cursor)?;
    if major != CBOR_MAP {
        return Err(Error::ExpectedMap);
    }
    
    let mut kty: Option<i64> = None;
    let mut alg: Option<i64> = None;
    let mut crv: Option<i64> = None;
    let mut x: Option<Vec<u8>> = None;
    let mut y: Option<Vec<u8>> = None;
    
    for _ in 0..arg {
        let label = decode_cbor_int(data, &mut cursor)?;
        
        match label {
            1 => kty = Some(decode_cbor_int(data, &mut cursor)?),
            3 => alg = Some(decode_cbor_int(data, &mut cursor)?),
            -1 => crv = Some(decode_cbor_int(data, &mut cursor)?),
            -2 => x = Some(decode_cbor_bytes(data, &mut cursor)?),
            -3 => y = Some(decode_cbor_bytes(data, &mut cursor)?),
            _ => skip_cbor_value(data, &mut cursor)?,
        }
    }
    
    // Validate ES256 parameters
    if kty != Some(2) || alg != Some(-7) || crv != Some(1) {
        return Err(Error::InvalidKeyParameters);
    }
    
    let x = x.ok_or(Error::MissingX)?;
    let y = y.ok_or(Error::MissingY)?;
    
    if x.len() != 32 || y.len() != 32 {
        return Err(Error::InvalidCoordinateLength);
    }
    
    Ok(Ec2PublicKey { x, y })
}
```

### Recommendation

Use **coset** unless:
- Binary size is critical concern
- Need minimal dependencies
- CBOR parsing is simple (only need EC2/ES256)

Manual parsing adds ~200-300 lines of code but removes ~50KB from WASM binary.

---

## 3. Signature Format Handling (DER vs Raw)

### ES256 Signature Formats

#### DER Format (ASN.1 ECDSA-Sig-Value)

WebAuthn/FIDO2 signatures are DER-encoded per RFC 3279:

```asn1
ECDSA-Sig-Value ::= SEQUENCE {
    r INTEGER,
    s INTEGER
}
```

**DER Structure**:
```
30 <total-length>           ; SEQUENCE tag + length
  02 <r-length> <r-bytes>   ; INTEGER tag + length + r value
  02 <s-length> <s-bytes>   ; INTEGER tag + length + s value
```

**Variable Length**: DER-encoded signatures are 70-72 bytes for P-256:
- SEQUENCE tag: 1 byte
- SEQUENCE length: 1 byte
- INTEGER tag (r): 1 byte
- INTEGER length (r): 1 byte
- r value: 32-33 bytes (extra 0x00 if high bit set)
- INTEGER tag (s): 1 byte
- INTEGER length (s): 1 byte
- s value: 32-33 bytes (extra 0x00 if high bit set)

#### Raw Format (Fixed-Size Concatenation)

Some contexts use fixed-size r||s format:

```
r: 32 bytes (big-endian, zero-padded)
s: 32 bytes (big-endian, zero-padded)
Total: 64 bytes
```

### DER to Raw Conversion

```rust
/// Convert DER-encoded ECDSA signature to raw (r || s) format
fn der_to_raw(der: &[u8]) -> Result<[u8; 64], Error> {
    // Minimum valid DER: 30 44 02 20 <r:32> 02 20 <s:32>
    if der.len() < 8 {
        return Err(Error::SignatureTooShort);
    }
    
    // Check SEQUENCE tag
    if der[0] != 0x30 {
        return Err(Error::InvalidDerTag);
    }
    
    let mut idx = 2; // Skip SEQUENCE tag and length
    
    // Parse r INTEGER
    if der[idx] != 0x02 {
        return Err(Error::InvalidIntegerTag);
    }
    idx += 1;
    
    let r_len = der[idx] as usize;
    idx += 1;
    
    let r_start = if r_len == 33 && der[idx] == 0x00 {
        idx + 1  // Skip leading zero
    } else {
        idx
    };
    let r = &der[r_start..r_start + 32];
    idx += r_len;
    
    // Parse s INTEGER
    if der[idx] != 0x02 {
        return Err(Error::InvalidIntegerTag);
    }
    idx += 1;
    
    let s_len = der[idx] as usize;
    idx += 1;
    
    let s_start = if s_len == 33 && der[idx] == 0x00 {
        idx + 1  // Skip leading zero
    } else {
        idx
    };
    let s = &der[s_start..s_start + 32];
    
    // Construct raw signature
    let mut raw = [0u8; 64];
    raw[..32].copy_from_slice(r);
    raw[32..].copy_from_slice(s);
    
    Ok(raw)
}
```

### Detection Heuristic

```rust
fn detect_signature_format(sig: &[u8]) -> SignatureFormat {
    match sig.len() {
        64 => SignatureFormat::Raw,
        70..=72 if sig[0] == 0x30 => SignatureFormat::Der,
        _ => SignatureFormat::Unknown,
    }
}
```

### Using ecdsa Crate

The ecdsa crate provides built-in DER handling:

```rust
use ecdsa::{Signature, signature::Verifier};
use p256::ecdsa::VerifyingKey;

// Parse DER signature
let signature = Signature::from_der(der_bytes)?;

// Or parse raw signature
let signature = Signature::from_bytes(raw_bytes.into())?;
```

---

## 4. WebAuthn Signature Verification Flow

### authenticatorData Structure

```
authenticatorData (≥37 bytes):
┌─────────────────────────────────────────────────────────────┐
│ rpIdHash (32 bytes) │ flags (1) │ signCount (4) │ ...opt... │
└─────────────────────────────────────────────────────────────┘
```

| Field | Bytes | Description |
|-------|-------|-------------|
| rpIdHash | 32 | SHA-256 hash of Relying Party ID |
| flags | 1 | Bit flags for authenticator state |
| signCount | 4 | Big-endian signature counter |
| attestedCredentialData | variable | (Optional) Present if AT flag set |
| extensions | variable | (Optional) Present if ED flag set |

#### Flags Byte

| Bit | Name | Description |
|-----|------|-------------|
| 0 | UP | User Present |
| 2 | UV | User Verified |
| 6 | AT | Attested credential data included |
| 7 | ED | Extension data included |

```rust
const FLAG_UP: u8 = 0x01;  // bit 0
const FLAG_UV: u8 = 0x04;  // bit 2
const FLAG_AT: u8 = 0x40;  // bit 6
const FLAG_ED: u8 = 0x80;  // bit 7

fn parse_authenticator_data(data: &[u8]) -> Result<AuthenticatorData, Error> {
    if data.len() < 37 {
        return Err(Error::AuthDataTooShort);
    }
    
    let rp_id_hash: [u8; 32] = data[0..32].try_into()?;
    let flags = data[32];
    let sign_count = u32::from_be_bytes(data[33..37].try_into()?);
    
    Ok(AuthenticatorData {
        rp_id_hash,
        user_present: flags & FLAG_UP != 0,
        user_verified: flags & FLAG_UV != 0,
        sign_count,
        // Parse attestedCredentialData if AT flag is set
        // Parse extensions if ED flag is set
    })
}
```

### clientDataJSON Structure

```json
{
  "type": "webauthn.get",
  "challenge": "<base64url-encoded-challenge>",
  "origin": "https://example.com",
  "crossOrigin": false
}
```

| Field | Description |
|-------|-------------|
| type | `"webauthn.create"` for registration, `"webauthn.get"` for authentication |
| challenge | Base64URL-encoded challenge from server |
| origin | Origin of the calling page |
| crossOrigin | (Optional) Whether request was cross-origin |

### Signature Verification Algorithm

The signed message is computed as:

```
signedData = authenticatorData || SHA-256(clientDataJSON)
signature = ECDSA-Sign(privateKey, SHA-256(signedData))
```

Verification:

```
SHA-256(signedData) = SHA-256(authenticatorData || SHA-256(clientDataJSON))
valid = ECDSA-Verify(publicKey, SHA-256(signedData), signature)
```

### Complete Verification Flow

```rust
use p256::ecdsa::{signature::Verifier, Signature, VerifyingKey};
use sha2::{Sha256, Digest};

pub fn verify_webauthn_signature(
    public_key: &VerifyingKey,
    authenticator_data: &[u8],
    client_data_json: &[u8],
    signature: &Signature,
) -> Result<bool, Error> {
    // Step 1: Hash clientDataJSON
    let client_data_hash = Sha256::digest(client_data_json);
    
    // Step 2: Concatenate authenticatorData || clientDataHash
    let mut signed_data = Vec::with_capacity(
        authenticator_data.len() + 32
    );
    signed_data.extend_from_slice(authenticator_data);
    signed_data.extend_from_slice(&client_data_hash);
    
    // Step 3: Verify signature
    // p256 crate internally hashes signed_data with SHA-256
    public_key.verify(&signed_data, signature)
        .map(|_| true)
        .map_err(|_| Error::InvalidSignature)
}
```

### WYSIWYS (What You See Is What You Sign) Verification

To ensure the user sees what they're signing:

```rust
pub fn verify_challenge_binding(
    client_data_json: &[u8],
    expected_challenge: &[u8],
    expected_origin: &str,
) -> Result<(), Error> {
    // Note: JSON parsing in no_std requires minimal-json or serde-json-core
    let client_data: ClientData = parse_client_data_json(client_data_json)?;
    
    // Verify type
    if client_data.r#type != "webauthn.get" {
        return Err(Error::InvalidType);
    }
    
    // Verify challenge
    let decoded_challenge = base64url_decode(&client_data.challenge)?;
    if decoded_challenge != expected_challenge {
        return Err(Error::ChallengeMismatch);
    }
    
    // Verify origin
    if client_data.origin != expected_origin {
        return Err(Error::OriginMismatch);
    }
    
    Ok(())
}
```

---

## 5. Substrate-Specific Considerations

### Pallet Integration

#### Dependency Configuration

```toml
[dependencies]
# Substrate dependencies
sp-std = { version = "14.0.0", default-features = false }
sp-runtime = { version = "31.0.0", default-features = false }
sp-core = { version = "28.0.0", default-features = false }
frame-support = { version = "28.0.0", default-features = false }
frame-system = { version = "28.0.0", default-features = false }

# Cryptographic dependencies
p256 = { version = "0.13", default-features = false, features = ["ecdsa-core", "alloc"] }
ecdsa = { version = "0.16", default-features = false, features = ["verifying"] }
sha2 = { version = "0.10", default-features = false }

# CBOR/COSE parsing
coset = { version = "0.4", default-features = false }
ciborium = { version = "0.2", default-features = false }

[features]
default = ["std"]
std = [
    "sp-std/std",
    "sp-runtime/std",
    "sp-core/std",
    "frame-support/std",
    "frame-system/std",
    "p256/std",
    "ecdsa/std",
    "sha2/std",
    "coset/std",
    "ciborium/std",
]
```

#### Import Pattern

```rust
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::vec::Vec;
use sp_std::prelude::*;
use frame_support::{dispatch::DispatchResult, pallet_prelude::*};
```

### Weight Calculation

ECDSA signature verification is computationally expensive. Use Substrate's benchmarking framework:

```rust
#[pallet::weight(T::WeightInfo::verify_webauthn())]
pub fn verify_webauthn(
    origin: OriginFor<T>,
    authenticator_data: Vec<u8>,
    client_data_json: Vec<u8>,
    signature: Vec<u8>,
) -> DispatchResult {
    // ...
}
```

#### Benchmark Implementation

```rust
#[benchmarks]
mod benchmarks {
    use super::*;
    use frame_benchmarking::v2::*;
    
    #[benchmark]
    fn verify_webauthn() {
        // Setup: Create valid test data
        let authenticator_data = vec![0u8; 37];
        let client_data_json = br#"{"type":"webauthn.get","challenge":"..."}"#.to_vec();
        let signature = vec![0u8; 64];
        let caller: T::AccountId = whitelisted_caller();
        
        #[extrinsic_call]
        _(
            RawOrigin::Signed(caller),
            authenticator_data,
            client_data_json,
            signature,
        );
    }
}
```

#### Estimated Weights

Based on similar cryptographic operations in Substrate:

| Operation | Estimated Weight |
|-----------|------------------|
| SHA-256 (per KB) | ~10,000 |
| ECDSA P-256 verify | ~500,000 - 1,000,000 |
| CBOR parsing (simple) | ~5,000 - 10,000 |
| DER signature parsing | ~1,000 - 2,000 |
| **Total verify_webauthn** | ~600,000 - 1,200,000 |

Actual values should be determined by benchmarking on target hardware.

### Storage Considerations

```rust
#[pallet::storage]
pub type PublicKeys<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    T::AccountId,
    CosePublicKey,  // 65-66 bytes for uncompressed EC point
    OptionQuery,
>;

#[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen)]
pub struct CosePublicKey {
    /// X coordinate (32 bytes)
    pub x: [u8; 32],
    /// Y coordinate (32 bytes)  
    pub y: [u8; 32],
}
```

### Error Handling

```rust
#[pallet::error]
pub enum Error<T> {
    /// Public key not registered
    PublicKeyNotFound,
    /// Invalid COSE key format
    InvalidCoseKey,
    /// Invalid signature format (neither DER nor raw)
    InvalidSignatureFormat,
    /// Signature verification failed
    SignatureVerificationFailed,
    /// Invalid authenticatorData
    InvalidAuthenticatorData,
    /// Challenge mismatch
    ChallengeMismatch,
    /// Origin mismatch
    OriginMismatch,
    /// User presence flag not set
    UserNotPresent,
}
```

### JSON Parsing in no_std

For clientDataJSON parsing, options include:

1. **serde-json-core** - Minimal JSON parser for no_std
2. **miniserde** - Tiny serde alternative
3. **Manual parsing** - For simple, known structures

```toml
# Option 1: serde-json-core
serde-json-core = { version = "0.5", default-features = false }
serde = { version = "1.0", default-features = false, features = ["derive", "alloc"] }
```

### Security Considerations

1. **Constant-time comparison** for challenge verification
2. **Replay protection** via sign_count validation
3. **Origin validation** to prevent phishing
4. **Input validation** for all byte arrays (length checks)

```rust
use sp_core::ConstantTimeEq;

fn verify_challenge(expected: &[u8], actual: &[u8]) -> bool {
    expected.ct_eq(actual).into()
}
```

---

## Summary

### Recommended Cargo.toml Dependencies

```toml
[dependencies]
# Cryptography
p256 = { version = "0.13", default-features = false, features = ["ecdsa-core", "alloc"] }
ecdsa = { version = "0.16", default-features = false, features = ["verifying"] }
sha2 = { version = "0.10", default-features = false }

# COSE/CBOR
coset = { version = "0.4", default-features = false }
ciborium = { version = "0.2", default-features = false }

# JSON (for clientDataJSON)
serde = { version = "1.0", default-features = false, features = ["derive", "alloc"] }
serde-json-core = { version = "0.5", default-features = false }

[features]
std = [
    "p256/std",
    "ecdsa/std", 
    "sha2/std",
    "coset/std",
    "ciborium/std",
    "serde/std",
    "serde-json-core/std",
]
```

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                     Pallet Identity                         │
├─────────────────────────────────────────────────────────────┤
│  verify_webauthn_signature()                                │
│    ├── parse_cose_key()          [coset/manual]            │
│    ├── parse_authenticator_data() [manual]                 │
│    ├── parse_client_data_json()  [serde-json-core]         │
│    ├── verify_challenge()        [constant-time]           │
│    └── verify_ecdsa()            [p256/ecdsa]              │
└─────────────────────────────────────────────────────────────┘
```

### Implementation Checklist

- [ ] Add cryptographic dependencies with no_std features
- [ ] Implement COSE key parsing (coset or manual)
- [ ] Implement DER signature parsing/conversion
- [ ] Implement authenticatorData parsing
- [ ] Implement clientDataJSON parsing
- [ ] Implement signature verification
- [ ] Add comprehensive error types
- [ ] Write benchmarks for weight calculation
- [ ] Add unit tests with real WebAuthn test vectors
- [ ] Security audit for timing attacks
