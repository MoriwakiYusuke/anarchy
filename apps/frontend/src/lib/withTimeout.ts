/**
 * Promise タイムアウトの共有ユーティリティ。
 *
 * 旧実装は useTransfer / useFaucet / reactionService に同じものがコピペされており、
 * いずれも setTimeout を clear しないためタイマーがリークしていた
 * (240s タイマーが promise 解決後も生存し続ける)。本実装は finally で必ず clear する。
 *
 * タイムアウト時は `TimeoutError` を投げる。これは「操作が失敗した」のではなく
 * 「結果が不明 (status unknown) — チェーン側では完了している可能性がある」を意味する。
 * 呼び出し側は `err instanceof TimeoutError` か `message.includes('Timeout')` で
 * 通常の失敗と区別できる。
 */

export class TimeoutError extends Error {
  constructor(operation: string, ms: number) {
    // 既存の呼び出し側は `message.includes('Timeout')` でマッチするため
    // prefix は維持する。
    super(
      `Timeout: ${operation} (${ms}ms) — status unknown: the operation may still complete on-chain`,
    );
    this.name = 'TimeoutError';
  }
}

/**
 * promise を ms ミリ秒のタイムアウト付きで実行する。
 * タイムアウト時は TimeoutError で reject。タイマーは必ず解放される。
 */
export function withTimeout<T>(promise: Promise<T>, ms: number, operation: string): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<never>((_, reject) => {
    timer = setTimeout(() => reject(new TimeoutError(operation, ms)), ms);
  });
  return Promise.race([promise, timeout]).finally(() => {
    if (timer !== undefined) clearTimeout(timer);
  });
}
