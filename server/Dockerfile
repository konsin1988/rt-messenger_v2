# ============================================
# Base stage - shared between dev and prod
# ============================================
FROM rust:1.97.1-bookworm AS base
RUN apt-get update && apt-get install -y --no-install-recommends \
    protobuf-compiler \
    cmake \
    clang \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
RUN cargo install cargo-watch 2>/dev/null || true

# ============================================
# Dependencies stage - cache layer
# ============================================
FROM base AS dependencies
COPY src ./src
RUN mkdir -p src && echo "fn main() {}" > src/main.rs && \
    cargo build 2>/dev/null || true
RUN rm -rf src

# ============================================
# Development stage - hot reload
# ============================================
FROM base AS dev
COPY --from=dependencies /usr/local/cargo /usr/local/cargo
COPY . .
RUN cargo build
EXPOSE 8080
CMD ["cargo", "watch", "-w", "src", "-x", "run"]

# ============================================
# Build stage - compile release binary
# ============================================
FROM base AS build
COPY --from=dependencies /usr/local/cargo /usr/local/cargo
COPY . .
RUN cargo build --release

# ============================================
# Production stage - minimal image
# ============================================
FROM debian:bookworm-slim AS prod
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /app/target/release/messenger-server .
EXPOSE 8080
CMD ["./messenger-server"]
