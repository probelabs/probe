# Centralized Code Search: Deploying the Probe Web Interface

This guide explains how to set up and deploy Probe's web interface as a centralized code search and intelligence platform for your entire organization.

## Overview

Probe's web interface provides a user-friendly chat experience that allows all team members to interact with your codebase using AI. By hosting this interface, you create a source of truth for how your product works that's accessible to both technical and non-technical team members. This enables quick issue resolution, documentation generation, architecture understanding, and helps product managers, QA teams, and other stakeholders make informed decisions without needing to understand implementation details.


## Quick Setup with npx

The easiest way to deploy Probe's web interface for a team is using npx:

```bash
# Set your API key first (either Anthropic or OpenAI)
export ANTHROPIC_API_KEY=your_anthropic_api_key
# OR
export OPENAI_API_KEY=your_openai_api_key

# Configure allowed folders (required)
export ALLOWED_FOLDERS=/path/to/repo1,/path/to/repo2

# Enable authentication for team use
export AUTH_ENABLED=true
export AUTH_USERNAME=team
export AUTH_PASSWORD=secure_password

# Run the web interface
npx -y @buger/probe-chat --web
```

### Access the Web Interface

Once the server is running, team members can access the web interface at:

```
http://your-server-ip:8080
```

You can customize the port using the `--port` option:

```bash
npx -y @buger/probe-chat --web --port 3000
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
4. **Consider Read-Only Access**: For additional security, consider setting up read-only filesystem permissions for the user running the service

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


## Command-Line Options

The web interface supports the following command-line options:

```bash
# Show help
npx -y @buger/probe-chat --help

# Run in web mode on a specific port
npx -y @buger/probe-chat --web --port 3000

# Specify a model
npx -y @buger/probe-chat --web --model claude-3-7-sonnet-latest

# Enable debug mode
npx -y @buger/probe-chat --web --debug

# Specify a directory to search
npx -y @buger/probe-chat --web /path/to/your/project
```

### Using the npm package (Alternative)

If you prefer, you can install the package globally:

```bash
# Install globally
npm install -g @buger/probe-chat

# Start the web interface
probe-chat --web --port 3000
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
4. **Regular Maintenance**: Keep the package and Node.js updated

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
2. Install Node.js (v18 or later)
3. Set up environment variables for API keys and allowed folders
4. Run the web interface using npx
5. Set up a domain name and SSL certificate
6. Configure a reverse proxy for HTTPS

### Process Management

For production deployments, use a process manager like PM2:

```bash
# Install PM2
npm install -g pm2

# Start the web interface with PM2
pm2 start "npx -y @buger/probe-chat --web" --name "probe-web" --env "ANTHROPIC_API_KEY=your_key" "ALLOWED_FOLDERS=/path/to/repos"

# Ensure it starts on system boot
pm2 startup
pm2 save
```

## Monitoring and Maintenance

### Usage Monitoring

1. **API Usage**: Monitor API costs and usage
2. **Server Resources**: Track CPU, memory, and disk usage
3. **User Activity**: Monitor access logs and usage patterns

### Regular Updates

1. **Update Package**: Regularly update to the latest version with `npm update -g @buger/probe-chat`
2. **Update Node.js**: Keep Node.js updated to the latest LTS version
3. **Rotate Credentials**: Change API keys and passwords periodically

## Next Steps

- For individual developer workflows, see [Integrating Probe into AI Code Editors](/use-cases/integrating-probe-into-ai-code-editors)
- For advanced CLI usage, check out [CLI AI Workflows](/use-cases/advanced-cli)
- For programmatic access, explore [Building AI Tools on Probe](/use-cases/building-ai-tools)