# Use distroless for minimal attack surface and smaller image
FROM gcr.io/distroless/cc-debian12

# Build arguments for metadata
ARG VERSION=dev
ARG BUILD_DATE
ARG VCS_REF
ARG TARGETARCH

# Add security and metadata labels
LABEL maintainer="Probe Team" \
      description="Probe - Code search tool" \
      version="${VERSION}" \
      org.opencontainers.image.created="${BUILD_DATE}" \
      org.opencontainers.image.source="https://github.com/probelabs/probe" \
      org.opencontainers.image.revision="${VCS_REF}" \
      org.opencontainers.image.version="${VERSION}" \
      org.opencontainers.image.title="Probe" \
      org.opencontainers.image.description="AI-friendly code search tool built in Rust"

# Distroless images run as non-root by default and include CA certificates

# Copy the pre-built binary based on target architecture
# TARGETARCH is automatically provided by Docker buildx (amd64, arm64)
COPY binaries/${TARGETARCH}/probe /usr/local/bin/probe

# Health check using the binary (distroless runs as non-root by default)
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD ["/usr/local/bin/probe", "--version"]

# Set the default command
ENTRYPOINT ["/usr/local/bin/probe"]