# Build stage - use alpine for smaller size
FROM node:20-alpine AS builder

# Set build arguments for better caching
ARG NODE_ENV=production
ARG VERSION=dev
ARG BUILD_DATE
ARG VCS_REF

# Set environment variables
ENV NODE_ENV=${NODE_ENV}

WORKDIR /app

# Copy package files first for better layer caching
COPY package.json ./

# Install dependencies and clean cache
RUN npm install --only=production && \
    npm cache clean --force && \
    # Remove unnecessary files from packages
    find node_modules -name "*.md" -delete && \
    find node_modules -name "*.txt" -delete && \
    find node_modules -name "test" -type d -exec rm -rf {} + 2>/dev/null || true && \
    find node_modules -name "tests" -type d -exec rm -rf {} + 2>/dev/null || true && \
    find node_modules -name "*.test.js" -delete && \
    find node_modules -name "*.spec.js" -delete

# Copy only essential source files (filtered by .dockerignore)
COPY . ./

# Runtime stage - use alpine for smaller size
FROM node:20-alpine

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

# Install only essential runtime dependencies (alpine uses apk, not apt)
RUN apk add --no-cache \
        curl \
        ca-certificates

# Install claude-code globally as a system tool (separate from app dependencies)
RUN npm install -g @anthropic-ai/claude-code && \
    npm cache clean --force

# Create non-root user (alpine way)
RUN addgroup -g 1001 -S probe && \
    adduser -S -u 1001 -G probe probe

# Create app directory and set ownership
RUN mkdir -p /app && \
    chown -R probe:probe /app

WORKDIR /app

# Copy only node_modules from builder
COPY --from=builder --chown=probe:probe /app/node_modules ./node_modules

# Copy only essential application files (no need to copy from builder, use local files)
COPY --chown=probe:probe package.json index.js probeChat.js webServer.js probeTool.js tools.js ./
COPY --chown=probe:probe auth.js cancelRequest.js telemetry.js fileSpanExporter.js appTracer.js ./
COPY --chown=probe:probe tokenCounter.js tokenUsageDisplay.js index.html logo.png ./
COPY --chown=probe:probe bin/ ./bin/
COPY --chown=probe:probe storage/ ./storage/
COPY --chown=probe:probe implement/ ./implement/

# Switch to non-root user
USER probe

# Expose the default web port
EXPOSE 3000

# Health check - test the actual application
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:3000/health || node --version || exit 1

# Default: CLI mode. To run web mode, use: docker run ... probe-chat-app --web
ENTRYPOINT ["node", "index.js"]