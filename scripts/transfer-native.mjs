#!/usr/bin/env node
/**
 * 開発用: ネイティブトークン（トランザクション手数料用）を送金するスクリプト
 * 
 * 使用方法:
 *   node scripts/transfer-native.mjs <address> [amount]
 *   node scripts/transfer-native.mjs 5HWA137txyG9gXabtBQdSmFWcTHT7A7K5T4sJKCmKzS3aBLN 1000000
 * 
 * ※ Aliceアカウントから送金されます
 * ※ amountを省略すると1,000,000 Unit（10^6）が送金されます
 */

import { createClient } from 'polkadot-api'
import { getWsProvider } from 'polkadot-api/ws-provider/node'
import { getPolkadotSigner } from 'polkadot-api/signer'
import { sr25519CreateDerive } from '@polkadot-labs/hdkd'
import { DEV_PHRASE, entropyToMiniSecret, mnemonicToEntropy } from '@polkadot-labs/hdkd-helpers'

const WS_ENDPOINT = process.env.WS_ENDPOINT || 'ws://127.0.0.1:9944'
const DEFAULT_AMOUNT = 1_000_000_000_000_000n // 1,000 Unit (10^15, 12桁精度)

// コマンドライン引数
const args = process.argv.slice(2)
if (args.length < 1) {
  console.log(`
使用方法: node scripts/transfer-native.mjs <address> [amount]

例:
  # 1,000 Unitを送金（デフォルト）
  node scripts/transfer-native.mjs 5HWA137txyG9gXabtBQdSmFWcTHT7A7K5T4sJKCmKzS3aBLN

  # 10,000 Unitを送金
  node scripts/transfer-native.mjs 5HWA137txyG9gXabtBQdSmFWcTHT7A7K5T4sJKCmKzS3aBLN 10000

  # テストアカウント名でもOK
  node scripts/transfer-native.mjs Bob 1000

※ Aliceアカウントから送金されます
※ ネイティブトークンはトランザクション手数料の支払いに必要です
`)
  process.exit(1)
}

// テストアカウント名をアドレスに変換
const testAccounts = {
  'Alice': '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY',
  'Bob': '5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty',
  'Charlie': '5FLSigC9HGRKVhB9FiEo4Y3koPsNmBmLJbpXg2mp1hXcS59Y',
  'Dave': '5DAAnrj7VHTznn2AWBemMuyBwZWs6FNFjdyVXUeYum3PTXFy',
  'Eve': '5HGjWAeFDfFCWPsjFQdVV2Msvz2XtMktvgocEZcCj68kUMaw',
  'Ferdie': '5CiPPseXPECbkjWCa6MnjNokrgYjMqmKndv2rSnekmSK2DjL',
}

let targetAddress = args[0]
if (testAccounts[targetAddress]) {
  targetAddress = testAccounts[targetAddress]
}

const amount = args[1] ? BigInt(args[1]) * BigInt(1_000_000_000_000) : DEFAULT_AMOUNT

async function main() {
  console.log(`\n=== Native Token Transfer (開発用) ===`)
  console.log(`対象アドレス: ${targetAddress}`)
  console.log(`送金量: ${Number(amount / BigInt(1_000_000_000_000))} Unit`)
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
    const transferTx = api.tx.Balances.transfer_keep_alive({
      dest: { type: 'Id', value: targetAddress },
      value: amount,
    })

    console.log('トランザクション送信中...')
    const result = await transferTx.signAndSubmit(signer)

    if (result.ok) {
      console.log(`\n✅ 送金成功!`)
      console.log(`   対象: ${targetAddress.slice(0, 8)}...${targetAddress.slice(-4)}`)
      console.log(`   送金量: ${Number(amount / BigInt(1_000_000_000_000))} Unit`)
      console.log(`   ブロック: #${result.block.number}`)
      console.log(`\n   これでトランザクション手数料を支払えるようになりました。`)
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
