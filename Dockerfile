FROM rustlang/rust:nightly-slim AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && echo "fn main() {}" > src/main.rs && cargo build --release
RUN rm -rf src

COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    && useradd -m -u 1000 blockkick \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/BlockKick .

RUN mkdir -p /app/logs /app/data \
    && chown -R blockkick:blockkick /app

USER blockkick

#API + P2P
EXPOSE 3000
EXPOSE 3001

# Healthcheck
HEALTHCHECK --interval=30s --timeout=10s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:3000/api/v1/chain || exit 1

CMD ["./BlockKick"]
