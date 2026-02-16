# KZG SRS (Structured Reference String)

This directory contains the Trusted Setup parameters for KZG commitments.

## Ethereum KZG Ceremony

We use the Ethereum KZG Ceremony (EIP-4844) Powers of Tau result.

### Files

- `trusted_setup.txt` - Ethereum KZG Ceremony trusted setup (4096 G1 + 65 G2 points)

### Download

Download from the official Ethereum c-kzg-4844 repository:
- https://github.com/ethereum/c-kzg-4844/blob/main/src/trusted_setup.txt

### File Format

The `trusted_setup.txt` file is in Ethereum KZG Ceremony text format:
- Line 1: number of G1 points (e.g., "4096")
- Line 2: number of G2 points (e.g., "65")
- Lines 3 to (2 + num_g1): G1 points as hex strings (96 chars = 48 bytes compressed)
- Lines (3 + num_g1) to end: G2 points as hex strings (192 chars = 96 bytes compressed)

We use G2[1] (second G2 point) as τ·G₂ for KZG verification.

### Binary Format (Alternative)

A binary format can also be used:
- First 4 bytes: number of G1 points (u32 LE)
- G1 points: 48 bytes each (compressed BLS12-381 G1)
- 96 bytes: G2 point (compressed BLS12-381 G2)

### Usage

**wasm-engine:**
```rust
use anarchy_wasm_engine::kzg::init_srs_from_ceremony_text;

let text = std::fs::read_to_string("srs/trusted_setup.txt")?;
init_srs_from_ceremony_text(&text)?;
```

**storage-node:**
Add to `config.toml`:
```toml
srs_path = "/path/to/trusted_setup.txt"
dev_mode = false
```

### Development SRS

For development/testing, `dev_mode = true` uses an insecure test SRS with τ=12345.
**WARNING: Never use test SRS in production!**

### Verification

The tau_g2 (G2[1]) is embedded in `pallet-storage` as `TAU_G2_BYTES`.
Run `cargo test test_tau_g2_validity` to verify it matches the trusted setup.
