---
# https://vitepress.dev/reference/default-theme-home-page
layout: home

hero:
  name: "PROBE"
  text: "CODE SEARCH REINVENTED"
  tagline: Find precise code blocks with full context in seconds. Built for developers who need answers fast.
  image:
    src: /logo.png
    alt: Probe Logo
  actions:
    - theme: brand
      text: GET STARTED →
      link: /quick-start
    - theme: alt
      text: GITHUB REPO
      link: https://github.com/buger/probe

features:
  - icon: 🔬
    title: DEEP CODE UNDERSTANDING
    details: Extract complete functions, classes and structures. Not just lines of code - full context every time.
  
  - icon: ⚡
    title: BUILT FOR SPEED
    details: Search massive codebases instantly. Powered by ripgrep and tree-sitter for performance that scales with your projects.
  
  - icon: 🛡️
    title: TOTALLY LOCAL
    details: Your code never leaves your machine. Full privacy with zero data collection or cloud dependencies.
  
  - icon: 🧮
    title: SMARTER RESULTS
    details: BM25 & TF-IDF algorithms deliver the most relevant code first. Find what you need without the noise.
  
  - icon: 🌍
    title: MULTI-LANGUAGE
    details: Works with Rust, JavaScript, Python, Go, Java, C++, Swift, Ruby and more. One tool for all your code.
  
  - icon: 🤖
    title: AI-READY
    details: Built for modern workflows with integrated AI chat and MCP server for seamless assistant integration.
---

## RAW POWER AT YOUR FINGERTIPS

```bash
# ONE-LINE INSTALL
curl -fsSL https://raw.githubusercontent.com/buger/probe/main/install.sh | bash

# FIND CODE THAT MATTERS
probe search "authentication flow" ./

# EXTRACT COMPLETE FUNCTIONS
probe extract src/main.rs:42

# TALK TO YOUR CODEBASE
export ANTHROPIC_API_KEY=your_api_key
probe chat
```

[EXPLORE ALL COMMANDS →](/installation)
