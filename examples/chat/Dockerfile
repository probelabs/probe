# Build stage
FROM node:20.12.2-slim AS builder

# Set build arguments for better caching
ARG NODE_ENV=production

# Set environment variables
ENV NODE_ENV=${NODE_ENV}

WORKDIR /app

# Copy package files first for better layer caching
COPY package.json ./

# Install dependencies and clean cache
RUN npm install --omit=dev && \
    npm cache clean --force

# Copy source code
COPY . ./

# Runtime stage
FROM node:20.12.2-slim

# Add security labels
LABEL maintainer="Probe Team" \
      description="Probe Chat - AI-powered chat interface" \
      version="latest"

# Install only essential runtime dependencies
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        ca-certificates \
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

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD node --version || exit 1

# Default: CLI mode. To run web mode, use: docker run ... probe-chat-app --web
ENTRYPOINT ["node", "index.js"]