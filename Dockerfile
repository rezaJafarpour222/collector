
# ---------- Build stage ----------
FROM rust:1.100-slim-bookworm AS builder

WORKDIR /src

# rustls needs no OpenSSL; only pkg-config for build scripts
RUN apt-get update \
 && apt-get install -y --no-install-recommends pkg-config \
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

# ca-certificates only — rustls uses the system CA bundle
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --create-home --uid 10001 --shell /usr/sbin/nologin app

COPY --from=builder /src/target/release/Collector /usr/local/bin/Collector

USER app
WORKDIR /home/app

ENV RUST_LOG=info

ENTRYPOINT ["/usr/local/bin/Collector"]
