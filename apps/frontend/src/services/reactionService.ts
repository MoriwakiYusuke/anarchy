/**
 * Reaction Service
 * 
 * Handles reaction submission to the blockchain via PAPI.
 * This service communicates with pallet-reaction for submitting
 * PoW-verified reactions (Like/Boost/Bad) to posts.
 * 
 * Feature: 017-reaction-mining
 */

import type { PolkadotSigner } from 'polkadot-api/signer'

/** Timeout for RPC calls in milliseconds (30 seconds) */
const RPC_TIMEOUT_MS = 30_000

/**
 * Wrap a promise with timeout
 */
function withTimeout<T>(promise: Promise<T>, ms: number, message: string): Promise<T> {
  return Promise.race([
    promise,
    new Promise<T>((_, reject) => 
      setTimeout(() => reject(new Error(`Timeout: ${message}`)), ms)
    )
  ])
}

/**
 * Reaction types matching pallet-reaction's ReactionType enum
 */
export enum ReactionType {
  Like = 'Like',
  Boost = 'Boost',
  Bad = 'Bad',
}

/**
 * Reaction submission parameters
 */
export interface ReactionParams {
  postId: bigint
  reactionType: ReactionType
  nonce: bigint
  challengeBlock: number
}

/**
 * Reaction submission result
 */
export interface ReactionResult {
  success: boolean
  txHash?: string
  reward?: bigint
  error?: string
}

/**
 * Get the current challenge for reaction PoW mining
 * Challenge = blake2b(post_id ++ user_address ++ block_number)
 * 
 * @param client - PAPI client for RPC calls
 * @param unsafeApi - PAPI unsafe API for pallet queries
 * @param postId - Target post ID
 * @param userAddress - User's account address
 * @returns Challenge bytes and block number
 */
export async function getReactionChallenge(
  client: any,
  unsafeApi: any,
  _postId: bigint,  // Not used in challenge computation (kept for API compatibility)
  userAddress: string
): Promise<{ challenge: Uint8Array; blockNumber: number; difficulty: number }> {
  // Get finalized block info (same as faucet)
  const blockHash = await withTimeout(
    client._request('chain_getFinalizedHead', []) as Promise<string>,
    RPC_TIMEOUT_MS,
    'chain_getFinalizedHead'
  )
  if (!blockHash) {
    throw new Error('Failed to get finalized head')
  }
  
  const header = await withTimeout(
    client._request('chain_getHeader', [blockHash]) as Promise<{ number: string } | null>,
    RPC_TIMEOUT_MS,
    'chain_getHeader'
  )
  if (!header?.number) {
    throw new Error('Failed to get block header')
  }
  
  const blockNumber = parseInt(header.number, 16)
  
  // Get current difficulty from pallet-reaction
  const currentDifficulty = await withTimeout(
    unsafeApi.query.Reaction.CurrentDifficulty.getValue() as Promise<number>,
    RPC_TIMEOUT_MS,
    'Reaction.CurrentDifficulty query'
  ) ?? 16 // Default to BaseDifficulty
  
  // Compute challenge: blake2b(block_hash ++ account_bytes)
  // This matches the pallet's compute_challenge function in primitives_pow
  const { blake2b } = await import('blakejs')
  const { getSs58AddressInfo } = await import('@polkadot-api/substrate-bindings')
  
  // Convert block hash from hex string to bytes (remove 0x prefix)
  const blockHashBytes = new Uint8Array(
    (blockHash.slice(2).match(/.{2}/g) || []).map(byte => parseInt(byte, 16))
  )
  
  // Decode SS58 address to raw bytes (32 bytes public key)
  const addressInfo = getSs58AddressInfo(userAddress)
  if (!addressInfo.isValid) {
    throw new Error(`Invalid SS58 address: ${userAddress}`)
  }
  const accountBytes = addressInfo.publicKey
  
  // Concatenate: block_hash (32) + account (32) = 64 bytes
  const input = new Uint8Array(blockHashBytes.length + accountBytes.length)
  input.set(blockHashBytes, 0)
  input.set(accountBytes, blockHashBytes.length)
  
  const challenge = blake2b(input, undefined, 32)
  
  return {
    challenge: new Uint8Array(challenge),
    blockNumber,
    difficulty: currentDifficulty,
  }
}

/**
 * Submit a reaction transaction to the blockchain
 * 
 * @param unsafeApi - PAPI unsafe API for transaction submission
 * @param signer - Polkadot signer for signing the transaction
 * @param params - Reaction parameters including nonce from mining
 * @returns Submission result
 */
export async function submitReaction(
  unsafeApi: any,
  signer: PolkadotSigner,
  params: ReactionParams
): Promise<ReactionResult> {
  const { postId, reactionType, nonce, challengeBlock } = params
  
  console.log('[submitReaction] params:', {
    postId: postId?.toString(),
    reactionType,
    nonce: nonce?.toString(),
    challengeBlock,
  })
  
  try {
    // Map ReactionType to pallet enum variant
    const reactionVariant = {
      type: reactionType,
    }
    
    // Call pallet-reaction's react extrinsic
    // Signature: react(origin, post_id: u64, reaction_type: ReactionType, block_number: u32, nonce: u64, cpu_power: u64, stealth_recipient: Option<AccountId>)
    console.log('[submitReaction] building tx with:', {
      post_id: postId,
      reaction_type: reactionVariant,
      block_number: challengeBlock,
      nonce: nonce,
      cpu_power: BigInt(1000000),
    })
    
    const tx = unsafeApi.tx.Reaction.react({
      post_id: postId,
      reaction_type: reactionVariant,
      block_number: challengeBlock, // u32, pass as number
      nonce: nonce,
      cpu_power: BigInt(1000000),
      stealth_recipient: undefined,
    })
    
    // Sign and submit transaction
    const result = await tx.signAndSubmit(signer)
    console.log('[submitReaction] result:', result)
    
    // Check for dispatch error (faucet-style)
    // signAndSubmit may succeed but dispatch can fail with pallet errors
    if (result?.ok === false || result?.dispatchError) {
      console.error('[submitReaction] Transaction dispatch failed:', result)
      // Check if it's AlreadyReacted error
      const errorStr = JSON.stringify(result.dispatchError || result)
      if (errorStr.includes('AlreadyReacted') || errorStr.includes('Module')) {
        return { success: false, error: 'AlreadyReacted: You have already reacted to this post' }
      }
      return { success: false, error: 'Transaction failed: ' + errorStr }
    }
    
    // Extract events to find reward amount
    let reward: bigint | undefined
    for (const event of result.events || []) {
      if (event.type === 'Reaction' && event.value?.type === 'ReactionCreated') {
        reward = event.value.value.reward_paid
        break
      }
    }
    
    return {
      success: true,
      txHash: result.txHash,
      reward,
    }
  } catch (err) {
    const errorMessage = err instanceof Error ? err.message : String(err)
    console.error('[submitReaction] error:', errorMessage)
    
    // Map known pallet errors (faucet-style error detection)
    // AlreadyReacted is the most common error on reload
    if (
      errorMessage.includes('AlreadyReacted') ||
      errorMessage.includes('Invalid Transaction') ||
      errorMessage.includes('InvalidTransaction') ||
      errorMessage.includes('Custom(1)') ||
      errorMessage.includes('1010') ||
      errorMessage.includes('dispatch')
    ) {
      return { success: false, error: 'AlreadyReacted: You have already reacted to this post' }
    }
    if (errorMessage.includes('InvalidPoW') || errorMessage.includes('InvalidProof')) {
      return { success: false, error: 'InvalidPoW: Invalid proof of work - try mining again' }
    }
    if (errorMessage.includes('ChallengeExpired') || errorMessage.includes('Expired')) {
      return { success: false, error: 'ChallengeExpired: Challenge expired - please restart mining' }
    }
    
    return { success: false, error: errorMessage }
  }
}

/**
 * Get reaction statistics for a post
 * 
 * @param unsafeApi - PAPI unsafe API
 * @param postId - Target post ID
 * @returns Reaction counts (likes, boosts, bads)
 */
export async function getReactionStats(
  unsafeApi: any,
  postId: bigint
): Promise<{ likes: number; boosts: number; bads: number } | null> {
  try {
    const stats = await withTimeout(
      unsafeApi.query.Reaction.ReactionStatsStorage.getValue(postId) as Promise<{ likes: number; boosts: number; bads: number } | null>,
      RPC_TIMEOUT_MS,
      'Reaction.ReactionStatsStorage query'
    )
    
    if (!stats) {
      return { likes: 0, boosts: 0, bads: 0 }
    }
    
    return {
      likes: Number(stats.likes),
      boosts: Number(stats.boosts),
      bads: Number(stats.bads),
    }
  } catch {
    return null
  }
}

/**
 * Get the current reaction reward pool balance
 * 
 * @param unsafeApi - PAPI unsafe API
 * @returns Pool balance in planck units
 */
export async function getRewardPoolBalance(
  unsafeApi: any
): Promise<bigint> {
  const balance = await withTimeout(
    unsafeApi.query.Reaction.ReactionRewardPool.getValue() as Promise<bigint>,
    RPC_TIMEOUT_MS,
    'Reaction.ReactionRewardPool query'
  )
  return balance ?? BigInt(0)
}
