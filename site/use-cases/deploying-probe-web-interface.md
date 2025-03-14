# Centralized Code Search: Deploying the Probe Web Interface

This guide explains how to set up and deploy Probe's web interface as a centralized code search and intelligence platform for your entire organization.

## Overview

Probe's web interface provides a user-friendly chat experience that allows all team members to interact with your codebase using AI. By hosting this interface, you create a source of truth for how your product works that's accessible to both technical and non-technical team members. This enables quick issue resolution, documentation generation, architecture understanding, and helps product managers, QA teams, and other stakeholders make informed decisions without needing to understand implementation details.

## Docker-Based Setup

The most reliable way to deploy Probe's web interface for a team is using Docker:

### 1. Create a Docker Deployment

```bash
# Clone the repository if you haven't already
git clone https://github.com/buger/probe.git
cd probe/examples/web

# Build the Docker image
docker build -t probe-web-interface .
```

### 2. Run the Container

```bash
# Run the container with appropriate configuration
docker run -p 8080:8080 \
  -e ANTHROPIC_API_KEY=your_anthropic_api_key \
  -e ALLOWED_FOLDERS=/app/code \
  -e AUTH_ENABLED=true \
  -e AUTH_USERNAME=team \
  -e AUTH_PASSWORD=secure_password \
  -v /path/to/your/repos:/app/code \
  probe-web-interface
```

### 3. Access the Web Interface

Once the container is running, team members can access the web interface at:

```
http://your-server-ip:8080
```

## Setting Allowed Folders (Security & Privacy)

Controlling which code repositories are accessible is crucial for security:

### Configuring Allowed Folders

```bash
# Specify which folders can be searched
export ALLOWED_FOLDERS=/path/to/repo1,/path/to/repo2,/path/to/repo3
```

This environment variable:
- Restricts search to only the specified directories
- Prevents access to sensitive files outside these directories
- Can include multiple repositories separated by commas

### Best Practices for Folder Access

1. **Use Absolute Paths**: Always use full paths to avoid ambiguity
2. **Limit Scope**: Only include repositories that should be accessible
3. **Exclude Sensitive Directories**: Don't include directories with secrets or sensitive data
4. **Mount Read-Only**: When using Docker, consider mounting volumes as read-only:

```bash
docker run -v /path/to/your/repos:/app/code:ro ...
```

## Managing API Keys for the Chat

The web interface requires an API key for either Anthropic Claude or OpenAI:

### API Key Management

```bash
# For Anthropic Claude (recommended)
export ANTHROPIC_API_KEY=your_anthropic_api_key

# OR for OpenAI
export OPENAI_API_KEY=your_openai_api_key
```

### Best Practices for API Keys

1. **Use Environment Variables**: Never hardcode API keys in files
2. **Rotate Keys Regularly**: Change keys periodically for security
3. **Monitor Usage**: Keep track of API usage to control costs
4. **Use a Secrets Manager**: For production deployments, consider using a secrets manager

### Model Selection

You can specify which AI model to use:

```bash
# For Anthropic Claude
export MODEL_NAME=claude-3-opus-20240229

# For OpenAI
export MODEL_NAME=gpt-4o
```

## Authentication & Environment Variables

Secure your deployment with authentication:

### Enabling Authentication

```bash
# Enable basic authentication
export AUTH_ENABLED=true
export AUTH_USERNAME=your_username
export AUTH_PASSWORD=your_secure_password
```

### Complete Environment Variable Reference

| Variable | Description | Default |
|----------|-------------|---------|
| `ANTHROPIC_API_KEY` | Your Anthropic API key | (Required if not using OpenAI) |
| `OPENAI_API_KEY` | Your OpenAI API key | (Required if not using Anthropic) |
| `ALLOWED_FOLDERS` | Comma-separated list of folders to search | (Required) |
| `PORT` | The port to run the server on | 8080 |
| `MODEL_NAME` | Override the default model | claude-3-7-sonnet-latest or gpt-4o |
| `AUTH_ENABLED` | Enable basic authentication | false |
| `AUTH_USERNAME` | Username for authentication | admin |
| `AUTH_PASSWORD` | Password for authentication | password |
| `DEBUG` | Enable debug mode | false |

### Docker Compose Example

For more complex deployments, use Docker Compose:

```yaml
# docker-compose.yml
version: '3'
services:
  probe-web:
    build: ./examples/web
    ports:
      - "8080:8080"
    environment:
      - ANTHROPIC_API_KEY=${ANTHROPIC_API_KEY}
      - ALLOWED_FOLDERS=/app/code
      - AUTH_ENABLED=true
      - AUTH_USERNAME=${AUTH_USERNAME}
      - AUTH_PASSWORD=${AUTH_PASSWORD}
      - MODEL_NAME=claude-3-sonnet-20240229
    volumes:
      - /path/to/your/repos:/app/code:ro
    restart: unless-stopped
```

## Best Practices for Organization-Wide Usage

### Cross-Functional Team Onboarding

1. **Create Role-Specific Documentation**: Tailor guides for different roles (developers, product managers, QA)
2. **Provide Example Queries**: Share effective prompts and questions for common use cases
3. **Set Usage Guidelines**: Establish best practices for different types of searches
4. **Training Sessions**: Conduct role-specific training for technical and non-technical users
5. **Highlight Business Benefits**: Demonstrate how code search improves collaboration and knowledge sharing

### Performance Optimization

1. **Limit Repository Size**: Include only necessary repositories
2. **Use a Powerful Server**: Ensure adequate CPU and memory
3. **Consider SSD Storage**: Faster disk access improves search performance
4. **Regular Maintenance**: Update the Docker image and dependencies

### Security Considerations

1. **Network Security**: Use a reverse proxy with HTTPS
2. **IP Restrictions**: Limit access to your company network
3. **Regular Updates**: Keep the software updated
4. **Audit Logs**: Monitor access and usage

### Example Nginx Configuration

```nginx
server {
    listen 443 ssl;
    server_name code-chat.yourcompany.com;

    ssl_certificate /path/to/cert.pem;
    ssl_certificate_key /path/to/key.pem;

    location / {
        proxy_pass http://localhost:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        
        # Rate limiting
        limit_req zone=one burst=10 nodelay;
    }
}
```

## Cloud Deployment Options

For teams working remotely:

### Virtual Private Server (VPS)

1. Provision a VPS with adequate resources
2. Install Docker and Docker Compose
3. Deploy using the Docker Compose configuration above
4. Set up a domain name and SSL certificate
5. Configure a reverse proxy for HTTPS

### Kubernetes Deployment

For larger organizations:

```yaml
# probe-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: probe-web
spec:
  replicas: 1
  selector:
    matchLabels:
      app: probe-web
  template:
    metadata:
      labels:
        app: probe-web
    spec:
      containers:
      - name: probe-web
        image: your-registry/probe-web-interface:latest
        ports:
        - containerPort: 8080
        env:
        - name: ANTHROPIC_API_KEY
          valueFrom:
            secretKeyRef:
              name: probe-secrets
              key: anthropic-api-key
        - name: ALLOWED_FOLDERS
          value: "/app/code"
        - name: AUTH_ENABLED
          value: "true"
        - name: AUTH_USERNAME
          valueFrom:
            secretKeyRef:
              name: probe-secrets
              key: auth-username
        - name: AUTH_PASSWORD
          valueFrom:
            secretKeyRef:
              name: probe-secrets
              key: auth-password
        volumeMounts:
        - name: code-volume
          mountPath: /app/code
          readOnly: true
      volumes:
      - name: code-volume
        persistentVolumeClaim:
          claimName: code-pvc
```

## Monitoring and Maintenance

### Usage Monitoring

1. **API Usage**: Monitor API costs and usage
2. **Server Resources**: Track CPU, memory, and disk usage
3. **User Activity**: Monitor access logs and usage patterns

### Regular Updates

1. **Update Docker Image**: Regularly rebuild with the latest code
2. **Update Dependencies**: Keep Node.js and other dependencies updated
3. **Rotate Credentials**: Change API keys and passwords periodically

## Next Steps

- For individual developer workflows, see [Integrating Probe into AI Code Editors](/use-cases/ai-code-editors)
- For advanced CLI usage, check out [CLI AI Workflows](/use-cases/cli-ai-workflows)
- For programmatic access, explore [Building AI Tools on Probe](/use-cases/building-ai-tools)