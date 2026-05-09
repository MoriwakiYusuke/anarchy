'use client'

import { useState, useEffect } from 'react'

// Runtime constants (12 decimals precision)
const DECIMALS = 12
const UNIT = BigInt(10 ** DECIMALS)

// フォールバック値（runtime設定と同期させる）
// ブロックチェーンから取得できない場合に使用
// TSTS v1: PostBaseCost=50 MORAL, PostByteCost=0.0008 MORAL/byte に更新済み
const FALLBACK_BASE_COST = 50      // 50 MORAL (TSTS v1)
const FALLBACK_BYTE_COST = 0.0008  // 0.0008 MORAL/byte (TSTS v1)
// raw units (12 decimals) を BigInt 整数リテラルで保持。
// `BigInt(0.0008 * 1e12)` は IEEE-754 rounding (例: 800000000.0000001) で例外を投げる可能性が
// あるため、最初から整数 bigint を直書きする (Copilot review #3199031111).
//   50 MORAL × 10^12 = 50_000_000_000_000
//   0.0008 MORAL × 10^12 = 800_000_000
const FALLBACK_BASE_COST_RAW: bigint = 50_000_000_000_000n
const FALLBACK_BYTE_COST_RAW: bigint = 800_000_000n

export interface PostCostConfig {
  baseCost: number      // 基本コスト (human readable)
  byteCost: number      // バイト単価 (human readable)
  baseCostRaw: bigint   // 基本コスト (raw units)
  byteCostRaw: bigint   // バイト単価 (raw units)
  /** TSTS P2: EIP-1559 base fee (動的, MORAL/byte). 0 なら base_fee 機能未稼働 */
  baseFee: number
  /** TSTS P2: base fee の raw 値 (units/byte) */
  baseFeeRaw: bigint
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
    baseCostRaw: FALLBACK_BASE_COST_RAW,
    byteCostRaw: FALLBACK_BYTE_COST_RAW,
    baseFee: 0,
    baseFeeRaw: 0n,
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
        // PAPI: Runtime constants から PostBaseCost と PostByteCost を取得
        const postConstants = unsafeApi.constants?.Post || unsafeApi.constants?.post
        
        if (!postConstants) {
          setConfig(prev => ({ ...prev, isLoading: false, isFromChain: false }))
          return
        }

        // PAPI では constants はゲッター関数として返される
        // 関数なら呼び出し、Promiseならawaitする
        const baseCostGetter = postConstants.PostBaseCost ?? postConstants.post_base_cost
        const byteCostGetter = postConstants.PostByteCost ?? postConstants.post_byte_cost

        if (baseCostGetter === undefined || byteCostGetter === undefined) {
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

        // BigInt に変換
        const baseCostBigInt = BigInt(baseCostRaw.toString())
        const byteCostBigInt = BigInt(byteCostRaw.toString())

        // Human readable に変換 (12 decimals)
        const baseCost = Number(baseCostBigInt) / Number(UNIT)
        const byteCost = Number(byteCostBigInt) / Number(UNIT)

        // TSTS P2: pallet_base_fee::BaseFee も同時取得 (失敗しても他は更新)
        let baseFeeRaw = 0n
        let baseFee = 0
        try {
          const baseFeeQuery = unsafeApi.query?.BaseFee?.BaseFee
            ?? unsafeApi.query?.baseFee?.baseFee
          if (baseFeeQuery) {
            const v = await baseFeeQuery.getValue()
            if (v != null) {
              baseFeeRaw = BigInt(v.toString())
              baseFee = Number(baseFeeRaw) / Number(UNIT)
            }
          }
        } catch {
          // base_fee 取得失敗は致命的でない (旧 chain では未実装) → 0 のまま
        }

        setConfig({
          baseCost,
          byteCost,
          baseCostRaw: baseCostBigInt,
          byteCostRaw: byteCostBigInt,
          baseFee,
          baseFeeRaw,
          isLoading: false,
          error: null,
          isFromChain: true,
        })
      } catch (err) {
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
 * バイト数から投稿コストを計算 (TSTS P2 base_fee 込み).
 *
 * 数式: total = baseCost + (byteCost + baseFee) × byteCount
 *
 * - `baseCost` (固定 50 MORAL): スパム抑止の固定費
 * - `byteCost` (固定 0.0008 MORAL/byte): storage tip 相当
 * - `baseFee` (動的, EIP-1559): 平常時 ~0、混雑時に指数的に上昇
 *
 * @param byteCount コンテンツのバイト数
 * @param config コスト設定
 * @returns 推定コスト (human readable, MORAL)
 */
export function calculatePostCost(byteCount: number, config: PostCostConfig): number {
  return config.baseCost + (config.byteCost + config.baseFee) * byteCount
}

/**
 * base_fee の混雑度を相対値 (0..1) で返す.
 * UI で「平常」「やや混雑」「非常に混雑」を表示するための補助.
 *
 * BaseFeeMin=1e-10, BaseFeeMax=1e-1 (MORAL/byte) を仮定.
 * log10 スケールで 0..1 にマップする.
 */
export function baseFeeCongestionLevel(baseFee: number): number {
  if (baseFee <= 0) return 0
  const minLog = Math.log10(1e-10)
  const maxLog = Math.log10(1e-1)
  const cur = Math.log10(Math.max(baseFee, 1e-12))
  return Math.max(0, Math.min(1, (cur - minLog) / (maxLog - minLog)))
}
