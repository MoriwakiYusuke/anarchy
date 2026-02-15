# KZG SRS (Structured Reference String)

This directory contains the Trusted Setup parameters for KZG commitments.

## Ethereum KZG Ceremony

We use the Ethereum KZG Ceremony (EIP-4844) Powers of Tau result.

### Download

Download from the official Ethereum KZG Ceremony:
- https://github.com/ethereum/c-kzg-4844/blob/main/src/trusted_setup.txt

Or use the ceremony output:
- https://ceremony.ethereum.org/

### File Format

The `mainnet.bin` file should be in the following format:
- First 4 bytes: number of G1 points (u32 LE)
- G1 points: 48 bytes each (compressed BLS12-381 G1)
- G2 point: 96 bytes (compressed BLS12-381 G2)

### Development SRS

For development/testing, a smaller SRS (degree-256 or degree-1024) can be used.
The production SRS should be degree-4096 (~200KB).

### Embedding

The SRS is embedded in the Wasm binary at compile time. To update:
1. Replace `mainnet.bin` with the new SRS file
2. Run `wasm-pack build --target web`
