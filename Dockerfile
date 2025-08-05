# ---- Build Stage ----
FROM rust:1.84 AS builder

# Set build arguments for better caching
ARG CARGO_HOME=/usr/local/cargo
ARG RUSTUP_HOME=/usr/local/rustup
ARG VERSION=dev
ARG BUILD_DATE
ARG VCS_REF

# Set environment variables
ENV CARGO_HOME=${CARGO_HOME}
ENV RUSTUP_HOME=${RUSTUP_HOME}
ENV PATH=${CARGO_HOME}/bin:${RUSTUP_HOME}/bin:$PATH

WORKDIR /usr/src/probe

# Copy manifest files first for caching
COPY Cargo.toml Cargo.lock* ./
# Copy source code
COPY src ./src
# Note: benches/ directory not needed for production builds
# Benchmarks are only used during development with `cargo bench`

# Build the project in release mode (this will generate Cargo.lock if missing)
RUN cargo build --release

# ---- Runtime Stage ----
FROM debian:bookworm-slim

# Build arguments for metadata
ARG VERSION=dev
ARG BUILD_DATE
ARG VCS_REF

# Add security and metadata labels
LABEL maintainer="Probe Team" \
      description="Probe - Code search tool" \
      version="${VERSION}" \
      org.opencontainers.image.created="${BUILD_DATE}" \
      org.opencontainers.image.source="https://github.com/buger/probe" \
      org.opencontainers.image.revision="${VCS_REF}" \
      org.opencontainers.image.version="${VERSION}" \
      org.opencontainers.image.title="Probe" \
      org.opencontainers.image.description="AI-friendly code search tool built in Rust"

# Install CA certificates and curl for health checks
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean

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

# Add health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD /usr/local/bin/probe --version || exit 1

# Set the default command
ENTRYPOINT ["/usr/local/bin/probe"]