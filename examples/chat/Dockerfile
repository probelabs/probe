# ---- Build Stage ----
FROM node:20-slim

WORKDIR /app

# Copy package files and install dependencies
COPY package.json package-lock.json* ./
RUN npm install --production

# Copy the rest of the source code
COPY . ./

# Expose the default web port (change if needed)
EXPOSE 3000

# Default: CLI mode. To run web mode, use: docker run ... probe-chat-app --web
ENTRYPOINT ["node", "index.js"]