
# ---------- Build stage ----------
FROM rust:1.82-slim-bookworm AS builder

WORKDIR /src

# OpenSSL headers needed by reqwest's default-tls
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
        pkg-config libssl-dev ca-certificates \
 && rm -rf /var/lib/apt/lists/*

# 1) Cache dependency compilation
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src \
 && echo 'fn main() {}' > src/main.rs \
 && cargo build --release \
 && rm -rf src target/release/deps/Collector* target/release/Collector*

# 2) Build the real binary
COPY src ./src
RUN cargo build --release \
 && strip target/release/Collector

# ---------- Runtime stage ----------
FROM debian:bookworm-slim

RUN apt-get update \
 && apt-get install -y --no-install-recommends \
        ca-certificates libssl3 \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --create-home --uid 10001 --shell /usr/sbin/nologin app

COPY --from=builder /src/target/release/Collector /usr/local/bin/Collector

USER app
WORKDIR /home/app

ENV RUST_LOG=info

ENTRYPOINT ["/usr/local/bin/Collector"]
