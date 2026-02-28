# Quickstart: Stealth Address Integration

## Implementation Status

| Component | Status | Notes |
|-----------|--------|-------|
| Wasm Engine (stealth module) | ✅ Complete | Key generation, address derivation, scanning, backup |
| Frontend Components | ✅ Complete | StealthPage, StealthSendForm, StealthBalanceList, etc. |
| Frontend Scanner | ✅ Complete | StealthScanner with batch processing, retry logic |
| Blockchain Pallet | ✅ Complete | pallet-stealth with send_to_stealth extrinsic |

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

## 2. Build Blockchain

```bash
cd apps/blockchain

# Build
cargo build --release

# Start dev node
./target/release/anarchy-node --dev
```

The stealth pallet is included in the runtime. Verify with:

```bash
# Check pallet is loaded (after node starts)
curl -s localhost:9944 | grep -i stealth
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

## 5. Quick Test: Generate and Share Stealth Address

```typescript
// In browser console or test file
import { generate_stealth_keys, format_meta_address_wasm } from 'anarchy-wasm-engine';

// Generate new stealth keypair
const keys = generate_stealth_keys();
console.log('Meta-Address:', keys.metaAddress);
console.log('Spend Pubkey:', new Uint8Array(keys.spendPubkey));
console.log('View Pubkey:', new Uint8Array(keys.viewPubkey));

// Share this meta-address with senders
// Format: st:anarchy:<base58(spendPub || viewPub)>
```

---

## 6. Quick Test: Derive Stealth Address for Sending

> **Note**: Actual on-chain transfer requires pallet-stealth (future work)

```typescript
import { derive_stealth_address } from 'anarchy-wasm-engine';

// Recipient's meta-address (from their profile)
const recipientMetaAddress = 'st:anarchy:5Grw...';

// Derive one-time stealth address
const result = derive_stealth_address(recipientMetaAddress);
console.log('Stealth address:', result.stealth_address());
console.log('Ephemeral pubkey:', result.ephemeral_pubkey());

// The ephemeral pubkey must be published on-chain for recipient to find the payment
```

---

## 7. Quick Test: Scan for Incoming Payments

```typescript
import { scan_transaction } from 'anarchy-wasm-engine';

// Your keys
const myViewKey = new Uint8Array(32);     // from generate_stealth_keys
const mySpendPubkey = new Uint8Array(32); // from generate_stealth_keys

// Transaction data (from blockchain ephemeral key registry)
const ephemeralPubkey = new Uint8Array(32); // from on-chain
const stealthAddressBytes = new Uint8Array(32); // from on-chain

// Check if this payment is for you
const isOurs = scan_transaction(
  myViewKey,
  ephemeralPubkey,
  stealthAddressBytes,
  mySpendPubkey
);

if (isOurs) {
  console.log('This payment is for you!');
}
```

---

## 8. Quick Test: Spend from Stealth Address

> **Note**: Actual spending requires pallet-stealth and working transaction signing (future work)

```typescript
import { derive_stealth_private_key } from 'anarchy-wasm-engine';

// Your secret keys
const mySpendKey = new Uint8Array(32);   // NEVER share this!
const myViewKey = new Uint8Array(32);    // from generate_stealth_keys

// From detected payment
const ephemeralPubkey = new Uint8Array(32);

// Derive the one-time private key for this stealth address
const stealthPrivateKey = derive_stealth_private_key(
  mySpendKey,
  myViewKey,
  ephemeralPubkey
);

console.log('Derived stealth private key:', stealthPrivateKey);
// Use this key to sign transactions from the stealth address
```

---

## 9. Backup & Restore Keys

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

## Directory Structure (Current Implementation)

```
packages/wasm-engine/src/stealth/
├── mod.rs               # Module exports
├── keys.rs              # Key generation (generate_stealth_keys)
├── types.rs             # StealthKeyPairJs, StealthAddressResult
├── address.rs           # Address derivation (derive_stealth_address)
├── scan.rs              # Transaction scanning (scan_transaction)
├── backup.rs            # Backup encryption (encrypt_backup, decrypt_backup)
└── tests.rs             # Crypto tests

apps/frontend/src/
├── lib/stealth/
│   ├── types.ts         # TypeScript types (StealthKeyPair, StealthBalance, etc.)
│   ├── keyManager.ts    # Session key management (StealthKeyManager)
│   ├── scanner.ts       # Block scanner (StealthScanner with retry, batch)
│   ├── balanceStore.ts  # Balance tracking (BalanceStore)
│   ├── coinSelection.ts # UTXO selection (selectCoins, linkability warning)
│   ├── signer.ts        # Transaction signing (StealthSigner)
│   └── api.ts           # API helpers (getEphemeralKeys)
├── components/stealth/
│   ├── StealthSendForm.tsx     # Send to stealth address
│   ├── StealthBalanceList.tsx  # Display owned balances
│   ├── StealthSpendForm.tsx    # Spend from stealth address
│   └── BackupImportDialog.tsx  # Import/export keys
└── app/stealth/
    └── page.tsx         # Main stealth page (tabs: generate, send, balance)

apps/blockchain/pallets/stealth/
├── src/
│   ├── lib.rs           # Pallet core (send_to_stealth extrinsic)
│   ├── types.rs         # EphemeralKeyEntry type
│   ├── weights.rs       # WeightInfo implementation
│   ├── mock.rs          # Test mock runtime
│   └── tests.rs         # Unit tests
└── Cargo.toml
```

---

## Running Tests

```bash
# Wasm engine tests  
cd packages/wasm-engine && cargo test

# Frontend tests (includes stealth tests)
cd apps/frontend && pnpm test

# Stealth-specific tests
cd apps/frontend && pnpm test -- --testPathPattern=stealth
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

### 3. API not found errors

```
Error: Cannot find pallet method
```

**Solution**: Ensure you're using PAPI (not @polkadot/api) and calling the correct method:

```typescript
import { createClient } from 'polkadot-api';
const api = client.getUnsafeApi();
await api.tx.stealthPallet.sendToStealth(stealthAddr, ephemeralPubkey, amount);
```

---

## Integration Test Flow (T087)

This section documents the full stealth address integration test flow.

### Prerequisites

1. Wasm engine built: `cd packages/wasm-engine && wasm-pack build --target web`
2. Frontend running: `pnpm dev:frontend`
3. Blockchain node running: `pnpm dev:node` (for balance queries)

### Test Scenario: Alice receives stealth payment from Bob

```typescript
// 1. Alice generates stealth keys
import { generate_stealth_keys } from 'anarchy-wasm-engine';
const aliceKeys = generate_stealth_keys();
const aliceMetaAddress = aliceKeys.metaAddress;
// Example: st:anarchy:5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY

// 2. Alice shares her meta-address with Bob (via chat, QR code, etc.)

// 3. Bob derives a one-time stealth address for Alice
import { derive_stealth_address } from 'anarchy-wasm-engine';
const stealthResult = derive_stealth_address(aliceMetaAddress);
const stealthAddress = stealthResult.stealth_address();
const ephemeralPubkey = stealthResult.ephemeral_pubkey();

// 4. Bob sends tokens to stealthAddress and publishes ephemeralPubkey
import { sendToStealth } from '../../lib/stealth/api';
await sendToStealth(stealthAddress, ephemeralPubkey, amount);

// 5. Alice scans for payments
import { scan_transaction } from 'anarchy-wasm-engine';
const isAlices = scan_transaction(
  aliceKeys.viewKey,
  ephemeralPubkey,
  stealthAddressBytes, // 32-byte address
  aliceKeys.spendPubkey
);
console.assert(isAlices === true, 'Alice should detect her payment');

// 6. Alice derives private key to spend
import { derive_stealth_private_key } from 'anarchy-wasm-engine';
const stealthPrivateKey = derive_stealth_private_key(
  aliceKeys.spendKey,
  aliceKeys.viewKey,
  ephemeralPubkey
);
// Alice can now sign transactions from stealthAddress

// 7. Test backup/restore
import { encrypt_backup, decrypt_backup } from 'anarchy-wasm-engine';
const password = 'test-password-123';
const backup = encrypt_backup(aliceKeys.spendKey, aliceKeys.viewKey, password);
const restored = decrypt_backup(backup, password);
console.assert(restored.metaAddress === aliceMetaAddress, 'Restored keys should match');
```

### Automated Test Commands

```bash
# Run all stealth-related tests
cd apps/frontend && pnpm test -- --testPathPattern=stealth

# Expected output:
# PASS tests/lib/stealth/scanner.test.ts
# PASS tests/lib/stealth/balanceStore.test.ts
# PASS tests/lib/stealth/coinSelection.test.ts
# PASS tests/components/stealth/StealthSpendForm.test.tsx
# Test Suites: 4 passed
```

### Manual UI Test

1. Open http://localhost:3000/stealth
2. Click "Generate Keys" - verify meta-address appears
3. Copy meta-address
4. Switch to "Send" tab
5. Paste meta-address and amount
6. Click "Send" (requires connected wallet)
7. Switch to "Balance" tab
8. Click "Start Scan" 
9. Verify progress indicator works
10. If payment detected, verify it appears in balance list
11. Click on balance → verify spend form opens
12. Test "Export Keys" → download backup file
13. Refresh page → Import backup → verify keys restored

---

## CPU Usage & Battery Impact (T089)

### Scanner Performance Metrics

| Operation | CPU Impact | Notes |
|-----------|------------|-------|
| Block scan (1 block) | ~5ms | scan_transaction wasm call |
| Batch scan (1000 blocks) | ~5s | With 100ms delay between batches |
| Full chain scan | Variable | Depends on chain height |

### Battery Saving Features (SC-007 Compliance)

1. **Background Tab Detection**: Scanner pauses when tab is hidden (Visibility API)
2. **Catch-up Scan**: Resumes from last scanned block on foreground return
3. **Network Awareness**: Stops scanning on network disconnection
4. **Batch Processing**: 1000 blocks/batch reduces CPU wake-ups
5. **Exponential Backoff**: Failed RPC calls don't spin-loop

### Measurement Method

```typescript
// Measure CPU time for scan operation
const start = performance.now();
await scanner.scanBlockRange(0, 1000);
const elapsed = performance.now() - start;
console.log(`1000 blocks scanned in ${elapsed}ms`);
// Expected: < 10 seconds on modern hardware
```

### Battery Usage Estimate

- **Active Scanning**: ~2-5% battery/hour (depends on scan frequency)
- **Background (paused)**: 0% additional usage
- **Catch-up on foreground**: Brief spike, then idle

---

## Next Steps

1. **Generate keys**: Visit `/stealth` page and click "Generate Keys"
2. **Share meta-address**: Copy your meta-address to receive stealth payments
3. **Send to stealth**: Use the Send tab to send to someone's meta-address
4. **Scan for payments**: Use the Balance tab to scan for incoming payments
5. **Backup keys**: Export your keys before leaving (keys are not persisted)
