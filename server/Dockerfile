# ── Builder stage ────────────────────────────────────────────────────────────
FROM rust:1-slim AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy only what cargo needs to resolve the workspace, excluding the Tauri
# client so its heavy system dependencies are never pulled in.
COPY Cargo.lock ./
COPY shared/ ./shared/
COPY nooto-server/ ./nooto-server/

# Minimal workspace that covers only server-side crates
RUN printf '[workspace]\nresolver = "3"\nmembers = ["shared", "nooto-server"]\n' > Cargo.toml

RUN cargo build --release -p nooto-server

# ── Runtime stage ─────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/nooto-server /usr/local/bin/nooto-server

EXPOSE 3000

CMD ["nooto-server"]
