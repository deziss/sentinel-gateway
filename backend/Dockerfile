# =============================================================================
# Sentinel Gateway — Backend Multi-Stage Docker Build
# Stages: chef → planner → builder → runtime
# Uses cargo-chef for dependency caching (rebuilds only when Cargo.lock changes)
# =============================================================================

# ── Stage 1: Chef ─────────────────────────────────────────────────────────────
# Install cargo-chef for dependency caching
FROM rust:1.92-slim AS chef
RUN cargo install cargo-chef --locked
RUN apt-get update && apt-get install -y pkg-config libssl-dev protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app

# ── Stage 2: Planner ─────────────────────────────────────────────────────────
# Analyze dependencies and create a build plan (recipe.json)
FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo chef prepare --recipe-path recipe.json

# ── Stage 3: Builder ─────────────────────────────────────────────────────────
# Build dependencies first (cached layer), then build the application
FROM chef AS builder

# Cook dependencies from the recipe — this layer is cached unless Cargo.lock changes
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Copy full source and build the application
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY migrations ./migrations
COPY .env.example ./.env.example
COPY keys ./keys
COPY config ./config

RUN cargo build --release

# ── Stage 4: Runtime ─────────────────────────────────────────────────────────
# Use same base as builder for glibc compatibility
FROM rust:1.92-slim AS runtime
WORKDIR /app

# Install runtime dependencies only
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libssl3 curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN groupadd -r sentinel && useradd -r -g sentinel -d /app -s /sbin/nologin sentinel

# Copy artifacts from builder
COPY --from=builder /app/target/release/gateway-server /usr/local/bin/
COPY --from=builder /app/migrations /app/migrations
COPY --from=builder /app/config /app/config
COPY --from=builder /app/keys /app/keys
COPY --from=builder /app/.env.example /app/.env

# Create directories for runtime data (ensure permissions)
RUN chown -R sentinel:sentinel /app

USER sentinel

EXPOSE 8080

HEALTHCHECK --interval=10s --timeout=3s --start-period=15s --retries=3 \
    CMD curl -sf http://localhost:8080/healthz || exit 1

CMD ["gateway-server", "serve"]
