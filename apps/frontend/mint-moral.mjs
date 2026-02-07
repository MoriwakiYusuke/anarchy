#!/usr/bin/env node
/**
 * 開発用: Sudo権限で$moralトークンをmintするスクリプト
 * apps/frontend ディレクトリから実行してください
 * 
 * 使用方法:
 *   node mint-moral.mjs <address> <amount>
 *   node mint-moral.mjs Alice 1000000
 */

import { createClient } from 'polkadot-api'
import { getWsProvider } from 'polkadot-api/ws-provider/node'
import { getPolkadotSigner } from 'polkadot-api/signer'
import { sr25519CreateDerive } from '@polkadot-labs/hdkd'
import { DEV_PHRASE, entropyToMiniSecret, mnemonicToEntropy } from '@polkadot-labs/hdkd-helpers'
import { encodeAddress } from '@polkadot/keyring'

const WS_ENDPOINT = process.env.WS_ENDPOINT || 'ws://127.0.0.1:9944'

// コマンドライン引数
const args = process.argv.slice(2)
if (args.length < 2) {
  console.log(`使用方法: node mint-moral.mjs <address|name> <amount>`)
  process.exit(1)
}

let targetAddress = args[0]
const amount = BigInt(args[1]) * BigInt(1_000_000_000_000) // 12桁精度

// Alice signer (AliceがSudo権限を持つ)
const entropy = mnemonicToEntropy(DEV_PHRASE)
const miniSecret = entropyToMiniSecret(entropy)
const derive = sr25519CreateDerive(miniSecret)
const aliceKeyPair = derive('//Alice')
const aliceSigner = getPolkadotSigner(aliceKeyPair.publicKey, 'Sr25519', aliceKeyPair.sign)

// テストアカウントの公開鍵を取得してSS58エンコード
function getAccountAddress(name) {
  const keyPair = derive(`//${name}`)
  // SS58フォーマットでエンコード (prefix 42 = Substrate)
  return encodeAddress(keyPair.publicKey, 42)
}

// アドレスを解決
let toAddress
if (targetAddress === 'Alice' || targetAddress === 'Bob' || targetAddress === 'Charlie' || 
    targetAddress === 'Dave' || targetAddress === 'Eve' || targetAddress === 'Ferdie') {
  toAddress = getAccountAddress(targetAddress)
} else {
  // すでにSS58アドレスと仮定
  toAddress = targetAddress
}

console.log(`Minting ${args[1]} MORAL to ${toAddress}...`)

async function main() {
  const client = createClient(getWsProvider(WS_ENDPOINT))
  
  // 動的にAPI取得
  const api = await client.getUnsafeApi()
  
  // Sudo.sudo(Moral.mint(target, amount))
  const mintCall = api.tx.Moral.mint({
    to: toAddress,
    amount: amount,
  }).decodedCall
  
  const tx = api.tx.Sudo.sudo({ call: mintCall })
  
  console.log('Sending transaction...')
  const result = await tx.signAndSubmit(aliceSigner)
  
  if (result.ok) {
    console.log('✅ Mint successful!')
    console.log(`Block: ${result.block?.index}`)
  } else {
    console.log('❌ Mint failed:', result.dispatchError)
  }
  
  client.destroy()
}

main().catch(err => {
  console.error('Error:', err)
  process.exit(1)
})
