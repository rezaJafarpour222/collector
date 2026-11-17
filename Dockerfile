
FROM rust:1.100-slim-bookworm AS builder
WORKDIR /src
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config \
 && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && echo 'fn main() {}' > src/main.rs \
 && cargo build --release \
 && rm -rf src target/release/deps/Collector* target/release/Collector*
COPY src ./src
RUN cargo build --release && strip target/release/Collector

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --create-home --uid 10001 --shell /usr/sbin/nologin app
COPY --from=builder /src/target/release/Collector /usr/local/bin/Collector
RUN mkdir -p /tmp && chmod 1777 /tmp
USER app
WORKDIR /home/app
ENV RUST_LOG=info
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/Collector"]
