#!/usr/bin/env node
/**
 * 開発用: Sudo権限で$moralトークンをmintするスクリプト
 * 
 * 使用方法:
 *   node scripts/mint-moral.mjs <address> <amount>
 *   node scripts/mint-moral.mjs 5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY 10000
 * 
 * ※ Aliceアカウント（Sudo権限）で実行されます
 */

import { createClient } from 'polkadot-api'
import { getWsProvider } from 'polkadot-api/ws-provider/node'
import { getPolkadotSigner } from 'polkadot-api/signer'
import { sr25519CreateDerive } from '@polkadot-labs/hdkd'
import { DEV_PHRASE, entropyToMiniSecret, mnemonicToEntropy } from '@polkadot-labs/hdkd-helpers'
import { Keyring } from '@polkadot/keyring'

const WS_ENDPOINT = process.env.WS_ENDPOINT || 'ws://127.0.0.1:9944'

// コマンドライン引数
const args = process.argv.slice(2)
if (args.length < 2) {
  console.log(`
使用方法: node scripts/mint-moral.mjs <address> <amount>

例:
  # Aliceに10,000 moralをmint
  node scripts/mint-moral.mjs 5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY 10000

  # テストアカウント名でもOK
  node scripts/mint-moral.mjs Alice 10000
  node scripts/mint-moral.mjs Bob 5000

※ Aliceアカウント（Sudo権限）で実行されます
`)
  process.exit(1)
}

let targetAddress = args[0]
const amount = BigInt(args[1]) * BigInt(1_000_000_000_000) // 12桁精度

// テストアカウント名をアドレスに変換
const testAccounts = {
  'Alice': '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
  'Bob': '5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty',
  'Charlie': '5FLSigC9HGRKVhB9FiEo4Y3koPsNmBmLJbpXg2mp1hXcS59Y',
  'Dave': '5DAAnrj7VHTznn2AWBemMuyBwZWs6FNFjdyVXUeYum3PTXFy',
  'Eve': '5HGjWAeFDfFCWPsjFQdVV2Msvz2XtMktvgocEZcCj68kUMaw',
  'Ferdie': '5CiPPseXPECbkjWCa6MnjNokrgYjMqmKndv2rSnekmSK2DjL',
}

if (testAccounts[targetAddress]) {
  targetAddress = testAccounts[targetAddress]
}

async function main() {
  console.log(`\n=== Moral Token Mint (開発用) ===`)
  console.log(`対象アドレス: ${targetAddress}`)
  console.log(`mint量: ${args[1]} moral`)
  console.log(`エンドポイント: ${WS_ENDPOINT}`)
  console.log('')

  // クライアント接続
  const provider = getWsProvider(WS_ENDPOINT)
  const client = createClient(provider)
  const api = client.getUnsafeApi()

  // Aliceの署名者を作成（Sudo権限）
  const entropy = mnemonicToEntropy(DEV_PHRASE)
  const miniSecret = entropyToMiniSecret(entropy)
  const derive = sr25519CreateDerive(miniSecret)
  const aliceKeyPair = derive('//Alice')
  
  const signer = getPolkadotSigner(
    aliceKeyPair.publicKey,
    'Sr25519',
    (input) => aliceKeyPair.sign(input)
  )

  console.log('Aliceアカウントで署名中...')

  try {
    // Sudo権限でMoral.mintを呼び出し
    // sudo.sudo(Moral.mint(to, amount))
    const mintCall = api.tx.Moral.mint({
      to: targetAddress,
      amount: amount,
    })

    const sudoTx = api.tx.Sudo.sudo({
      call: mintCall.decodedCall,
    })

    console.log('トランザクション送信中...')
    const result = await sudoTx.signAndSubmit(signer)

    if (result.ok) {
      console.log(`\n✅ Mint成功!`)
      console.log(`   ブロック: #${result.block.number}`)
      console.log(`   ${args[1]} moral を ${targetAddress.slice(0, 8)}... にmintしました`)
    } else {
      console.error(`\n❌ Mint失敗:`, result.dispatchError)
    }
  } catch (err) {
    console.error(`\n❌ エラー:`, err.message || err)
  }

  client.destroy()
  process.exit(0)
}

main().catch(console.error)
