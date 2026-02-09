/**
 * PoW Faucet Challenge Utilities
 * 
 * Blake2b-256ベースのProof of Workチャレンジ計算
 * パレット側の実装と同一のアルゴリズムを使用
 */

import { blake2b } from 'blakejs'

/**
 * チャレンジを計算: challenge = blake2b_256(block_hash || account_id_bytes)
 * 
 * @param blockHash - ブロックハッシュ (0x prefixed hex string, 32 bytes)
 * @param accountId - アカウントID (SS58 address as bytes, 32 bytes)
 * @returns challenge as Uint8Array (32 bytes)
 */
export function computeChallenge(blockHash: Uint8Array, accountId: Uint8Array): Uint8Array {
  // block_hash ++ account_id をハッシュ
  const input = new Uint8Array(blockHash.length + accountId.length)
  input.set(blockHash)
  input.set(accountId, blockHash.length)
  
  // Blake2b-256 (32 bytes output)
  return blake2b(input, undefined, 32)
}

/**
 * PoWハッシュを計算: hash = blake2b_256(challenge || nonce_le_bytes)
 * 
 * @param challenge - チャレンジハッシュ (32 bytes)
 * @param nonce - nonce値 (u64)
 * @returns hash as Uint8Array (32 bytes)
 */
export function computePoWHash(challenge: Uint8Array, nonce: bigint): Uint8Array {
  // nonce を little-endian 8 bytes に変換
  const nonceBytes = new Uint8Array(8)
  const view = new DataView(nonceBytes.buffer)
  view.setBigUint64(0, nonce, true) // little-endian
  
  // challenge ++ nonce_le_bytes をハッシュ
  const input = new Uint8Array(challenge.length + nonceBytes.length)
  input.set(challenge)
  input.set(nonceBytes, challenge.length)
  
  return blake2b(input, undefined, 32)
}

/**
 * ハッシュの先頭ゼロビット数をカウント
 * 
 * @param hash - ハッシュ値 (Uint8Array)
 * @returns 先頭のゼロビット数
 */
export function countLeadingZeroBits(hash: Uint8Array): number {
  let zeros = 0
  
  for (let i = 0; i < hash.length; i++) {
    const byte = hash[i]
    if (byte === 0) {
      zeros += 8
    } else {
      // 最後の非ゼロバイトの先頭ゼロビットをカウント
      // Math.clz32 は 32ビット整数の先頭ゼロをカウント
      // バイトを24ビット左シフトして上位8ビットに配置
      zeros += Math.clz32(byte) - 24
      break
    }
  }
  
  return zeros
}

/**
 * PoWが難易度を満たすか検証
 * 
 * @param challenge - チャレンジハッシュ (32 bytes)
 * @param nonce - nonce値 (u64)
 * @param difficulty - 必要な先頭ゼロビット数
 * @returns 難易度を満たしていればtrue
 */
export function verifyProof(challenge: Uint8Array, nonce: bigint, difficulty: number): boolean {
  const hash = computePoWHash(challenge, nonce)
  const leadingZeros = countLeadingZeroBits(hash)
  return leadingZeros >= difficulty
}

/**
 * Hex文字列をUint8Arrayに変換
 * 
 * @param hex - 0x prefixed hex string
 * @returns Uint8Array
 */
export function hexToBytes(hex: string): Uint8Array {
  const cleanHex = hex.startsWith('0x') ? hex.slice(2) : hex
  const bytes = new Uint8Array(cleanHex.length / 2)
  for (let i = 0; i < bytes.length; i++) {
    bytes[i] = parseInt(cleanHex.slice(i * 2, i * 2 + 2), 16)
  }
  return bytes
}

/**
 * Uint8ArrayをHex文字列に変換
 * 
 * @param bytes - Uint8Array
 * @returns 0x prefixed hex string
 */
export function bytesToHex(bytes: Uint8Array): string {
  const hex = Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('')
  return '0x' + hex
}
