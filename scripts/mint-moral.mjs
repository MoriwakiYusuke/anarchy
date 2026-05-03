#!/usr/bin/env node
/**
 * 開発用: $moralトークンを送金するスクリプト
 * $moral = ネイティブトークン（pallet_balances）
 * 
 * 使用方法:
 *   node scripts/mint-moral.mjs <address> <amount>
 *   node scripts/mint-moral.mjs 5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY 10000
 * 
 * ※ Aliceアカウントから送金されます（Genesisで10,000 $moral保有）
 */

import { createClient } from 'polkadot-api'
import { getWsProvider } from 'polkadot-api/ws-provider/node'
import { getPolkadotSigner } from 'polkadot-api/signer'
import { sr25519CreateDerive } from '@polkadot-labs/hdkd'
import { DEV_PHRASE, entropyToMiniSecret, mnemonicToEntropy } from '@polkadot-labs/hdkd-helpers'
import { decodeAddress } from '@polkadot/util-crypto'

const WS_ENDPOINT = process.env.WS_ENDPOINT || 'ws://127.0.0.1:9944'

// コマンドライン引数
const args = process.argv.slice(2)
if (args.length < 2) {
  console.log(`
使用方法: node scripts/mint-moral.mjs <address> <amount>

例:
  # Bobに10,000 $moralを送金
  node scripts/mint-moral.mjs Bob 10000

  # テストアカウント名でもOK
  node scripts/mint-moral.mjs Alice 10000
  node scripts/mint-moral.mjs 5HWA... 5000

※ Alice（Genesisで$moral保有）から送金されます
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

// (#42-MED-1) SS58 アドレス検証 — 不正なアドレスでスクリプトを走らせて
// fund を Alice からブラックホールに飛ばす事故を防ぐ。decode に失敗したら exit。
try {
  decodeAddress(targetAddress)
} catch (err) {
  console.error(`❌ Invalid SS58 address: ${targetAddress}`)
  console.error(`   ${err.message ?? err}`)
  process.exit(1)
}

async function main() {
  console.log(`\n=== $moral Token Transfer (開発用) ===`)
  console.log(`対象アドレス: ${targetAddress}`)
  console.log(`送金量: ${args[1]} $moral`)
  console.log(`エンドポイント: ${WS_ENDPOINT}`)
  console.log('')

  // クライアント接続
  const provider = getWsProvider(WS_ENDPOINT)
  const client = createClient(provider)
  const api = client.getUnsafeApi()

  // Aliceの署名者を作成
  const entropy = mnemonicToEntropy(DEV_PHRASE)
  const miniSecret = entropyToMiniSecret(entropy)
  const derive = sr25519CreateDerive(miniSecret)
  const aliceKeyPair = derive('//Alice')
  
  const signer = getPolkadotSigner(
    aliceKeyPair.publicKey,
    'Sr25519',
    (input) => aliceKeyPair.sign(input)
  )

  console.log('Aliceアカウントから送金中...')

  try {
    // Balances.transfer_allow_death で$moralを送金
    const transferTx = api.tx.Balances.transfer_allow_death({
      dest: { type: 'Id', value: targetAddress },
      value: amount,
    })

    console.log('トランザクション送信中...')
    const result = await transferTx.signAndSubmit(signer)

    if (result.ok) {
      console.log(`\n✅ 送金成功!`)
      console.log(`   ブロック: #${result.block.number}`)
      console.log(`   ${args[1]} $moral を ${targetAddress.slice(0, 8)}... に送金しました`)
    } else {
      console.error(`\n❌ 送金失敗:`, result.dispatchError)
    }
  } catch (err) {
    console.error(`\n❌ エラー:`, err.message || err)
  }

  client.destroy()
  process.exit(0)
}

main().catch(console.error)
