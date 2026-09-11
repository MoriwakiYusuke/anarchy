/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  // 静的 export (Cloudflare Workers Static Assets / 任意の静的ホスティング用)
  output: 'export',
  // export では Next.js の画像オプティマイザ (サーバ) が使えない。
  // このアプリの画像は実質すべて復号済み blob なので元々最適化対象外。
  images: { unoptimized: true },
  // 開発用テストアカウント (//Alice 等) の入口を出すかどうか。ここで '0'/'1' の定数に
  // 正規化してから DefinePlugin に渡すことで、未設定時も `process.env.X === '1'` が
  // ビルド時に false へ畳まれ、dev ログイン UI / DEV_PHRASE 派生が bundle から消える。
  // `pnpm dev` は .env.development で 1、`next build` は明示しない限り 0。
  env: {
    NEXT_PUBLIC_ENABLE_DEV_ACCOUNTS: process.env.NEXT_PUBLIC_ENABLE_DEV_ACCOUNTS === '1' ? '1' : '0',
  },
  // Transpile polkadot packages to handle WASM and SSR issues
  transpilePackages: [
    'anarchy-wasm-engine',
    '@polkadot/keyring',
    '@polkadot/networks',
    '@polkadot/rpc-augment',
    '@polkadot/rpc-core',
    '@polkadot/rpc-provider',
    '@polkadot/types',
    '@polkadot/types-augment',
    '@polkadot/types-codec',
    '@polkadot/types-create',
    '@polkadot/types-known',
    '@polkadot/util',
    '@polkadot/util-crypto',
    '@polkadot/wasm-bridge',
    '@polkadot/wasm-crypto',
    '@polkadot/wasm-crypto-asmjs',
    '@polkadot/wasm-crypto-init',
    '@polkadot/wasm-crypto-wasm',
    '@polkadot/wasm-util',
    '@polkadot/x-bigint',
    '@polkadot/x-global',
    '@polkadot/x-randomvalues',
    '@polkadot/x-textdecoder',
    '@polkadot/x-textencoder',
  ],
  webpack: (config, { isServer }) => {
    // Handle WASM files
    config.experiments = {
      ...config.experiments,
      asyncWebAssembly: true,
    };
    
    // Required for polkadot packages
    if (!isServer) {
      config.resolve.fallback = {
        ...config.resolve.fallback,
        fs: false,
        net: false,
        tls: false,
        crypto: false,
      };
    }
    
    return config;
  },
  // Empty turbopack config to silence Next.js 16 warning
  turbopack: {},
}

module.exports = nextConfig
