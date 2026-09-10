# syntax=docker/dockerfile:1
#
# anarchy-node (チェーンノード) のコンテナイメージ。
# build context はリポジトリルート:
#   docker build -f infra/docker/chain/Dockerfile -t anarchy-node .

# ---------- builder ----------
# runtime と同じ Debian bookworm でビルドし glibc を揃える。
# Rust は明示的にピン留めする。
# rust-toolchain.toml は channel = "stable" なので、放っておくと rustup が
# 最新 stable を引く。pinned な polkadot-sdk (stable2503) は新しい rustc の
# wasm リンク挙動に追従できず、ランタイム blob のリンクで sp_io のホスト関数が
# undefined symbol になる (ext_crypto_sr25519_verify_version_2 など)。
# RUSTUP_TOOLCHAIN は rust-toolchain.toml より優先されるため、これで固定できる。
ARG RUST_VERSION=1.93.1
FROM rust:${RUST_VERSION}-bookworm AS builder
ARG RUST_VERSION
ENV RUSTUP_TOOLCHAIN=${RUST_VERSION}

# cmake/clang: randomx-rs が RandomX (C++) を cmake でビルドし、bindgen が libclang を要求する
# protobuf-compiler: polkadot-sdk のビルドに protoc が必要
RUN apt-get update && apt-get install -y --no-install-recommends \
        cmake clang libclang-dev protobuf-compiler pkg-config libssl-dev git \
    && rm -rf /var/lib/apt/lists/*

# RUSTUP_TOOLCHAIN でチャンネルを固定した場合、rust-toolchain.toml の
# components / targets は自動適用されないので明示的に追加する。
RUN rustup component add rust-src && rustup target add wasm32v1-none

WORKDIR /src
# rust-toolchain.toml (stable + rust-src + wasm32v1-none) を先に置き、
# rustup にツールチェーンを解決させる
# kzg-constants は packages/ 配下の path 依存なので一緒に持ち込む必要がある
COPY packages/kzg-constants ./packages/kzg-constants
COPY apps/blockchain ./apps/blockchain
WORKDIR /src/apps/blockchain

# cache mount を使うので、成果物は同一 RUN 内でマウント外に退避する
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/apps/blockchain/target \
    cargo build --release --bin anarchy-node \
    && cp target/release/anarchy-node /usr/local/bin/anarchy-node

# ---------- runtime ----------
FROM debian:bookworm-slim AS runtime

# libstdc++6 は randomx-rs (C++) のリンク先。
# torsocks: reqwest に socks feature が無いため、.onion へ出るには LD_PRELOAD で包む必要がある
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates libssl3 libstdc++6 zlib1g libzstd1 torsocks \
    && rm -rf /var/lib/apt/lists/*

# tor は別コンテナで動かす前提。compose のサービス名で引く。
# 別構成にする場合はこのファイルを上書きマウントすること。
RUN printf 'TorAddress tor\nTorPort 9050\nAllowInbound 1\n' > /etc/tor/torsocks.conf

RUN useradd -r -u 10001 -m -d /data anarchy
COPY --from=builder /usr/local/bin/anarchy-node /usr/local/bin/anarchy-node

USER anarchy
WORKDIR /data
VOLUME ["/data"]

# p2p / rpc / prometheus
EXPOSE 30333 9944 9615

ENTRYPOINT ["/usr/local/bin/anarchy-node"]
