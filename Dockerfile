FROM debian:bookworm AS builder

RUN apt-get update && apt-get install -y \
    build-essential \
    git \
    curl \
    pkg-config \
    libssl-dev \
    libclang-dev \
    && rm -rf /var/lib/apt/lists/*

# Get rustup and install the stable toolchain
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"
RUN rustup install 1.85.1

# install node
RUN curl -sL https://deb.nodesource.com/setup_23.x | bash 

# Install Node.js
RUN apt-get install -y nodejs

# Install PNPM
RUN corepack enable
RUN corepack prepare pnpm@latest --activate

COPY . /app
WORKDIR /app

# Build the project
RUN cargo build --release


ENV ALU_ROM_DIR=/roms
ENV ALU_DATABASE_URL="rocksdb:///data"
ENV RUST_LOG=info
ENV ALU_PRIMARY_REGION="US"
ENV ALU_PRIMARY_LANGUAGE="en"
ENV ALU_PROD_KEYS="/keys/prod.keys"
ENV ALU_TITLE_KEYS="/keys/title.keys"
ENV ALU_HOST="0.0.0.0:3000"

CMD ["./target/release/alumulemu"]

