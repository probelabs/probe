# Hosting a Team Chat for the Whole Company

This guide explains how to set up and deploy Probe's web interface as a centralized code intelligence platform for your entire team or company.

## Overview

Probe's web interface provides a user-friendly chat experience that allows team members to interact with your codebase using AI. By hosting this interface for your team, you can:

- Create a central knowledge hub for your codebase
- Enable non-technical team members to explore code
- Standardize code exploration and documentation practices
- Secure access to your codebase while making it more accessible
- Reduce onboarding time for new team members

## Use Cases

### 1. Developer Onboarding

New team members can quickly get up to speed on your codebase:

- "Explain the architecture of this project"
- "How is authentication implemented?"
- "What design patterns are used throughout the codebase?"
- "Show me examples of API endpoints"

### 2. Cross-Team Collaboration

Enable collaboration between different teams:

- Product managers can explore implementation details
- Designers can understand UI component implementations
- QA engineers can investigate how features are implemented
- Support teams can look up error handling patterns

### 3. Knowledge Sharing

Create a shared understanding of your codebase:

- Generate architecture diagrams
- Document complex workflows
- Explain historical design decisions
- Create consistent mental models across the team

### 4. Code Reviews

Enhance your code review process:

- Analyze changes for potential issues
- Compare implementation approaches
- Understand the context of changes
- Generate documentation for new features

## Deployment Options

### 1. Local Network Deployment

For teams working in the same office:

```bash
# Start the server on your local network
export ANTHROPIC_API_KEY=your_api_key
export ALLOWED_FOLDERS=/path/to/repos
export AUTH_ENABLED=true
export AUTH_USERNAME=team
export AUTH_PASSWORD=secure_password

npx -y @buger/probe-chat@latest --web
```

Then share the local IP address with your team (e.g., `http://192.168.1.100:8080`).

### 2. Docker Deployment

For more robust deployment:

```bash
# Build the Docker image
docker build -t code-search-chat ./examples/web

# Run the container
docker run -p 8080:8080 \
  -e ANTHROPIC_API_KEY=your_anthropic_api_key \
  -e ALLOWED_FOLDERS=/app/code \
  -e AUTH_ENABLED=true \
  -e AUTH_USERNAME=team \
  -e AUTH_PASSWORD=secure_password \
  -v /path/to/your/repos:/app/code \
  code-search-chat
```

### 3. Cloud Deployment

For teams working remotely:

1. Deploy to a VPS or cloud instance
2. Set up HTTPS with a valid SSL certificate
3. Configure authentication and access controls
4. Mount or sync your code repositories to the server

Example using a cloud VM:

```bash
# On your cloud server
git clone https://github.com/buger/probe.git
cd probe/examples/web
npm install

# Configure environment
export ANTHROPIC_API_KEY=your_api_key
export ALLOWED_FOLDERS=/path/to/repos
export AUTH_ENABLED=true
export AUTH_USERNAME=team
export AUTH_PASSWORD=secure_password
export PORT=8080

# Use PM2 to keep the server running
npm install -g pm2
pm2 start npm -- start
```

## Security and Privacy Considerations

When hosting for a team, security and privacy become especially important:

### Authentication

Always enable authentication for team deployments:

```bash
export AUTH_ENABLED=true
export AUTH_USERNAME=your_username
export AUTH_PASSWORD=your_secure_password
```

### Access Control

Carefully control which repositories are accessible:

```bash
# Only allow specific repositories
export ALLOWED_FOLDERS=/path/to/repo1,/path/to/repo2

# Exclude sensitive directories
# (Use .gitignore patterns in the repositories themselves)
```

### Network Security

For cloud deployments:

1. Use HTTPS with a valid SSL certificate
2. Consider IP whitelisting for company networks
3. Set up a VPN for additional security
4. Use a reverse proxy like Nginx with rate limiting

Example Nginx configuration:

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

### Privacy Considerations

When using AI-powered code chat with your team:

1. **External AI Services**: When using Probe with external AI services (Anthropic, OpenAI, etc.), code snippets are sent to those services
2. **Sensitive Code**: Be cautious about allowing access to repositories with sensitive or proprietary code
3. **Data Retention**: Be aware of the AI provider's data retention policies
4. **Local Model Options**: For maximum privacy, consider using Probe with locally-running AI models
5. **Clear Communication**: Ensure team members understand which parts of the system are local vs. cloud-based

### API Key Management

Securely manage your API keys:

1. Use environment variables instead of hardcoding keys
2. Consider using a secrets manager for cloud deployments
3. Regularly rotate API keys
4. Set up usage alerts to monitor API costs

## Team Workflow Integration

### Integration with Development Tools

Connect your team chat with other development tools:

1. **Link from Issue Trackers**: Add links to the team chat in JIRA or GitHub issues
2. **Documentation References**: Reference the chat in your documentation
3. **Slack Integration**: Create a Slack command to query the chat
4. **CI/CD Integration**: Generate code explanations during the build process

### Onboarding Process

Incorporate the team chat into your onboarding process:

1. Include it in your onboarding documentation
2. Create starter prompts for new team members
3. Use it during pair programming sessions
4. Generate team-specific documentation

## Advanced Configuration

### Custom Styling

Customize the appearance to match your company branding:

1. Edit the CSS in `examples/web/index.html`
2. Add your company logo
3. Adjust colors and typography
4. Add custom welcome messages

### Multiple Repositories

Configure access to multiple repositories:

```bash
# Allow access to multiple repositories
export ALLOWED_FOLDERS=/path/to/frontend,/path/to/backend,/path/to/shared-libs

# Start with a specific repository context
# (Users can still switch between allowed repositories)
export DEFAULT_FOLDER=/path/to/frontend
```

### Model Configuration

Optimize the AI model for your team's needs:

```bash
# Use a more powerful model for complex codebases
export MODEL_NAME=claude-3-opus-20240229

# Or use a faster model for quicker responses
export MODEL_NAME=claude-3-haiku-20240307
```

## Monitoring and Maintenance

### Usage Monitoring

Keep track of how your team is using the chat:

1. Enable debug logging for detailed usage information
2. Monitor API usage to control costs
3. Collect feedback from team members
4. Identify common questions for documentation

### Regular Updates

Keep your deployment up to date:

1. Regularly update the Probe package
2. Keep your code repositories in sync
3. Rotate API keys and credentials
4. Review and update access controls

## Example: Enterprise Deployment

Here's a complete example of an enterprise deployment setup:

1. **Infrastructure**:
   - Dedicated VM or container in your cloud environment
   - HTTPS with valid SSL certificate
   - Company VPN access
   - Regular backups

2. **Configuration**:
   ```bash
   # API access
   export ANTHROPIC_API_KEY=your_api_key
   
   # Repository access
   export ALLOWED_FOLDERS=/app/repos/frontend,/app/repos/backend,/app/repos/mobile
   
   # Security
   export AUTH_ENABLED=true
   export AUTH_USERNAME=company_username
   export AUTH_PASSWORD=strong_company_password
   
   # Performance
   export MODEL_NAME=claude-3-opus-20240229
   export PORT=8080
   
   # Debugging
   export DEBUG=false
   ```

3. **Repository Sync**:
   - Set up a cron job to regularly sync repositories
   - Use read-only access for security
   - Exclude sensitive files and directories

4. **Monitoring**:
   - Set up logging to a central system
   - Monitor API usage and costs
   - Track system performance
   - Set up alerts for errors or downtime

## Best Practices

1. **Create Usage Guidelines**: Document how and when to use the team chat
2. **Develop Common Queries**: Share effective prompts and queries
3. **Integrate with Documentation**: Link to the chat from your documentation
4. **Regular Training**: Train team members on effective usage
5. **Feedback Loop**: Collect and incorporate user feedback
6. **Cost Management**: Monitor and optimize API usage
7. **Security Reviews**: Regularly review access and security settings

## Troubleshooting

### Common Team Deployment Issues

| Issue | Solution |
|-------|----------|
| High API costs | Implement usage limits, use smaller models for simple queries |
| Slow responses | Check network connectivity, consider a more powerful server |
| Authentication problems | Verify credentials, check for typos in environment variables |
| Repository access issues | Check file permissions, verify paths in ALLOWED_FOLDERS |
| Model errors | Verify API key permissions, check model availability |
| Privacy concerns with sensitive code | Use local AI models, restrict access to sensitive repositories, or implement custom data filtering |

### Support Resources

If you encounter issues with your team deployment:

1. Check the [Web Interface Reference](/web-interface) for detailed configuration options
2. Review the [AI Integration Reference](/ai-integration) for model-specific information
3. Visit the [Probe GitHub repository](https://github.com/buger/probe) for the latest updates
4. Open an issue if you encounter a bug or have a feature request

## Next Steps

- Learn about [using Probe with AI Code Editors](/use-cases/ai-code-editors) for individual developer workflows
- Explore [building custom tools](/use-cases/nodejs-sdk) with the Node.js SDK
- Check out the [CLI Reference](/cli-mode) for advanced command-line usage