FROM debian:bookworm AS base
RUN apt-get update && apt-get install -y \
    build-essential \
    git \
    curl \
    pkg-config \
    libssl-dev \
    libclang-dev \
    && rm -rf /var/lib/apt/lists/*

FROM base AS rust-base
# Get rustup and install the stable toolchain
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"
RUN rustup default stable

FROM rust-base AS node-base
# Install Node.js
RUN curl -sL https://deb.nodesource.com/setup_23.x | bash
RUN apt-get update && apt-get install -y nodejs && rm -rf /var/lib/apt/lists/*
# Install PNPM
RUN corepack enable
RUN corepack prepare pnpm@latest --activate

FROM node-base AS build
WORKDIR /app

# Copy frontend dependencies first
COPY alu-panel/package.json alu-panel/pnpm-lock.yaml ./alu-panel/
WORKDIR /app/alu-panel
# Install dependencies
RUN --mount=type=cache,target=/root/.pnpm-store \
    pnpm install


WORKDIR /app
# Copy the rest of the source code
COPY . .

# Build the Rust project
RUN --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/root/.cargo/git \
    cargo build --release

# Frontend build
WORKDIR /app/alu-panel
RUN --mount=type=cache,target=/root/.pnpm-store \
    pnpm build

FROM base AS runtime
COPY --from=build /app/target/release/alumulemu /app/alumulemu
COPY --from=build /app/alu-panel/dist /app/alu-panel/dist
WORKDIR /app
ENV ALU_ROM_DIR=/roms
ENV ALU_DATABASE_URL="rocksdb:///data"
ENV RUST_LOG=info
ENV ALU_PRIMARY_REGION="US"
ENV ALU_PRIMARY_LANGUAGE="en"
ENV ALU_PROD_KEYS="/keys/prod.keys"
ENV ALU_TITLE_KEYS="/keys/title.keys"
ENV ALU_HOST="0.0.0.0:3000"
ENV ALU_CACHE_DIR="/var/cache/alumulemu"
EXPOSE 3000
CMD ["/app/alumulemu"]
