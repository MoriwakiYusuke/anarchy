/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
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
