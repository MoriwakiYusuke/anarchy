# Quickstart: Stealth Address Integration

## Prerequisites

- Rust 1.87 (stable with wasm32v1-none target)
- Node.js 18+ / pnpm
- wasm-pack (`cargo install wasm-pack`)
- Running Anarchy node (`pnpm dev:node`)

---

## 1. Build Wasm Engine with Stealth Module

```bash
cd packages/wasm-engine

# Rebuild with stealth module
wasm-pack build --target web --out-dir pkg

# Verify exports
grep "stealth" pkg/anarchy_wasm_engine.d.ts
# Should show: generate_stealth_keys, derive_stealth_address, etc.
```

---

## 2. Build Blockchain with Stealth Pallet

```bash
cd apps/blockchain

# Build with new pallet
cargo build --release

# Run tests
cargo test -p pallet-stealth

# Start dev node
./target/release/anarchy-node --dev
```

---

## 3. Frontend Development

```bash
# Install dependencies (includes local wasm-engine)
pnpm install

# Start frontend dev server
pnpm dev:frontend
```

---

## 4. Quick Test: Generate Stealth Keys

```typescript
// In browser console or test file
import { generate_stealth_keys } from 'anarchy-wasm-engine';

const keys = generate_stealth_keys();
console.log('Meta-Address:', keys.metaAddress);
console.log('Spend Pubkey:', keys.spendPubkey);
console.log('View Pubkey:', keys.viewPubkey);
```

---

## 5. Quick Test: Send to Stealth Address

```typescript
import { createClient } from 'polkadot-api';
import { getWsProvider } from 'polkadot-api/ws-provider/node';
import { derive_stealth_address } from 'anarchy-wasm-engine';

// Connect to node
const client = createClient(getWsProvider('ws://127.0.0.1:9944'));
const api = client.getUnsafeApi();

// Derive stealth address for recipient
const recipientMetaAddress = 'st:anarchy:5Grw...';
const { stealthAddress, ephemeralPubkey } = derive_stealth_address(recipientMetaAddress);

// Send to stealth address (10 MORAL)
const amount = 10_000_000_000_000n;  // 10 MORAL
const tx = api.tx.stealthPallet.sendToStealth(
  stealthAddress,
  ephemeralPubkey,
  amount
);

await tx.signAndSubmit(senderSigner);
console.log('Sent to stealth address:', stealthAddress);
```

---

## 6. Quick Test: Scan for Incoming Payments

```typescript
import { scan_transaction } from 'anarchy-wasm-engine';

async function scanBlocks(api, myViewKey, mySpendPubkey, startBlock, endBlock) {
  const detected = [];
  
  for (let block = startBlock; block <= endBlock; block++) {
    const entries = await api.query.stealthPallet.ephemeralKeys(block);
    
    for (const entry of entries) {
      const isOurs = scan_transaction(
        myViewKey,
        entry.ephemeralPubkey,
        entry.stealthAddress,
        mySpendPubkey
      );
      
      if (isOurs) {
        console.log(`Detected payment at block ${block}:`, entry.stealthAddress);
        detected.push({
          block,
          stealthAddress: entry.stealthAddress,
          ephemeralPubkey: entry.ephemeralPubkey,
        });
      }
    }
  }
  
  return detected;
}

// Usage
const payments = await scanBlocks(api, myViewKey, mySpendPubkey, 0, 1000);
```

---

## 7. Quick Test: Spend from Stealth Address

```typescript
import { derive_stealth_private_key } from 'anarchy-wasm-engine';

// Derive private key for detected stealth address
const stealthPrivateKey = derive_stealth_private_key(
  mySpendKey,
  myViewKey,
  detectedPayment.ephemeralPubkey
);

// Create signer from stealth private key
const stealthSigner = createSignerFromPrivateKey(stealthPrivateKey);

// Transfer out
const tx = api.tx.balances.transfer(
  recipientAddress,
  transferAmount
);

await tx.signAndSubmit(stealthSigner);
console.log('Spent from stealth address');
```

---

## 8. Backup & Restore Keys

```typescript
import { encrypt_backup, decrypt_backup } from 'anarchy-wasm-engine';

// Create backup
const password = 'my-secure-password';
const backupData = encrypt_backup(mySpendKey, myViewKey, password);

// Download as file
const blob = new Blob([backupData], { type: 'application/octet-stream' });
const url = URL.createObjectURL(blob);
const a = document.createElement('a');
a.href = url;
a.download = 'stealth-backup.bin';
a.click();

// Restore from backup
const fileData = await readFile(backupFile);
const restoredKeys = decrypt_backup(fileData, password);
console.log('Restored meta-address:', restoredKeys.metaAddress);
```

---

## Directory Structure After Implementation

```
apps/blockchain/pallets/stealth/
├── Cargo.toml
└── src/
    ├── lib.rs           # Pallet implementation
    ├── types.rs         # EphemeralKeyEntry, etc.
    ├── weights.rs       # Benchmark weights
    └── tests.rs         # Unit tests

packages/wasm-engine/src/stealth/
├── mod.rs               # Module exports
├── keys.rs              # Key generation
├── address.rs           # Address derivation
├── scan.rs              # Transaction scanning
├── backup.rs            # Backup encryption
└── tests.rs             # Crypto tests

apps/frontend/src/
├── lib/stealth/
│   ├── worker.ts        # Web Worker
│   ├── scanner.ts       # Background scanner
│   ├── keyManager.ts    # Session key management
│   └── types.ts         # TypeScript types
├── components/stealth/
│   ├── StealthAddressGenerator.tsx
│   ├── StealthSendForm.tsx
│   ├── StealthBalanceList.tsx
│   └── BackupImportDialog.tsx
└── app/stealth/
    └── page.tsx         # Main stealth page
```

---

## Running Tests

```bash
# Pallet tests
cargo test -p pallet-stealth

# Wasm engine tests  
cd packages/wasm-engine && cargo test

# Frontend tests
cd apps/frontend && pnpm test
```

---

## Common Issues

### 1. Wasm module not found

```
Error: Failed to load wasm module
```

**Solution**: Rebuild wasm-engine and reinstall frontend dependencies:

```bash
cd packages/wasm-engine && wasm-pack build --target web --out-dir pkg
cd ../.. && pnpm install
```

### 2. RPC connection failed

```
Error: WebSocket connection failed
```

**Solution**: Ensure dev node is running:

```bash
pnpm dev:node
```

### 3. Stealth pallet not found

```
Error: Cannot find pallet 'stealthPallet'
```

**Solution**: Rebuild blockchain and restart node:

```bash
cd apps/blockchain && cargo build --release
./target/release/anarchy-node --dev --force-authoring
```
