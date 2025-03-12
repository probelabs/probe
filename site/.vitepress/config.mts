import { defineConfig } from 'vitepress'

// https://vitepress.dev/reference/site-config
export default defineConfig({
  title: "Probe",
  description: "AI-friendly, fully local, semantic code search tool designed to power the next generation of AI coding assistants",
  lastUpdated: true,
  cleanUrls: true,
  
  themeConfig: {
    
    nav: [
      { text: 'Home', link: '/' },
      { text: 'Installation', link: '/installation' },
      { text: 'Features', link: '/features' },
      {
        text: 'Usage',
        items: [
          { text: 'Quick Start', link: '/quick-start' },
          { text: 'CLI Mode', link: '/cli-mode' },
          { text: 'MCP Server', link: '/mcp-server' },
          { text: 'AI Chat', link: '/ai-chat' },
          { text: 'Web Interface', link: '/web-interface' }
        ]
      }
    ],

    sidebar: [
      {
        text: 'Getting Started',
        items: [
          { text: 'Introduction', link: '/' },
          { text: 'Installation', link: '/installation' },
          { text: 'Quick Start', link: '/quick-start' }
        ]
      },
      {
        text: 'Usage Modes',
        items: [
          { text: 'CLI Mode', link: '/cli-mode' },
          { text: 'MCP Server Mode', link: '/mcp-server' },
          { text: 'AI Chat Mode', link: '/ai-chat' },
          { text: 'Web Interface', link: '/web-interface' }
        ]
      },
      {
        text: 'Advanced Topics',
        items: [
          { text: 'Features', link: '/features' },
          { text: 'Supported Languages', link: '/supported-languages' },
          { text: 'How It Works', link: '/how-it-works' },
          { text: 'Adding New Languages', link: '/adding-languages' }
        ]
      }
    ],

    socialLinks: [
      { icon: 'github', link: 'https://github.com/buger/probe' }
    ],
    
    footer: {
      message: 'Released under the MIT License.',
      copyright: 'Copyright © 2023-present Leonid Bugaev'
    },
    
    search: {
      provider: 'local'
    },
    
    outline: {
      level: [2, 3],
      label: 'On this page'
    },
    
    carbonAds: {
      code: 'your-carbon-code',
      placement: 'your-carbon-placement'
    }
  }
})
