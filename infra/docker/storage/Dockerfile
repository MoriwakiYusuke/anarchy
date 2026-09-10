# syntax=docker/dockerfile:1
#
# anarchy-storage-node のコンテナイメージ。
# build context はリポジトリルート:
#   docker build -f infra/docker/storage/Dockerfile -t anarchy-storage-node .

# ---------- builder ----------
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

# storage-node は C++ 依存が無い (redb / libp2p / axum はすべて Rust) が、
# openssl (reqwest の default-tls) と bindgen 用の clang は必要。
RUN apt-get update && apt-get install -y --no-install-recommends \
        clang libclang-dev pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# RUSTUP_TOOLCHAIN でチャンネルを固定した場合、rust-toolchain.toml の
# components / targets は自動適用されないので明示的に追加する。
RUN rustup component add rust-src && rustup target add wasm32v1-none

WORKDIR /src
# kzg-constants は packages/ 配下の path 依存なので一緒に持ち込む必要がある
COPY packages/kzg-constants ./packages/kzg-constants
COPY apps/storage-node ./apps/storage-node
WORKDIR /src/apps/storage-node

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/apps/storage-node/target \
    cargo build --release --bin anarchy-storage-node \
    && cp target/release/anarchy-storage-node /usr/local/bin/anarchy-storage-node

# ---------- runtime ----------
FROM debian:bookworm-slim AS runtime

# torsocks: subxt (jsonrpsee WS) も reqwest も SOCKS プロキシを差す口が無いため、
# .onion のチェーンや他ストレージに出るには LD_PRELOAD で包む必要がある
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates libssl3 zlib1g libzstd1 torsocks \
    && rm -rf /var/lib/apt/lists/*

RUN printf 'TorAddress tor\nTorPort 9050\nAllowInbound 1\n' > /etc/tor/torsocks.conf

RUN useradd -r -u 10001 -m -d /data anarchy
COPY --from=builder /usr/local/bin/anarchy-storage-node /usr/local/bin/anarchy-storage-node

USER anarchy
WORKDIR /data
VOLUME ["/data"]

# http json-rpc / libp2p
EXPOSE 3030 4001

ENTRYPOINT ["/usr/local/bin/anarchy-storage-node"]
