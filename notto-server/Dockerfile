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
COPY notto-server/ ./notto-server/

# Minimal workspace that covers only server-side crates
RUN printf '[workspace]\nresolver = "3"\nmembers = ["shared", "notto-server"]\n' > Cargo.toml

RUN cargo build --release -p notto-server

# ── Runtime stage ─────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/notto-server /usr/local/bin/notto-server

EXPOSE 3000

CMD ["notto-server"]
