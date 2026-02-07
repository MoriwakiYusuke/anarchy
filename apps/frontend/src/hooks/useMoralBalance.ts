'use client'

import { useState, useEffect, useCallback } from 'react'

interface UseMoralBalanceResult {
  balance: bigint | null
  isLoading: boolean
  error: string | null
  refetch: () => Promise<void>
}

export function useMoralBalance(
  unsafeApi: any,
  accountAddress: string | null,
  refreshTrigger?: number
): UseMoralBalanceResult {
  const [balance, setBalance] = useState<bigint | null>(null)
  const [isLoading, setIsLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const fetchBalance = useCallback(async () => {
    if (!unsafeApi || !accountAddress) {
      setBalance(null)
      return
    }

    setIsLoading(true)
    setError(null)

    try {
      // $moral = ネイティブトークン (System.Account.data.free)
      if (!unsafeApi.query?.System?.Account) {
        throw new Error('System pallet not found')
      }

      const result = await unsafeApi.query.System.Account.getValue(accountAddress)
      // result.data.free が利用可能残高
      setBalance(result?.data?.free ?? BigInt(0))
    } catch (err) {
      console.error('Failed to fetch balance:', err)
      setError(err instanceof Error ? err.message : '残高取得に失敗しました')
      setBalance(null)
    } finally {
      setIsLoading(false)
    }
  }, [unsafeApi, accountAddress])

  // 初回取得と依存変更時の再取得
  useEffect(() => {
    fetchBalance()
  }, [fetchBalance, refreshTrigger])

  // 定期的な更新（10秒ごと）
  useEffect(() => {
    if (!unsafeApi || !accountAddress) return

    const interval = setInterval(fetchBalance, 10000)
    return () => clearInterval(interval)
  }, [unsafeApi, accountAddress, fetchBalance])

  return { balance, isLoading, error, refetch: fetchBalance }
}

// 残高のフォーマット（1 moral = 1_000_000_000_000 units）
export function formatMoralBalance(balance: bigint | null): string {
  if (balance === null) return '-'
  
  // 12桁で割って小数点以下2桁まで表示
  const divisor = BigInt(1_000_000_000_000)
  const whole = balance / divisor
  const fraction = (balance % divisor) / BigInt(10_000_000_000) // 小数点以下2桁
  
  if (fraction === BigInt(0)) {
    return whole.toString()
  }
  return `${whole}.${fraction.toString().padStart(2, '0')}`
}
