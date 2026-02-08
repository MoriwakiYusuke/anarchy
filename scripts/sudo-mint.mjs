#!/usr/bin/env node
/**
 * 開発用: Sudoで$moralトークンをmint（残高設定）するスクリプト
 * $moral = ネイティブトークン（pallet_balances）
 * 
 * 使用方法:
 *   node scripts/sudo-mint.mjs <address> <amount>
 *   node scripts/sudo-mint.mjs Alice 1000000
 *   node scripts/sudo-mint.mjs 5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY 10000
 * 
 * ※ Sudoを使用するため、Aliceの残高に関係なく任意のアドレスに残高を設定できます
 */

import { createClient } from 'polkadot-api'
import { getWsProvider } from 'polkadot-api/ws-provider/node'
import { getPolkadotSigner } from 'polkadot-api/signer'
import { sr25519CreateDerive } from '@polkadot-labs/hdkd'
import { DEV_PHRASE, entropyToMiniSecret, mnemonicToEntropy } from '@polkadot-labs/hdkd-helpers'

const WS_ENDPOINT = process.env.WS_ENDPOINT || 'ws://127.0.0.1:9944'

// コマンドライン引数
const args = process.argv.slice(2)
if (args.length < 2) {
  console.log(`
使用方法: node scripts/sudo-mint.mjs <address> <amount>

例:
  # Aliceに1,000,000 $moralをmint
  node scripts/sudo-mint.mjs Alice 1000000

  # 任意のアドレスにmint
  node scripts/sudo-mint.mjs 5HWA... 50000

※ Sudo権限を使用するため、Aliceの現在残高に関係なくmint可能です
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
  console.log(`\n=== $moral Token Mint (Sudo) ===`)
  console.log(`対象アドレス: ${targetAddress}`)
  console.log(`mint量: ${args[1]} $moral`)
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

  console.log('Sudo.sudo(Balances.force_set_balance) を実行中...')

  try {
    // Balances.force_set_balance をSudoで実行
    const forceSetBalanceCall = api.tx.Balances.force_set_balance({
      who: { type: 'Id', value: targetAddress },
      new_free: amount,
    })

    const sudoTx = api.tx.Sudo.sudo({
      call: forceSetBalanceCall.decodedCall,
    })

    console.log('トランザクション送信中...')
    const result = await sudoTx.signAndSubmit(signer)

    if (result.ok) {
      console.log(`\n✅ Mint成功!`)
      console.log(`   ブロック: #${result.block.number}`)
      console.log(`   ${args[1]} $moral を ${targetAddress.slice(0, 8)}... に設定しました`)
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
