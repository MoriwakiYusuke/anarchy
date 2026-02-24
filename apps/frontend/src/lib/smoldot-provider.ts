/**
 * smoldot Light Client Provider for polkadot-api
 * @module lib/smoldot-provider
 * 
 * This module provides a singleton smoldot light client instance
 * that connects to the Anarchy blockchain via P2P network.
 */

import { start, type Client, type Chain } from 'smoldot'
import { getSmProvider } from 'polkadot-api/sm-provider'
import { createClient, PolkadotClient } from 'polkadot-api'
import chainSpecJson from './chainspec.json'

// Module-level singleton state
let smoldotClient: Client | null = null
let anarchyChain: Chain | null = null
let papiClient: PolkadotClient | null = null
let initPromise: Promise<PolkadotClient> | null = null

/**
 * Initialize the smoldot light client and connect to Anarchy chain
 * Returns a singleton PAPI client instance
 * 
 * @returns Promise resolving to PolkadotClient
 * @throws Error if initialization fails
 */
export async function initSmoldotClient(): Promise<PolkadotClient> {
  // Return existing client if already initialized
  if (papiClient) {
    return papiClient
  }
  
  // Return existing promise if initialization is in progress
  if (initPromise) {
    return initPromise
  }
  
  initPromise = (async () => {
    try {
      console.log('[smoldot] Initializing...')
      
      // Start smoldot light client
      // Note: In browser, this runs in main thread but is designed to be non-blocking
      // For production, consider Web Worker setup for better performance
      smoldotClient = start({
        forbidWs: false,
        forbidNonLocalWs: false,
        forbidWss: false,
      })
      
      console.log('[smoldot] Client started, adding chain...')
      
      // Add Anarchy chain with static chain spec
      // Note: addChain returns a Promise<Chain>, but getSmProvider accepts Promise directly
      const chainPromise = smoldotClient!.addChain({
        chainSpec: JSON.stringify(chainSpecJson)
      })
      
      // Create provider from chain promise (no need to await!)
      const provider = getSmProvider(chainPromise)
      
      // Create PAPI client
      papiClient = createClient(provider)
      
      // Store chain reference for cleanup
      anarchyChain = await chainPromise
      console.log(`[smoldot] Chain added: ${chainSpecJson.id}`)
      
      return papiClient
    } catch (error) {
      // Clean up on failure
      initPromise = null
      destroySmoldotClient()
      throw error
    }
  })()
  
  return initPromise
}

/**
 * Get the current smoldot PAPI client if initialized
 * 
 * @returns PolkadotClient or null if not initialized
 */
export function getSmoldotClient(): PolkadotClient | null {
  return papiClient
}

/**
 * Check if smoldot is currently initialized
 * 
 * @returns true if smoldot client is available
 */
export function isSmoldotInitialized(): boolean {
  return papiClient !== null
}

/**
 * Destroy the smoldot client and release all resources
 * Should be called when the app unmounts
 */
export function destroySmoldotClient(): void {
  console.log('[smoldot] Destroying client...')
  
  if (papiClient) {
    try {
      papiClient.destroy()
    } catch (e) {
      console.warn('[smoldot] Error destroying PAPI client:', e)
    }
    papiClient = null
  }
  
  if (anarchyChain) {
    try {
      anarchyChain.remove()
    } catch (e) {
      console.warn('[smoldot] Error removing chain:', e)
    }
    anarchyChain = null
  }
  
  if (smoldotClient) {
    try {
      smoldotClient.terminate()
    } catch (e) {
      console.warn('[smoldot] Error terminating smoldot:', e)
    }
    smoldotClient = null
  }

  initPromise = null
  console.log('[smoldot] Client destroyed')
}

/**
 * Get connection state info for debugging
 * 
 * @returns Object with current connection state info
 */
export function getSmoldotDebugInfo(): {
  isInitialized: boolean
  hasChain: boolean
  hasClient: boolean
} {
  return {
    isInitialized: papiClient !== null,
    hasChain: anarchyChain !== null,
    hasClient: smoldotClient !== null
  }
}
