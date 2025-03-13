FROM node:20-slim

# Set working directory
WORKDIR /app

# Copy package.json and package-lock.json (if available)
COPY package.json ./
COPY package-lock.json* ./

# Install dependencies
RUN npm install

# Copy application files
COPY . .

# Set environment variables with defaults
ENV PORT=8080
ENV DEBUG=false
# Authentication is disabled by default
ENV AUTH_ENABLED=false
ENV AUTH_USERNAME=admin
ENV AUTH_PASSWORD=password
# API keys will be provided at runtime
# ENV ANTHROPIC_API_KEY=your_api_key_here
# ENV OPENAI_API_KEY=your_api_key_here
# ENV MODEL_NAME=your_model_override
# ENV ANTHROPIC_API_URL=https://api.anthropic.com
# ENV OPENAI_API_URL=https://api.openai.com/v1

# Expose the port the app runs on
EXPOSE 8080

# Command to run the application
CMD ["node", "main.js"]