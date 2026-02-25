# Contract: Transfer API (Frontend Hook)

**Version**: 1.0.0  
**Date**: 2026-02-25  
**Type**: React Hook

## Overview

MORAL送金機能のフロントエンドインターフェース。PAPI経由で`Balances.transfer_allow_death`を実行する。

## Hook Interface

### useTransfer

```typescript
// apps/frontend/src/hooks/useTransfer.ts

export interface UseTransferOptions {
  onSuccess?: (txHash: string) => void
  onError?: (error: string) => void
}

export interface UseTransferResult {
  /** Current transfer state */
  state: TransferState
  
  /** Initiate transfer (shows confirmation dialog) */
  transfer: (recipient: string, amount: bigint) => void
  
  /** Confirm and submit the transfer */
  confirm: () => Promise<void>
  
  /** Cancel the pending transfer */
  cancel: () => void
  
  /** Reset state to idle */
  reset: () => void
  
  /** Validate recipient address */
  validateRecipient: (address: string) => ValidationResult
  
  /** Check if amount is valid */
  validateAmount: (amount: bigint, balance: bigint) => ValidationResult
}

export interface TransferState {
  status: 'idle' | 'confirming' | 'pending' | 'success' | 'error'
  recipient?: string
  amount?: bigint
  txHash?: string
  error?: string
}

export interface ValidationResult {
  valid: boolean
  error?: string  // i18n key
}

export function useTransfer(options?: UseTransferOptions): UseTransferResult
```

## Usage Example

```tsx
// components/TransferForm.tsx
import { useTransfer } from '@/hooks/useTransfer'
import { useBalance } from '@/hooks/useBalance'
import { useLocale } from '@/i18n'

function TransferForm({ signer }: { signer: PolkadotSigner }) {
  const { t } = useLocale()
  const { balance } = useBalance()
  const { state, transfer, confirm, cancel, validateRecipient, validateAmount } = useTransfer({
    onSuccess: (hash) => console.log('Transfer success:', hash),
    onError: (err) => console.error('Transfer failed:', err)
  })
  
  const [recipient, setRecipient] = useState('')
  const [amount, setAmount] = useState('')
  
  const recipientValid = validateRecipient(recipient)
  const amountBigint = parseAmount(amount)
  const amountValid = validateAmount(amountBigint, balance)
  
  const handleSubmit = () => {
    if (recipientValid.valid && amountValid.valid) {
      transfer(recipient, amountBigint)
    }
  }
  
  return (
    <form>
      <input 
        value={recipient} 
        onChange={e => setRecipient(e.target.value)}
        placeholder={t('transfer.recipientPlaceholder')}
      />
      {!recipientValid.valid && <span>{t(recipientValid.error!)}</span>}
      
      <input 
        value={amount}
        onChange={e => setAmount(e.target.value)}
        placeholder={t('transfer.amountPlaceholder')}
      />
      {!amountValid.valid && <span>{t(amountValid.error!)}</span>}
      
      <button onClick={handleSubmit}>{t('transfer.send')}</button>
      
      {/* Confirmation Dialog */}
      {state.status === 'confirming' && (
        <ConfirmDialog
          recipient={state.recipient}
          amount={state.amount}
          onConfirm={confirm}
          onCancel={cancel}
        />
      )}
      
      {/* Status display */}
      {state.status === 'pending' && <Spinner />}
      {state.status === 'success' && <SuccessMessage txHash={state.txHash} />}
      {state.status === 'error' && <ErrorMessage error={state.error} />}
    </form>
  )
}
```

## Blockchain API Contract

### Balances.transfer_allow_death

```typescript
// PAPI extrinsic call
const api = client.getUnsafeApi()

const tx = api.tx.Balances.transfer_allow_death({
  dest: { 
    type: 'Id', 
    value: recipientAccountId  // SS58 decoded to bytes
  },
  value: amountInPlanck  // bigint, 1 MORAL = 1_000_000_000_000n
})

// Sign and submit with watch for events
const subscription = tx.signSubmitAndWatch(signer)

for await (const status of subscription) {
  if (status.type === 'finalized') {
    // Transaction included in finalized block
    const txHash = status.txHash
    break
  }
  if (status.type === 'error') {
    throw new Error(status.error)
  }
}
```

## Validation Rules

### Recipient Address

| Rule | Error Key | Description |
|------|-----------|-------------|
| 非NULL | `transfer.invalidAddress` | 空でないこと |
| SS58形式 | `transfer.invalidAddress` | 有効なSS58エンコード |
| 自己送金不可 | `error.selfTransfer` | sender !== recipient |

### Amount

| Rule | Error Key | Description |
|------|-----------|-------------|
| > 0 | `transfer.invalidAmount` | 正の数 |
| <= balance | `transfer.insufficient` | 残高以下 |
| 数値形式 | `transfer.invalidAmount` | パース可能 |

## Error Handling

| Blockchain Error | UI Error Key | Description |
|------------------|--------------|-------------|
| `InsufficientBalance` | `transfer.insufficient` | 残高不足 |
| `DeadAccount` | `transfer.deadAccount` | 宛先がED未満 |
| Network timeout | `transfer.networkError` | 接続エラー |
| Other | `transfer.error` | 汎用エラー |

## i18n Keys

```typescript
type TransferKeys =
  | 'transfer.title'
  | 'transfer.recipient'
  | 'transfer.recipientPlaceholder'
  | 'transfer.amount'
  | 'transfer.amountPlaceholder'
  | 'transfer.balance'
  | 'transfer.invalidAddress'
  | 'transfer.invalidAmount'
  | 'transfer.insufficient'
  | 'transfer.send'
  | 'transfer.sending'
  | 'transfer.confirm'
  | 'transfer.confirmMessage'
  | 'transfer.success'
  | 'transfer.error'
  | 'transfer.cancel'
  | 'transfer.networkError'
```
