# Stage 1: Build the Rust binary
FROM nvidia/cuda:12.2.2-devel-ubuntu22.04 AS builder

RUN apt-get update && apt-get install -y protobuf-compiler && rm -rf /var/lib/apt/lists/*

ENV DEBIAN_FRONTEND=noninteractive

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

# Build the ank-server
ARG FEATURES=""
RUN cargo build --release -p ank-server ${FEATURES:+--features $FEATURES}

# Stage 2: Runtime environment
FROM nvidia/cuda:12.2.2-runtime-ubuntu22.04

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y \
    libsqlite3-0 \
    libssl3 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Creamos el usuario, pero no intentamos hacer chown de todo el sistema
RUN useradd -m -s /bin/bash aegis

WORKDIR /app

# Creamos los directorios de datos y asignamos dueño solo a nuestras carpetas de trabajo
RUN mkdir -p /app/users /app/models && \
    chown -R aegis:aegis /app/users /app/models

# Copiamos el binario y le damos permisos
COPY --from=builder /app/target/release/ank-server /app/ank-server
RUN chown aegis:aegis /app/ank-server && chmod +x /app/ank-server

USER aegis

ENV RUST_LOG=info
EXPOSE 50051

CMD ["./ank-server"]