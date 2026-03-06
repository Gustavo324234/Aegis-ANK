# Stage 1: Build the Rust binary
FROM nvidia/cuda:12.2.2-devel-ubuntu22.04 AS builder

ENV DEBIAN_FRONTEND=noninteractive

# Update system and install required build dependencies
RUN apt-get update && apt-get install -y \
    cmake \
    build-essential \
    libssl-dev \
    libsqlite3-dev \
    pkg-config \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Install Rust toolchain
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /app
COPY . .

# Build the ank-server explicitly for release
RUN cargo build --release -p ank-server

# Stage 2: Runtime environment
FROM nvidia/cuda:12.2.2-runtime-ubuntu22.04

ENV DEBIAN_FRONTEND=noninteractive

# Install runtime dependencies for SQLite and SSL
RUN apt-get update && apt-get install -y \
    libsqlite3-0 \
    libssl3 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create a non-root user for SRE Security
RUN useradd -m -s /bin/bash aegis

WORKDIR /app

# Prepare directories for volumes and ensure correct ownership
RUN mkdir -p /app/users /app/models && chown -R aegis:aegis /app

# Copy the build artifact from the builder stage
COPY --from=builder /app/target/release/ank-server /app/ank-server
RUN chown aegis:aegis /app/ank-server

# Drop privileges
USER aegis

ENV RUST_LOG=info
EXPOSE 50051

CMD ["./ank-server"]
