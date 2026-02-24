/**
 * Connection state types for smoldot light client
 * @module types/connection
 */

/**
 * Connection status enum representing smoldot lifecycle states
 */
export type ConnectionStatus = 
  | 'initializing'  // smoldot起動中
  | 'syncing'       // チェーン同期中
  | 'connected'     // 接続完了、操作可能
  | 'error'         // エラー発生

/**
 * Connection state interface for useApi/useSmoldot hooks
 */
export interface ConnectionState {
  /** Current connection status */
  status: ConnectionStatus
  /** Latest block number (only available when connected) */
  blockNumber?: number
  /** Error message (only available when status is 'error') */
  errorMessage?: string
}

/**
 * Helper function to check if connection state indicates fully connected
 * @param state - ConnectionState to check
 * @returns true if status is 'connected'
 */
export function isConnected(state: ConnectionState): boolean {
  return state.status === 'connected'
}

/**
 * Helper function to check if connection state indicates syncing
 * @param state - ConnectionState to check
 * @returns true if status is 'syncing'
 */
export function isSyncing(state: ConnectionState): boolean {
  return state.status === 'syncing'
}

/**
 * Helper function to check if operations can be performed
 * Operations are only available when fully connected
 * @param state - ConnectionState to check
 * @returns true if operations are available
 */
export function canPerformOperations(state: ConnectionState): boolean {
  return state.status === 'connected'
}
