/**
 * DM (Direct Message, feature 019) frontend API のエントリポイント。
 *
 * Phase 2 時点では型 / 変換ユーティリティ / Zustand store のみ。
 * `sender.ts` / `scanner.ts` / `keyManager.ts` / `worker.ts` の実体は
 * Phase 3 (T038-T046) で追加する。
 */

export * from './types';
export { dmMetaFromString, dmMetaToString } from './api';
export { useDmStore } from './store';
