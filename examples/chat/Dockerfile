# Build stage
FROM node:20.12.2-slim AS builder

# Set build arguments for better caching
ARG NODE_ENV=production
ARG VERSION=dev
ARG BUILD_DATE
ARG VCS_REF

# Set environment variables
ENV NODE_ENV=${NODE_ENV}

WORKDIR /app

# Copy package files first for better layer caching
COPY package.json package-lock.json* ./

# Install dependencies and clean cache
RUN npm ci --omit=dev && \
    npm cache clean --force

# Copy source code
COPY . ./

# Runtime stage
FROM node:20.12.2-slim

# Build arguments for metadata
ARG VERSION=dev
ARG BUILD_DATE
ARG VCS_REF

# Add security and metadata labels
LABEL maintainer="Probe Team" \
      description="Probe Chat - AI-powered chat interface" \
      version="${VERSION}" \
      org.opencontainers.image.created="${BUILD_DATE}" \
      org.opencontainers.image.source="https://github.com/buger/probe" \
      org.opencontainers.image.revision="${VCS_REF}" \
      org.opencontainers.image.version="${VERSION}" \
      org.opencontainers.image.title="Probe Chat" \
      org.opencontainers.image.description="AI-powered chat interface for code search and analysis"

# Install only essential runtime dependencies
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        && rm -rf /var/lib/apt/lists/* \
        && apt-get clean

# Create non-root user
RUN groupadd -r probe && \
    useradd -r -g probe -s /bin/bash -m probe

# Create app directory and set ownership
RUN mkdir -p /app && \
    chown -R probe:probe /app

WORKDIR /app

# Copy node_modules and source from builder
COPY --from=builder --chown=probe:probe /app/node_modules ./node_modules
COPY --from=builder --chown=probe:probe /app ./

# Switch to non-root user
USER probe

# Expose the default web port
EXPOSE 3000

# Health check - test the actual application
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:3000/health || node --version || exit 1

# Default: CLI mode. To run web mode, use: docker run ... probe-chat-app --web
ENTRYPOINT ["node", "index.js"]