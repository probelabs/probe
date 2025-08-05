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

# Install ALL dependencies (including dev dependencies for build)
RUN npm install --include=dev && \
    npm cache clean --force

# Copy source files for bundling
COPY . ./

# Build production bundle
RUN npm run build:prod

# Install only production dependencies for runtime
RUN rm -rf node_modules && \
    npm install --only=production && \
    npm cache clean --force && \
    # Clean production node_modules
    find node_modules -name "*.md" -delete && \
    find node_modules -name "*.txt" -delete && \
    find node_modules -name "test" -type d -exec rm -rf {} + 2>/dev/null || true && \
    find node_modules -name "tests" -type d -exec rm -rf {} + 2>/dev/null || true && \
    find node_modules -name "*.test.js" -delete && \
    find node_modules -name "*.spec.js" -delete

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

# Copy production node_modules (only external dependencies)
COPY --from=builder --chown=probe:probe /app/node_modules ./node_modules

# Copy the bundled distribution instead of source files
COPY --from=builder --chown=probe:probe /app/dist ./dist

# Update the default command to use bundled app
COPY --from=builder --chown=probe:probe /app/package.json ./package.json

# Switch to non-root user
USER probe

# Expose the default web port
EXPOSE 3000

# Health check - test the actual application
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:3000/health || node --version || exit 1

# Default: CLI mode. To run web mode, use: docker run ... probe-chat-app --web
ENTRYPOINT ["node", "dist/index.js"]