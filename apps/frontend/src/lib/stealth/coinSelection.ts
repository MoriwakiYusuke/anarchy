/**
 * Coin Selection Algorithm
 * 
 * T066 [US4] Implement coin selection algorithm
 * T067 [US4] Add linkability warning for multi-UTXO selection
 * 
 * ステルス残高から最適なUTXOを選択するアルゴリズム
 */

import { DetectedStealthBalance } from './types';

/**
 * 複数入力によるリンク可能性警告の閾値
 */
export const LINKABILITY_WARNING_THRESHOLD = 2;

/**
 * Coin selection options
 */
export interface CoinSelectionOptions {
  /** トランザクション手数料 */
  fee?: bigint;
  /** 特定のアドレスを優先的に選択 */
  preferAddresses?: string[];
}

/**
 * Coin selection result
 */
export interface SelectionResult {
  /** 選択されたバランス */
  selected: DetectedStealthBalance[];
  /** 選択された合計額 */
  total: bigint;
  /** お釣り */
  change: bigint;
  /** 十分な資金があるか */
  sufficient: boolean;
  /** リンク可能性警告（複数入力使用時） */
  linkabilityWarning: boolean;
}

/**
 * 指定額を満たす最適なUTXOを選択
 * 
 * 選択戦略:
 * 1. 使用済みコインを除外
 * 2. 単一コインで十分な場合は最小の適格コインを選択
 * 3. 複数コインが必要な場合は大きい順に選択
 * 
 * @param balances 利用可能な残高リスト
 * @param targetAmount 送金目標額
 * @param options 選択オプション
 * @returns 選択結果
 */
export function selectCoins(
  balances: DetectedStealthBalance[],
  targetAmount: bigint,
  options: CoinSelectionOptions = {}
): SelectionResult {
  const fee = options.fee ?? BigInt(0);
  const requiredAmount = targetAmount + fee;

  // 0額リクエスト
  if (targetAmount === BigInt(0)) {
    return {
      selected: [],
      total: BigInt(0),
      change: BigInt(0),
      sufficient: true,
      linkabilityWarning: false,
    };
  }

  // 使用済みを除外し、残高でソート
  const available = balances
    .filter(b => !b.spent)
    .sort((a, b) => {
      // bigintは直接比較できないのでNumber変換（大きな値では精度低下の可能性あり）
      const diff = a.balance - b.balance;
      if (diff < BigInt(0)) return -1;
      if (diff > BigInt(0)) return 1;
      return 0;
    });

  if (available.length === 0) {
    return {
      selected: [],
      total: BigInt(0),
      change: BigInt(0),
      sufficient: false,
      linkabilityWarning: false,
    };
  }

  // 単一コインで十分か確認（最小適格コイン戦略）
  const singleCoin = available.find(b => b.balance >= requiredAmount);
  if (singleCoin) {
    return {
      selected: [singleCoin],
      total: singleCoin.balance,
      change: singleCoin.balance - requiredAmount,
      sufficient: true,
      linkabilityWarning: false,
    };
  }

  // 複数コインが必要 - 大きい順に選択
  const sortedDesc = [...available].sort((a, b) => {
    const diff = b.balance - a.balance;
    if (diff < BigInt(0)) return -1;
    if (diff > BigInt(0)) return 1;
    return 0;
  });

  const selected: DetectedStealthBalance[] = [];
  let total = BigInt(0);

  for (const coin of sortedDesc) {
    selected.push(coin);
    total += coin.balance;
    if (total >= requiredAmount) {
      break;
    }
  }

  const sufficient = total >= requiredAmount;

  return {
    selected: sufficient ? selected : [],
    total: sufficient ? total : BigInt(0),
    change: sufficient ? total - requiredAmount : BigInt(0),
    sufficient,
    linkabilityWarning: selected.length >= LINKABILITY_WARNING_THRESHOLD,
  };
}

/**
 * 利用可能な合計残高を計算
 */
export function calculateTotalAvailable(balances: DetectedStealthBalance[]): bigint {
  return balances
    .filter(b => !b.spent)
    .reduce((sum, b) => sum + b.balance, BigInt(0));
}
