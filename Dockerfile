# ---- Build Stage ----
FROM rust:latest AS builder

# Set build arguments for better caching
ARG CARGO_HOME=/usr/local/cargo
ARG RUSTUP_HOME=/usr/local/rustup

# Set environment variables
ENV CARGO_HOME=${CARGO_HOME}
ENV RUSTUP_HOME=${RUSTUP_HOME}
ENV PATH=${CARGO_HOME}/bin:${RUSTUP_HOME}/bin:$PATH

WORKDIR /usr/src/probe

# Copy manifest file first for caching
COPY Cargo.toml ./
# Copy source code
COPY src ./src
# Copy benches for benchmarks referenced in Cargo.toml
COPY benches ./benches

# Build the project in release mode (this will generate Cargo.lock if missing)
RUN cargo build --release

# ---- Runtime Stage ----
FROM debian:bookworm-slim

# Add security labels
LABEL maintainer="Probe Team" \
      description="Probe - Code search tool" \
      version="latest"

# Install CA certificates (for HTTPS support)
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN groupadd -r probe && \
    useradd -r -g probe -s /bin/bash -m probe

# Create directory and set ownership
RUN mkdir -p /app && \
    chown -R probe:probe /app

WORKDIR /app

# Copy the compiled binary from the builder
COPY --from=builder --chown=probe:probe /usr/src/probe/target/release/probe /usr/local/bin/probe

# Switch to non-root user
USER probe

# Set the default command
ENTRYPOINT ["/usr/local/bin/probe"]