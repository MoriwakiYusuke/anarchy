/**
 * devAccounts: 開発用テストアカウント (//Alice 等) のビルドフラグ。
 *
 * `NEXT_PUBLIC_ENABLE_DEV_ACCOUNTS=1` のビルドでのみ有効。next.config.js の `env` で
 * ビルド時に '0'/'1' の定数へ置換されるため、本番ビルド (未設定) では
 * `process.env.NEXT_PUBLIC_ENABLE_DEV_ACCOUNTS === '1'` が false に畳まれ、
 * その分岐は dead code として bundle から落ちる。`pnpm dev` は .env.development で有効化。
 *
 * **必ず `process.env.NEXT_PUBLIC_ENABLE_DEV_ACCOUNTS === '1'` を使う側でインラインに書くこと。**
 * ここで `export const ENABLED = ...` にして他モジュールから参照すると、Turbopack は
 * モジュール跨ぎで定数を畳まないため分岐が bundle に残る (実測済み)。
 * `isDevAccountsEnabled()` はテスト / 非 bundle 用途のヘルパで、JSX の条件には使わない。
 *
 * 注意: これは UI から dev 入口を消すだけ。//Alice 等は Substrate の公開既知鍵なので、
 * 本番 chainspec がこれらに残高 / sudo を与えていないことが本来の防御線。
 */
const DEV_PHRASE = 'bottom drive obey lake curtain smoke basket hold race lonely fit walk'

export function isDevAccountsEnabled(): boolean {
  return process.env.NEXT_PUBLIC_ENABLE_DEV_ACCOUNTS === '1'
}

/**
 * WalletConnect が保持する seed 文字列を Keyring の URI に解決する。
 * dev 有効時のみ `//Alice` 形式を DEV_PHRASE からの派生パスとして扱う。
 */
export function resolveSeedUri(seedPhrase: string): string {
  // インライン参照 (上記の理由)
  if (process.env.NEXT_PUBLIC_ENABLE_DEV_ACCOUNTS === '1' && seedPhrase.startsWith('//')) {
    return `${DEV_PHRASE}${seedPhrase}`
  }
  return seedPhrase
}
