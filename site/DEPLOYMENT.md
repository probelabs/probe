# Cloudflare Pages Deployment

This documentation site is deployed using Cloudflare Pages with automatic deployments from the main branch to **probelabs.com**.

## Deployment Configuration

### Files
- `wrangler.toml` - Cloudflare Pages configuration
- `public/_headers` - HTTP headers for security and caching
- `public/_redirects` - URL redirect rules
- `.env.example` - Environment variable template

### Build Settings
- **Build command**: `npm run build`
- **Build output directory**: `.vitepress/dist`
- **Root directory**: `site`
- **Node.js version**: 20

## Setup Instructions

### 1. Cloudflare Pages Setup
1. Go to [Cloudflare Pages](https://pages.cloudflare.com/)
2. Connect your GitHub repository
3. Configure the build settings:
   - **Project name**: `probe-docs`
   - **Production branch**: `main`
   - **Build command**: `npm run build`
   - **Build output directory**: `.vitepress/dist`
   - **Root directory**: `site`

### 2. Environment Variables
Set these in Cloudflare Pages dashboard if needed:
- `NODE_VERSION`: `20`
- `NPM_VERSION`: `latest`

### 3. Custom Domain Setup
The site is configured to deploy to **probelabs.com**. To set this up:

1. In Cloudflare Pages dashboard, go to Custom domains
2. Add the domain `probelabs.com`
3. Add a redirect from `www.probelabs.com` to `probelabs.com` (already configured in `_redirects`)
4. Update your DNS records:
   - **A Record**: `probelabs.com` → Your Cloudflare Pages IP
   - **CNAME**: `www.probelabs.com` → `probelabs.com`
5. The domain configuration is already set in `wrangler.toml`

## Build Process

The site builds automatically when:
- Code is pushed to the `main` branch
- Pull requests are created (preview deployments)

### Local Development
```bash
cd site
npm install
npm run dev
```

### Local Build Test
```bash
cd site
npm run build
npm run preview
```

## Troubleshooting

### Common Issues
1. **Build fails**: Check Node.js version is 20
2. **Assets not loading**: Verify `public/` directory structure
3. **404 errors**: Check `_redirects` file configuration

### Logs
Build logs are available in the Cloudflare Pages dashboard under the deployment details.

## Migration Notes

This site was migrated from GitHub Pages. The old workflow file has been disabled and renamed to `vitepress-gh-pages.yml.disabled`.

## Performance Features

- **Edge deployment**: Served from Cloudflare's global edge network
- **Automatic HTTPS**: SSL certificates managed automatically
- **Caching**: Optimized caching headers for static assets
- **Security headers**: CSP and security headers configured