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
  postId: bigint,
  userAddress: string
): Promise<{ challenge: Uint8Array; blockNumber: number; difficulty: number }> {
  // Get finalized block info
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
  
  // Compute challenge: blake2b(post_id ++ user_address ++ block_number)
  // This matches the pallet's compute_challenge function
  const { blake2b } = await import('blakejs')
  const { getSs58AddressInfo } = await import('@polkadot-api/substrate-bindings')
  
  // Encode post_id as little-endian u64
  const postIdBytes = new Uint8Array(8)
  const postIdView = new DataView(postIdBytes.buffer)
  postIdView.setBigUint64(0, postId, true)
  
  // Decode SS58 address to raw bytes
  const addressInfo = getSs58AddressInfo(userAddress)
  if (!addressInfo.isValid) {
    throw new Error(`Invalid SS58 address: ${userAddress}`)
  }
  const accountBytes = addressInfo.publicKey
  
  // Encode block_number as little-endian u32
  const blockBytes = new Uint8Array(4)
  const blockView = new DataView(blockBytes.buffer)
  blockView.setUint32(0, blockNumber, true)
  
  // Concatenate: post_id (8) + account (32) + block (4) = 44 bytes
  const input = new Uint8Array(44)
  input.set(postIdBytes, 0)
  input.set(accountBytes, 8)
  input.set(blockBytes, 40)
  
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
  
  try {
    // Map ReactionType to pallet enum variant
    const reactionVariant = {
      type: reactionType,
    }
    
    // Call pallet-reaction's react extrinsic
    // react(origin, post_id: u64, reaction_type: ReactionType, nonce: u64, challenge_block: BlockNumber)
    const tx = unsafeApi.tx.Reaction.react({
      post_id: postId,
      reaction_type: reactionVariant,
      nonce: nonce,
      challenge_block: challengeBlock,
    })
    
    // Sign and submit transaction
    const result = await tx.signAndSubmit(signer)
    
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
    
    // Map known pallet errors
    if (errorMessage.includes('AlreadyReacted')) {
      return { success: false, error: 'You have already reacted to this post' }
    }
    if (errorMessage.includes('InvalidPoW')) {
      return { success: false, error: 'Invalid proof of work - try mining again' }
    }
    if (errorMessage.includes('ChallengeExpired')) {
      return { success: false, error: 'Challenge expired - please restart mining' }
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
