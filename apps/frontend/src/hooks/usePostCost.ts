'use client'

import { useState, useEffect } from 'react'

// Runtime constants (12 decimals precision)
const DECIMALS = 12
const UNIT = BigInt(10 ** DECIMALS)

// フォールバック値（runtime設定と同期させる）
// ブロックチェーンから取得できない場合に使用
const FALLBACK_BASE_COST = 10  // 10 MORAL
const FALLBACK_BYTE_COST = 0.1 // 0.1 MORAL/byte

export interface PostCostConfig {
  baseCost: number      // 基本コスト (human readable)
  byteCost: number      // バイト単価 (human readable)
  baseCostRaw: bigint   // 基本コスト (raw units)
  byteCostRaw: bigint   // バイト単価 (raw units)
  isLoading: boolean
  error: string | null
  isFromChain: boolean  // チェーンから取得したかどうか
}

/**
 * ブロックチェーンから投稿コスト設定を動的に取得するフック
 * 仕様書「クレンジング・パラダイム」に従い、フロントエンドはプロトコルから設定を取得
 * 取得失敗時はフォールバック値を使用
 */
export function usePostCost(unsafeApi: any): PostCostConfig {
  const [config, setConfig] = useState<PostCostConfig>({
    baseCost: FALLBACK_BASE_COST,
    byteCost: FALLBACK_BYTE_COST,
    baseCostRaw: BigInt(FALLBACK_BASE_COST * Number(UNIT)),
    byteCostRaw: BigInt(FALLBACK_BYTE_COST * Number(UNIT)),
    isLoading: true,
    error: null,
    isFromChain: false,
  })

  useEffect(() => {
    if (!unsafeApi) {
      return
    }

    const fetchConstants = async () => {
      try {
        // デバッグ: 利用可能なパレットと定数を確認
        console.log('Available pallets in constants:', Object.keys(unsafeApi.constants || {}))
        
        // PAPI: Runtime constants から PostBaseCost と PostByteCost を取得
        const postConstants = unsafeApi.constants?.Post || unsafeApi.constants?.post
        
        if (!postConstants) {
          console.warn('Post pallet constants not found, using fallback values')
          console.log('Available constant pallets:', Object.keys(unsafeApi.constants || {}))
          setConfig(prev => ({ ...prev, isLoading: false, isFromChain: false }))
          return
        }

        // デバッグ: Post パレットで利用可能な定数
        console.log('Available constants in Post pallet:', Object.keys(postConstants))

        // PAPI では constants はゲッター関数として返される
        // 関数なら呼び出し、Promiseならawaitする
        const baseCostGetter = postConstants.PostBaseCost ?? postConstants.post_base_cost
        const byteCostGetter = postConstants.PostByteCost ?? postConstants.post_byte_cost

        if (baseCostGetter === undefined || byteCostGetter === undefined) {
          console.warn('PostBaseCost or PostByteCost not found, using fallback values')
          console.log('Available keys:', Object.keys(postConstants))
          setConfig(prev => ({ ...prev, isLoading: false, isFromChain: false }))
          return
        }

        // PAPI constants: 関数を呼び出して値を取得
        // 引数なしで呼び出すとPromiseが返る
        const baseCostRaw = typeof baseCostGetter === 'function' 
          ? await baseCostGetter() 
          : baseCostGetter
        const byteCostRaw = typeof byteCostGetter === 'function' 
          ? await byteCostGetter() 
          : byteCostGetter

        console.log('Raw constants from chain:', { baseCostRaw, byteCostRaw })

        // BigInt に変換
        const baseCostBigInt = BigInt(baseCostRaw.toString())
        const byteCostBigInt = BigInt(byteCostRaw.toString())

        // Human readable に変換 (12 decimals)
        const baseCost = Number(baseCostBigInt) / Number(UNIT)
        const byteCost = Number(byteCostBigInt) / Number(UNIT)

        console.log('Fetched post costs from chain:', { baseCost, byteCost })

        setConfig({
          baseCost,
          byteCost,
          baseCostRaw: baseCostBigInt,
          byteCostRaw: byteCostBigInt,
          isLoading: false,
          error: null,
          isFromChain: true,
        })
      } catch (err) {
        console.error('Failed to fetch post cost constants:', err)
        // フォールバック値を使用
        setConfig(prev => ({
          ...prev,
          isLoading: false,
          error: null, // エラーを隠してフォールバック値を使用
          isFromChain: false,
        }))
      }
    }

    fetchConstants()
  }, [unsafeApi])

  return config
}

/**
 * バイト数から投稿コストを計算
 * @param byteCount コンテンツのバイト数
 * @param config コスト設定
 * @returns 推定コスト (human readable)
 */
export function calculatePostCost(byteCount: number, config: PostCostConfig): number {
  return config.baseCost + config.byteCost * byteCount
}
