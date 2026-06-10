/**
 * StealthModal.handleSpend — multi-UTXO 分割送金のユニットテスト
 * (refactor/full-code-review の CRITICAL fix 回帰防止)。
 *
 * 旧バグ: 選択した UTXO ごとに「要求額の全額」を送っていたため、
 * 5/5/5 MORAL の UTXO 3 つで 8 MORAL を送ると 8+8+8=24 MORAL の
 * transfer を試みていた (残高不足で部分失敗 or 過剰送金)。
 *
 * 修正後: 要求額を選択 UTXO に分割し、各 UTXO からは
 * min(残り必要額, その UTXO の残高) のみ送り、残りが 0 になったら打ち切る。
 *
 * 本テスト: 5/5/5 MORAL × 3 UTXO で 8 MORAL を送金 →
 *   - transfer は 2 回のみ (5 MORAL と 3 MORAL)。3 つ目の UTXO は触らない。
 *   - 合計送金額 = ちょうど 8 MORAL。
 *   - 成功メッセージに実送金額 8.0000 が出る。
 *
 * handleSpend は StealthModal 内部の useCallback なので、子コンポーネント
 * (StealthBalanceList / StealthSpendForm) をスタブ化して UI 経由で実コードを
 * 駆動する。チェーン API / wasm / stealth signer はモック。
 */

import React from 'react'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'
import { StealthModal } from '@/components/stealth/StealthModal'
import { deriveKeyFromBalance, createStealthSigner } from '@/lib/stealth/signer'

const MORAL = BigInt(1_000_000_000_000) // 12 decimals

// ---- mocks ----------------------------------------------------------------

// wasm: 鍵整合性チェック (parse_meta_address) のみ通ればよい
jest.mock('anarchy-wasm-engine', () => ({
  parse_meta_address: jest.fn(() => ({
    spend_pubkey: new Array(32).fill(0),
    view_pubkey: new Array(32).fill(0),
  })),
}))

jest.mock('@/lib/stealth/keyManager', () => ({
  stealthKeyManager: {
    getKeyPair: jest.fn(() => ({
      metaAddress: 'st:anarchy:testmeta',
      spendKey: new Uint8Array(32),
      viewKey: new Uint8Array(32),
      spendPubkey: new Uint8Array(32),
      viewPubkey: new Uint8Array(32),
    })),
    getMetaAddress: jest.fn(() => 'st:anarchy:testmeta'),
    destroy: jest.fn(),
  },
}))

jest.mock('@/lib/stealth/signer', () => ({
  deriveKeyFromBalance: jest.fn(async () => new Uint8Array(32)),
  createStealthSigner: jest.fn(async () => ({
    getPolkadotSigner: async () => ({ mock: 'stealth-signer' }),
    destroy: jest.fn(),
  })),
}))

jest.mock('@/lib/stealth/api', () => ({
  sendToStealth: jest.fn(),
}))

// Scanner: 5 MORAL の owned UTXO を 3 件返す
jest.mock('@/lib/stealth/scanner', () => ({
  StealthScanner: jest.fn().mockImplementation(() => ({
    scanBlockRange: jest.fn(async () => [
      { isOwned: true, stealthAddress: 'st-utxo-1', blockNumber: 1, ephemeralPubkey: new Uint8Array(32) },
      { isOwned: true, stealthAddress: 'st-utxo-2', blockNumber: 2, ephemeralPubkey: new Uint8Array(32) },
      { isOwned: true, stealthAddress: 'st-utxo-3', blockNumber: 3, ephemeralPubkey: new Uint8Array(32) },
    ]),
    stop: jest.fn(),
  })),
}))

// 子コンポーネントは spend フローに必要な最小スタブに差し替える
jest.mock('@/components/stealth/StealthAddressGenerator', () => ({
  StealthAddressGenerator: () => null,
}))
jest.mock('@/components/stealth/BackupImportDialog', () => ({
  BackupImportDialog: () => null,
}))
jest.mock('@/components/stealth/StealthSendForm', () => ({
  StealthSendForm: () => null,
}))
jest.mock('@/components/stealth/StealthBalanceList', () => ({
  __esModule: true,
  default: ({ onStartScan }: { onStartScan: () => void }) => (
    <button type="button" data-testid="trigger-scan" onClick={() => void onStartScan()}>
      scan
    </button>
  ),
}))
jest.mock('@/components/stealth/StealthSpendForm', () => ({
  StealthSpendForm: ({
    balances,
    onSpend,
    successMessage,
    errorMessage,
  }: {
    balances: Array<{ stealthAddress: string; balance: bigint }>
    onSpend: (values: unknown) => Promise<void>
    successMessage: string | null
    errorMessage: string | null
  }) => (
    <div>
      <button
        type="button"
        data-testid="trigger-spend"
        onClick={() =>
          void onSpend({
            // 検出された 3 UTXO 全部を選択して 8 MORAL 送る
            selectedBalances: balances,
            recipientAddress: 'RECIPIENT_ADDR',
            amount: BigInt(8_000_000_000_000),
          }).catch(() => undefined)
        }
      >
        spend
      </button>
      {successMessage && <p data-testid="spend-success">{successMessage}</p>}
      {errorMessage && <p data-testid="spend-error">{errorMessage}</p>}
    </div>
  ),
}))

jest.mock('@/i18n/context', () => ({
  useLocale: () => ({
    t: (key: string, params?: Record<string, string | number>) =>
      params ? `${key} ${Object.values(params).join(' ')}` : key,
  }),
}))

// ---- test -----------------------------------------------------------------

describe('StealthModal multi-UTXO spend split', () => {
  it('splits 8 MORAL across 5/5/5 UTXOs as 5 + 3 (third UTXO untouched)', async () => {
    const signAndSubmit = jest.fn().mockResolvedValue({ ok: true })
    const transferAllowDeath = jest.fn(() => ({ signAndSubmit }))
    const unsafeApi = {
      query: {
        System: {
          Account: {
            // scan 後の残高取得: どの UTXO も 5 MORAL
            getValue: jest.fn(async () => ({ data: { free: BigInt(5) * MORAL } })),
          },
        },
      },
      tx: {
        Balances: {
          transfer_allow_death: transferAllowDeath,
        },
      },
    }

    render(
      <StealthModal
        isOpen
        onClose={() => undefined}
        unsafeApi={unsafeApi}
        signer={{} as never}
        accountAddress="5TestAccount"
        isConnected
        blockNumber={100}
      />,
    )

    // Balance タブへ (t() スタブはキーをそのまま返す)
    fireEvent.click(screen.getByText('stealth.balance'))

    // スキャン実行 → balanceStore に 5 MORAL × 3 が入る
    fireEvent.click(screen.getByTestId('trigger-scan'))
    const sendBalanceBtn = await screen.findByText('stealth.spend.sendBalance')

    // Spend フォームを開いて 8 MORAL 送金を発火
    fireEvent.click(sendBalanceBtn)
    fireEvent.click(await screen.findByTestId('trigger-spend'))

    // 成功メッセージ = 実送金合計 8.0000 MORAL
    await waitFor(() => {
      expect(screen.getByTestId('spend-success')).toHaveTextContent(
        'stealth.spend.success 8.0000',
      )
    })

    // transfer は 2 回のみ: 5 MORAL → 3 MORAL。3 件目の UTXO には触らない。
    expect(transferAllowDeath).toHaveBeenCalledTimes(2)
    expect(transferAllowDeath).toHaveBeenNthCalledWith(1, {
      dest: { type: 'Id', value: 'RECIPIENT_ADDR' },
      value: BigInt(5) * MORAL,
    })
    expect(transferAllowDeath).toHaveBeenNthCalledWith(2, {
      dest: { type: 'Id', value: 'RECIPIENT_ADDR' },
      value: BigInt(3) * MORAL,
    })
    expect(signAndSubmit).toHaveBeenCalledTimes(2)

    // 合計はちょうど 8 MORAL (過剰送金していない)
    const total = transferAllowDeath.mock.calls.reduce(
      (sum: bigint, call: unknown[]) => sum + (call[0] as { value: bigint }).value,
      BigInt(0),
    )
    expect(total).toBe(BigInt(8) * MORAL)

    // 署名鍵 derive も 2 UTXO 分のみ (3 件目は untouched の傍証)
    expect(deriveKeyFromBalance).toHaveBeenCalledTimes(2)
    expect(createStealthSigner).toHaveBeenCalledTimes(2)
  })
})
