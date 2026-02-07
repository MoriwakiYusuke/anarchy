#!/usr/bin/env node
/**
 * 開発用: シードフレーズからAccountIdを導出し、$moralトークンをmintするスクリプト
 * 
 * 使用方法:
 *   node scripts/mint-moral-seed.mjs <seed-phrase> [amount]
 *   node scripts/mint-moral-seed.mjs "word1 word2 word3 ... word12" 10000
 * 
 * ※ Aliceアカウント（Sudo権限）で実行されます
 * ※ amountを省略すると10,000 moralがmintされます
 */

import { createClient } from 'polkadot-api'
import { getWsProvider } from 'polkadot-api/ws-provider/node'
import { getPolkadotSigner } from 'polkadot-api/signer'
import { sr25519CreateDerive } from '@polkadot-labs/hdkd'
import { DEV_PHRASE, entropyToMiniSecret, mnemonicToEntropy } from '@polkadot-labs/hdkd-helpers'
import { Keyring } from '@polkadot/keyring'
import { mnemonicValidate } from '@polkadot/util-crypto'
import { cryptoWaitReady } from '@polkadot/util-crypto'

const WS_ENDPOINT = process.env.WS_ENDPOINT || 'ws://127.0.0.1:9944'
const DEFAULT_AMOUNT = 10000

// コマンドライン引数
const args = process.argv.slice(2)
if (args.length < 1) {
  console.log(`
使用方法: node scripts/mint-moral-seed.mjs <seed-phrase> [amount]

例:
  # シードフレーズを指定して10,000 moralをmint（デフォルト）
  node scripts/mint-moral-seed.mjs "word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 word11 word12"

  # シードフレーズを指定して50,000 moralをmint
  node scripts/mint-moral-seed.mjs "word1 word2 ... word12" 50000

  # 開発用シードフレーズ（DEV_PHRASE）を使用
  node scripts/mint-moral-seed.mjs dev 10000

※ Aliceアカウント（Sudo権限）で実行されます
`)
  process.exit(1)
}

// シードフレーズ取得
let seedPhrase = args[0]
if (seedPhrase.toLowerCase() === 'dev') {
  seedPhrase = DEV_PHRASE
  console.log('開発用シードフレーズ(DEV_PHRASE)を使用します')
}

const amount = BigInt(args[1] || DEFAULT_AMOUNT) * BigInt(1_000_000_000_000) // 12桁精度

async function main() {
  // crypto初期化
  await cryptoWaitReady()

  // シードフレーズ検証
  if (!mnemonicValidate(seedPhrase)) {
    console.error('❌ 無効なシードフレーズです')
    process.exit(1)
  }

  // シードフレーズからAccountIdを導出
  const keyring = new Keyring({ type: 'sr25519' })
  const targetKeyPair = keyring.addFromUri(seedPhrase)
  const targetAddress = targetKeyPair.address

  console.log(`\n=== Moral Token Mint (シードフレーズ指定) ===`)
  console.log(`シードフレーズ: ${seedPhrase.split(' ').slice(0, 3).join(' ')}...（一部表示）`)
  console.log(`導出されたAccountId: ${targetAddress}`)
  console.log(`mint量: ${args[1] || DEFAULT_AMOUNT} moral`)
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

  console.log('Aliceアカウント（Sudo）で署名中...')

  try {
    // Sudo権限でMoral.mintを呼び出し
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
      console.log(`   AccountId: ${targetAddress}`)
      console.log(`   mint量: ${args[1] || DEFAULT_AMOUNT} moral`)
      console.log(`   ブロック: #${result.block.number}`)
      console.log(`   txHash: ${result.txHash}`)
    } else {
      console.error(`\n❌ Mint失敗`)
      console.error(result)
    }
  } catch (err) {
    console.error(`\n❌ エラー: ${err.message}`)
    if (err.message.includes('connect')) {
      console.error('   → ノードが起動しているか確認してください')
      console.error(`   → WS_ENDPOINT=${WS_ENDPOINT}`)
    }
  } finally {
    client.destroy()
    process.exit(0)
  }
}

main()
