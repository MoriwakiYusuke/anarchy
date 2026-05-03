/**
 * 開発時のみ console に出すロガー。
 *
 * Why: `console.error('[dm]', err)` がエラー object をそのまま出力すると、
 * PAPI extrinsic 失敗時の error.message に SS58 / nonce / merkle root 等の
 * **ユーザー固有メタデータ** が含まれることがある。devtools 録画 / extension /
 * 他人が画面を覗き見たケースで匿名性 (CLAUDE.md Security Principle #1) を
 * 破壊しうる。
 *
 * 本ロガーは `process.env.NODE_ENV === 'production'` のとき完全 no-op となる。
 * Next.js は build 時に dead-code elimination するので production bundle から
 * `console.error(...)` 呼出自体が消える。
 *
 * 開発デバッグや E2E (`pnpm dev` / `next dev`) では従来通り console に流れる。
 *
 * ## 使い分け
 *
 * - **`debugError(label, err)`**: 失敗パスのログ。dev でのみ stack + 詳細を出す。
 * - **`debugWarn(label, ...args)`**: 想定範囲内の異常フローの注意。同じく dev only。
 * - **`debugLog(label, ...args)`**: 通常の進捗ログ。同じく dev only。
 *
 * production で本当に必要な可観測性 (= ユーザーに見せる error 通知) は
 * UI 側で表示するか、外部 telemetry に流す。console は使わない。
 */

const IS_DEV = process.env.NODE_ENV !== 'production';

export function debugLog(label: string, ...args: unknown[]): void {
  if (!IS_DEV) return;
  // eslint-disable-next-line no-console
  console.log(label, ...args);
}

export function debugWarn(label: string, ...args: unknown[]): void {
  if (!IS_DEV) return;
  // eslint-disable-next-line no-console
  console.warn(label, ...args);
}

export function debugError(label: string, ...args: unknown[]): void {
  if (!IS_DEV) return;
  // eslint-disable-next-line no-console
  console.error(label, ...args);
}
