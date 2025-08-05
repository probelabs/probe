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

# Copy the entire build context (filtered by .dockerignore)
# This is simpler and more maintainable than selective copying
COPY . .

# Build the project in release mode (this will generate Cargo.lock if missing)
RUN cargo build --release

# ---- Runtime Stage ----
# Use distroless for minimal attack surface and smaller image
FROM gcr.io/distroless/cc-debian12

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

# Distroless images run as non-root by default and include CA certificates

# Copy the compiled binary from the builder (distroless uses /usr/local/bin)
COPY --from=builder /usr/src/probe/target/release/probe /usr/local/bin/probe

# Health check using the binary (distroless runs as non-root by default)
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD ["/usr/local/bin/probe", "--version"]

# Set the default command
ENTRYPOINT ["/usr/local/bin/probe"]