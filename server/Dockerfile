# ── Builder stage ────────────────────────────────────────────────────────────
FROM rust:1-slim AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

COPY shared/ ./shared/
COPY server/ ./server/

# Minimal workspace that covers only server-side crates
RUN printf '[workspace]\nresolver = "3"\nmembers = ["shared", "server"]\n' > Cargo.toml

RUN cargo build --release -p nooto-server

# ── Runtime stage ─────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/notto-server /usr/local/bin/nooto-server

EXPOSE 3000

CMD ["nooto-server"]
