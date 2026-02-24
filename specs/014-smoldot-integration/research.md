# Research: smoldot Light Client統合 with polkadot-api (PAPI)

**Date**: 2026-02-24  
**Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)

## 1. Package Names and Versions

### Required Packages for smoldot + PAPI Integration

```json
{
  "dependencies": {
    "polkadot-api": "^1.23.3",  // Already installed in the project
    "smoldot": "^2.0.40"        // Core smoldot light client
  }
}
```

**Note**: `@polkadot-api/smoldot` and `@polkadot-api/sm-provider` are internal PAPI packages already bundled with `polkadot-api`. No need to install them separately.

### Current Package Versions (from pnpm-lock.yaml)

| Package | Version | Notes |
|---------|---------|-------|
| `polkadot-api` | ^1.23.3 | Already installed |
| `smoldot` | 2.0.40 | Transitive dependency via PAPI |
| `@polkadot-api/smoldot` | 0.3.15 | Bundled with PAPI |
| `@polkadot-api/sm-provider` | 0.1.16 | Bundled with PAPI |

**Compatibility**: polkadot-api ^1.23.3 is compatible with smoldot 2.x series. No additional packages needed beyond adding `smoldot` as a direct dependency.

---

## 2. smoldot Package: How It Works

### Core Concepts

smoldot is a light client for Substrate-based chains (Polkadot, Kusama, custom chains). Key characteristics:

- **Byzantine-resilient**: Doesn't trust RPC servers; connects directly to P2P network
- **Browser-native**: Compiles to WebAssembly, runs in browsers
- **No full state sync**: Only downloads finalized block headers

### Browser Requirements

| Feature | Requirement | Notes |
|---------|------------|-------|
| WebAssembly | Required | All modern browsers support this |
| Web Worker | Strongly Recommended | Prevents UI blocking |
| SharedArrayBuffer | **Not Required** | smoldot works without it |

**Important Finding**: smoldot does NOT require `SharedArrayBuffer` or cross-origin isolation headers (COOP/COEP). These are only needed if using `SharedArrayBuffer` for shared memory between workers, which smoldot doesn't require.

### Web Worker Setup

smoldot runs CPU-intensive WebAssembly code. Running in a Web Worker prevents UI jank.

**Main Thread (Not Recommended)**:
```typescript
import { start } from 'polkadot-api/smoldot'
const smoldot = start()  // Blocks main thread
```

**Web Worker (Recommended)**:

Using Webpack/Next.js:
```typescript
// main.ts
import { startFromWorker } from 'polkadot-api/smoldot/from-worker'

const smWorker = new Worker(
  new URL('polkadot-api/smoldot/worker', import.meta.url)
)
const smoldot = startFromWorker(smWorker)
```

Using Vite:
```typescript
import { startFromWorker } from 'polkadot-api/smoldot/from-worker'
import SmWorker from 'polkadot-api/smoldot/worker?worker'
const smoldot = startFromWorker(new SmWorker())
```

---

## 3. Chain Spec Export Command

### Command to Export Chain Spec

The Anarchy node binary supports `build-spec` subcommand:

```bash
# Export raw chain spec (recommended for smoldot)
./target/release/anarchy-node build-spec --raw > chainspec.json

# Export from dev chain
./target/release/anarchy-node build-spec --dev --raw > chainspec-dev.json

# With custom chain
./target/release/anarchy-node build-spec --chain=local --raw > chainspec-local.json
```

### Key Flags

| Flag | Description |
|------|-------------|
| `--raw` | Output raw genesis storage (required for smoldot) |
| `--disable-default-bootnode` | Exclude default localhost bootnode |
| `--chain <CHAIN_SPEC>` | Input chain (dev, local, staging, or file) |
| `--dev` | Use development chain |

### Chain Spec Contents Required for smoldot

```json
{
  "name": "Anarchy",
  "id": "anarchy",
  "chainType": "Live", // or "Development", "Local"
  "bootNodes": [
    "/ip4/<IP>/tcp/30333/p2p/<PEER_ID>",
    "/dns4/bootnode.example.com/tcp/30333/p2p/<PEER_ID>"
  ],
  "genesis": { "raw": { ... } },  // Required: raw genesis storage
  // ... other fields
}
```

**Critical**: smoldot requires `bootNodes` to contain reachable P2P addresses. For development, add your local node's address.

---

## 4. polkadot-api smoldot Provider: Complete Example

### Full Integration Code

```typescript
// apps/frontend/src/lib/smoldot-provider.ts
import { start } from 'polkadot-api/smoldot'
import { getSmProvider } from 'polkadot-api/sm-provider'
import { createClient } from 'polkadot-api'
import chainSpec from './chainspec.json'

// Option 1: Main thread (simpler, but blocks UI)
export async function createSmoldotClient() {
  // Start smoldot
  const smoldot = start()
  
  // Add chain with chain spec
  const chain = await smoldot.addChain({ 
    chainSpec: JSON.stringify(chainSpec) 
  })
  
  // Create PAPI provider from smoldot chain
  const provider = getSmProvider(chain)
  
  // Create PAPI client
  const client = createClient(provider)
  
  return { client, smoldot, chain }
}
```

### Web Worker Integration (Recommended)

```typescript
// apps/frontend/src/lib/smoldot-provider.ts
import { startFromWorker } from 'polkadot-api/smoldot/from-worker'
import { getSmProvider } from 'polkadot-api/sm-provider'
import { createClient, PolkadotClient } from 'polkadot-api'
import chainSpec from './chainspec.json'

let smoldotInstance: Awaited<ReturnType<typeof startFromWorker>> | null = null
let clientInstance: PolkadotClient | null = null

export async function initSmoldotClient(): Promise<PolkadotClient> {
  if (clientInstance) return clientInstance
  
  // Create worker for smoldot (Next.js/Webpack compatible)
  const smWorker = new Worker(
    new URL('polkadot-api/smoldot/worker', import.meta.url)
  )
  
  // Start smoldot in worker
  smoldotInstance = startFromWorker(smWorker)
  
  // Add our chain
  const chain = smoldotInstance.addChain({ 
    chainSpec: JSON.stringify(chainSpec)
  })
  
  // Create provider (no need to await chain!)
  const provider = getSmProvider(chain)
  
  // Create client
  clientInstance = createClient(provider)
  
  return clientInstance
}

export function destroySmoldotClient() {
  if (clientInstance) {
    clientInstance.destroy()
    clientInstance = null
  }
  if (smoldotInstance) {
    smoldotInstance.terminate()
    smoldotInstance = null
  }
}
```

### React Hook Integration

```typescript
// apps/frontend/src/hooks/useSmoldot.ts
'use client'

import { useState, useEffect, useCallback } from 'react'
import { PolkadotClient } from 'polkadot-api'
import { initSmoldotClient, destroySmoldotClient } from '@/lib/smoldot-provider'

export type ConnectionState = 
  | 'initializing'  // smoldot starting
  | 'syncing'       // Chain syncing
  | 'connected'     // Ready for use
  | 'error'         // Failed

export interface UseSmoldotResult {
  client: PolkadotClient | null
  unsafeApi: any
  connectionState: ConnectionState
  error: string | null
}

export function useSmoldot(): UseSmoldotResult {
  const [client, setClient] = useState<PolkadotClient | null>(null)
  const [unsafeApi, setUnsafeApi] = useState<any>(null)
  const [connectionState, setConnectionState] = useState<ConnectionState>('initializing')
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    let mounted = true

    const init = async () => {
      try {
        setConnectionState('initializing')
        
        const clientInstance = await initSmoldotClient()
        if (!mounted) return
        
        setClient(clientInstance)
        setUnsafeApi(clientInstance.getUnsafeApi())
        setConnectionState('syncing')
        
        // Wait for sync by querying latest block
        const api = clientInstance.getUnsafeApi()
        await api.query.System.Number.getValue()
        
        if (!mounted) return
        setConnectionState('connected')
        
      } catch (err) {
        if (!mounted) return
        setConnectionState('error')
        setError(err instanceof Error ? err.message : 'smoldot初期化に失敗しました')
      }
    }

    init()

    return () => {
      mounted = false
      destroySmoldotClient()
    }
  }, [])

  return { client, unsafeApi, connectionState, error }
}
```

---

## 5. Compatibility Concerns with polkadot-api ^1.23.3

### No Known Issues

- polkadot-api 1.23.x has built-in smoldot support via `polkadot-api/smoldot` exports
- The internal `@polkadot-api/smoldot@0.3.15` works with `smoldot@2.0.40`
- TypeScript types are fully included

### Potential Gotchas

1. **Next.js Dynamic Import for Workers**
   
   Next.js requires special handling for Web Workers in client components:
   ```typescript
   // Must be in 'use client' component
   // Worker import must use import.meta.url pattern
   ```

2. **Chain Spec JSON Import**
   
   TypeScript may need configuration for JSON imports:
   ```json
   // tsconfig.json
   {
     "compilerOptions": {
       "resolveJsonModule": true
     }
   }
   ```

3. **getSmProvider Accepts Promise**
   
   `getSmProvider(chain)` can take a `Promise<Chain>` directly - no need to await:
   ```typescript
   const chain = smoldot.addChain({ chainSpec })  // Returns Promise<Chain>
   const provider = getSmProvider(chain)           // Works with Promise!
   ```

4. **No Fallback Needed**
   
   Per spec, WebSocket fallback is NOT required. smoldot-only is acceptable.

---

## 6. Existing Codebase Analysis

### Current WebSocket Provider Usage

File: [apps/frontend/src/hooks/useApi.ts](../../apps/frontend/src/hooks/useApi.ts)

```typescript
// Current implementation uses:
import { getWsProvider } from 'polkadot-api/ws-provider/web'

// Creates client:
const provider = getWsProvider(WS_ENDPOINT)
const clientInstance = createClient(provider)
```

### Migration Path

Replace `getWsProvider` → `getSmProvider`:

| Current | New |
|---------|-----|
| `polkadot-api/ws-provider/web` | `polkadot-api/sm-provider` |
| `getWsProvider(WS_ENDPOINT)` | `getSmProvider(chain)` |
| Environment variable endpoint | Static chain spec JSON |

### Files to Modify

1. `apps/frontend/src/hooks/useApi.ts` - Replace with smoldot
2. `apps/frontend/next.config.js` - Add worker support if needed
3. New: `apps/frontend/src/lib/chainspec.json` - Chain spec file

### Files to Delete (Post-Migration)

- Any `WS_ENDPOINT` environment variable references
- Fallback connection logic (none currently exists)

---

## 7. Chain Spec Generation Script

Create script for exporting chain spec:

```bash
#!/bin/bash
# apps/blockchain/scripts/export-chainspec.sh

set -e

SCRIPT_DIR=$(dirname "$0")
BLOCKCHAIN_DIR="$SCRIPT_DIR/.."
FRONTEND_DIR="$BLOCKCHAIN_DIR/../frontend/src/lib"

# Build the node if needed
if [ ! -f "$BLOCKCHAIN_DIR/target/release/anarchy-node" ]; then
  echo "Building anarchy-node..."
  cd "$BLOCKCHAIN_DIR" && cargo build --release
fi

# Export chain spec
echo "Exporting chain spec..."
"$BLOCKCHAIN_DIR/target/release/anarchy-node" build-spec \
  --chain=dev \
  --raw \
  --disable-default-bootnode > "$FRONTEND_DIR/chainspec.json"

echo "Chain spec exported to: $FRONTEND_DIR/chainspec.json"
echo "WARNING: You must add bootnode addresses to chainspec.json!"
```

---

## 8. Next.js Configuration Changes

### Required for Web Worker Support

```javascript
// next.config.js additions

const nextConfig = {
  // ... existing config
  
  webpack: (config, { isServer }) => {
    // ... existing config
    
    // Enable Web Workers
    if (!isServer) {
      config.output.workerPublicPath = '_next/static/workers/'
    }
    
    return config
  },
  
  // Optional: If using SharedArrayBuffer (NOT required for smoldot)
  // async headers() {
  //   return [
  //     {
  //       source: '/:path*',
  //       headers: [
  //         { key: 'Cross-Origin-Opener-Policy', value: 'same-origin' },
  //         { key: 'Cross-Origin-Embedder-Policy', value: 'require-corp' },
  //       ],
  //     },
  //   ]
  // },
}
```

**Important**: COOP/COEP headers are NOT required for smoldot basic operation.

---

## 9. Summary and Recommendations

### Package Installation

```bash
cd apps/frontend
pnpm add smoldot
```

### Implementation Steps

1. Generate chain spec: `./apps/blockchain/scripts/export-chainspec.sh`
2. Add bootnode addresses to chain spec
3. Create `src/lib/smoldot-provider.ts` with Web Worker setup
4. Replace `useApi.ts` implementation with smoldot
5. Remove `WS_ENDPOINT` environment variable usage
6. Test with `pnpm dev:frontend`

### Testing Strategy

1. Unit test: Mock smoldot for fast tests
2. Integration test: Connect to local dev node
3. E2E test: Full flow with smoldot → blockchain

### Bundle Size Estimate

| Package | Size (gzipped) |
|---------|----------------|
| smoldot WASM | ~1.2MB |
| smoldot JS | ~50KB |
| Total additional | ~1.3MB |

Within the 2MB limit specified in NFR-003.

---

## References

- [PAPI Smoldot Provider Docs](https://papi.how/providers/sm)
- [npm: smoldot](https://www.npmjs.com/package/smoldot)
- [npm: @polkadot-api/smoldot](https://www.npmjs.com/package/@polkadot-api/smoldot)
- [npm: @polkadot-api/sm-provider](https://www.npmjs.com/package/@polkadot-api/sm-provider)
- [smoldot GitHub](https://github.com/smol-dot/smoldot)
- [MDN: SharedArrayBuffer](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/SharedArrayBuffer)
