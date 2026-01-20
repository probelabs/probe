---
layout: page
title: Probe - Code Intelligence for AI
---

<div class="probe-page">

<!-- Hero Section -->
<section class="hero">
  <div class="hero-bg">
    <div class="hero-gradient"></div>
    <div class="hero-grid"></div>
  </div>
  <div class="hero-content">
    <div class="hero-badge">
      <span class="badge-dot"></span>
      Code Intelligence
    </div>
    <h1 class="hero-title">
      <span class="gradient-text">Probe</span>
    </h1>
    <p class="hero-tagline">Give every AI agent instant, accurate context from your private codebase.</p>
    <p class="hero-subtitle">Fully local code search that understands your codebase semantically. Query millions of lines in milliseconds. Your code never leaves your infrastructure.</p>
    <div class="hero-cta">
      <a href="/quick-start" class="btn btn-primary">
        <span>Get Started Free</span>
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none"><path d="M3.33 8h9.34M8.67 4l4 4-4 4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>
      </a>
      <a href="https://github.com/probelabs/probe" class="btn btn-secondary">View on GitHub</a>
    </div>
    <div class="hero-stats">
      <div class="stat">
        <span class="stat-value">50+</span>
        <span class="stat-label">Languages</span>
      </div>
      <div class="stat-divider"></div>
      <div class="stat">
        <span class="stat-value">100%</span>
        <span class="stat-label">Local</span>
      </div>
      <div class="stat-divider"></div>
      <div class="stat">
        <span class="stat-value">Open</span>
        <span class="stat-label">Source</span>
      </div>
    </div>
  </div>
</section>

<!-- Pain Section -->
<section class="section section-pain">
  <div class="container">
    <div class="section-header">
      <span class="section-label">The Problem</span>
      <h2>AI Tools Don't Understand Your Private Codebase</h2>
    </div>
    <div class="pain-content">
      <p>They hallucinate APIs that don't exist. They suggest patterns that contradict your architecture. They can't find the code that's actually relevant to your question. And sending your proprietary code to cloud AI services isn't an option.</p>
    </div>
  </div>
</section>

<!-- Solution Section -->
<section class="section section-solution">
  <div class="container">
    <div class="section-header">
      <span class="section-label">The Solution</span>
      <h2>Semantic Code Intelligence</h2>
    </div>
    <div class="solution-grid">
      <div class="solution-card">
        <div class="card-icon">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
            <circle cx="11" cy="11" r="8"/>
            <path d="M21 21l-4.35-4.35"/>
          </svg>
        </div>
        <h3>Semantic Understanding</h3>
        <p>Probe understands your code's structure - functions, classes, modules - not just text patterns. Ask "how does authentication work" and get accurate, relevant results.</p>
      </div>
      <div class="solution-card">
        <div class="card-icon">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
            <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/>
          </svg>
        </div>
        <h3>Millisecond Speed</h3>
        <p>Built on ripgrep for blazing fast search across millions of lines of code. Results appear instantly, even on massive monorepos.</p>
      </div>
      <div class="solution-card">
        <div class="card-icon">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
            <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
          </svg>
        </div>
        <h3>100% Local</h3>
        <p>Your code never leaves your machine. No cloud, no accounts, no telemetry. Works offline and in air-gapped environments.</p>
      </div>
      <div class="solution-card">
        <div class="card-icon">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
            <path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"/>
          </svg>
        </div>
        <h3>AI-Native Output</h3>
        <p>Results are structured for LLM consumption. Pipe directly into your AI workflows or use the built-in MCP server for Claude, Cursor, and other AI editors.</p>
      </div>
    </div>
  </div>
</section>

<!-- Code Section -->
<section class="section section-code">
  <div class="container">
    <div class="section-header">
      <span class="section-label">Quick Start</span>
      <h2>Get Started in Seconds</h2>
    </div>
    <div class="code-block">
      <div class="code-header">
        <span class="code-dot"></span>
        <span class="code-dot"></span>
        <span class="code-dot"></span>
        <span class="code-title">Terminal</span>
      </div>
      <pre><code>npx -y @probelabs/probe@latest "authentication flow" ./src</code></pre>
    </div>
    <div class="code-options">
      <div class="code-option">
        <h4>AI Chat Mode</h4>
        <div class="code-mini">
          <code>npx -y @probelabs/probe-chat@latest --web</code>
        </div>
        <p>Built-in AI agent with OpenAI/Anthropic support</p>
      </div>
      <div class="code-option">
        <h4>MCP Server</h4>
        <div class="code-mini">
          <code>probe mcp</code>
        </div>
        <p>For Claude, Cursor, and AI editors</p>
      </div>
    </div>
  </div>
</section>

<!-- Features Section -->
<section class="section section-features">
  <div class="container">
    <div class="section-header">
      <span class="section-label">Capabilities</span>
      <h2>Key Features</h2>
    </div>
    <div class="features-grid">
      <div class="feature-card">
        <h3>50+ Languages</h3>
        <p>Support for all major programming languages with accurate AST parsing.</p>
      </div>
      <div class="feature-card">
        <h3>Elasticsearch Syntax</h3>
        <p>Powerful query syntax you already know. Boolean operators, phrase matching, fuzzy search.</p>
      </div>
      <div class="feature-card">
        <h3>Code Extraction</h3>
        <p>Extract specific functions, classes, or code blocks with context.</p>
      </div>
      <div class="feature-card">
        <h3>Web Interface</h3>
        <p>Browser-based search for teams and collaboration.</p>
      </div>
      <div class="feature-card">
        <h3>Node.js SDK</h3>
        <p>Programmatic access for building custom AI tools.</p>
      </div>
      <div class="feature-card">
        <h3>Open Source</h3>
        <p>Apache 2.0 licensed. Run it, modify it, embed it.</p>
      </div>
    </div>
  </div>
</section>

<!-- Docs Section -->
<section class="section section-docs">
  <div class="container">
    <div class="section-header">
      <span class="section-label">Resources</span>
      <h2>Documentation</h2>
    </div>
    <div class="docs-grid">
      <a href="/quick-start" class="doc-card">
        <h3>Quick Start</h3>
        <p>Get up and running in minutes</p>
        <span class="doc-link">Read guide <svg width="12" height="12" viewBox="0 0 12 12"><path d="M2.5 6h7M6.5 3l3 3-3 3" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"/></svg></span>
      </a>
      <a href="/features" class="doc-card">
        <h3>Features</h3>
        <p>Explore all capabilities</p>
        <span class="doc-link">Learn more <svg width="12" height="12" viewBox="0 0 12 12"><path d="M2.5 6h7M6.5 3l3 3-3 3" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"/></svg></span>
      </a>
      <a href="/mcp-server" class="doc-card">
        <h3>MCP Server</h3>
        <p>Integrate with AI editors</p>
        <span class="doc-link">View docs <svg width="12" height="12" viewBox="0 0 12 12"><path d="M2.5 6h7M6.5 3l3 3-3 3" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"/></svg></span>
      </a>
      <a href="/cli-mode" class="doc-card">
        <h3>CLI Reference</h3>
        <p>Command line usage</p>
        <span class="doc-link">See commands <svg width="12" height="12" viewBox="0 0 12 12"><path d="M2.5 6h7M6.5 3l3 3-3 3" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"/></svg></span>
      </a>
    </div>
  </div>
</section>

<!-- CTA Section -->
<section class="section section-cta">
  <div class="container">
    <div class="cta-card">
      <h2>Start Understanding Your Codebase</h2>
      <p>Open source and free to use. Single binary, no dependencies.</p>
      <div class="cta-buttons">
        <a href="/quick-start" class="btn btn-primary btn-lg">
          <span>Get Started</span>
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none"><path d="M3.33 8h9.34M8.67 4l4 4-4 4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>
        </a>
        <a href="https://github.com/probelabs/probe" class="btn btn-secondary">GitHub</a>
        <a href="https://discord.gg/hBN4UsTZ" class="btn btn-ghost">Join Discord</a>
      </div>
    </div>
  </div>
</section>

</div>

<style>
/* ========================================
   Design System - Matching Homepage
   ======================================== */

@import url('https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&display=swap');

.probe-page {
  /* Colors */
  --c-bg: #000000;
  --c-bg-subtle: #0a0a0a;
  --c-bg-muted: #141414;
  --c-border: rgba(255, 255, 255, 0.08);
  --c-border-hover: rgba(255, 255, 255, 0.15);
  --c-text: #ededed;
  --c-text-muted: #888888;
  --c-text-subtle: #666666;
  --c-primary: #7c3aed;
  --c-primary-light: #a78bfa;
  --c-accent: #06b6d4;
  --c-green: #22c55e;

  /* Gradients */
  --g-primary: linear-gradient(135deg, #7c3aed 0%, #06b6d4 100%);

  /* Typography */
  --font: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
  --font-mono: 'SF Mono', Monaco, 'Cascadia Code', monospace;

  /* Spacing */
  --space-xs: 0.25rem;
  --space-sm: 0.5rem;
  --space-md: 1rem;
  --space-lg: 1.5rem;
  --space-xl: 2rem;
  --space-2xl: 3rem;
  --space-3xl: 4rem;
  --space-4xl: 6rem;

  /* Radius */
  --radius-sm: 6px;
  --radius-md: 10px;
  --radius-lg: 16px;
  --radius-xl: 24px;

  /* Shadows */
  --shadow-sm: 0 2px 8px rgba(0, 0, 0, 0.3);
  --shadow-md: 0 8px 30px rgba(0, 0, 0, 0.4);
  --shadow-glow: 0 0 60px rgba(124, 58, 237, 0.3);

  /* Base */
  font-family: var(--font);
  background: var(--c-bg);
  color: var(--c-text);
  line-height: 1.6;
  -webkit-font-smoothing: antialiased;
}

.container { max-width: 1200px; margin: 0 auto; padding: 0 var(--space-xl); }

/* Hero */
.hero {
  position: relative;
  min-height: 80vh;
  display: flex;
  align-items: center;
  justify-content: center;
  text-align: center;
  padding: var(--space-4xl) var(--space-xl);
  overflow: hidden;
}

.hero-bg { position: absolute; inset: 0; }

.hero-gradient {
  position: absolute;
  top: -50%;
  left: 50%;
  transform: translateX(-50%);
  width: 150%;
  height: 100%;
  background: radial-gradient(ellipse at center, rgba(124, 58, 237, 0.15) 0%, transparent 60%);
}

.hero-grid {
  position: absolute;
  inset: 0;
  background-image: linear-gradient(rgba(255, 255, 255, 0.02) 1px, transparent 1px), linear-gradient(90deg, rgba(255, 255, 255, 0.02) 1px, transparent 1px);
  background-size: 60px 60px;
  mask-image: radial-gradient(ellipse at center, black 20%, transparent 70%);
}

.hero-content { position: relative; max-width: 800px; z-index: 1; }

.hero-badge {
  display: inline-flex;
  align-items: center;
  gap: var(--space-sm);
  font-size: 0.8125rem;
  font-weight: 500;
  color: var(--c-text-muted);
  background: var(--c-bg-muted);
  border: 1px solid var(--c-border);
  padding: var(--space-sm) var(--space-md);
  border-radius: 100px;
  margin-bottom: var(--space-xl);
}

.badge-dot {
  width: 6px;
  height: 6px;
  background: var(--c-primary);
  border-radius: 50%;
  animation: pulse 2s ease-in-out infinite;
}

@keyframes pulse { 0%, 100% { opacity: 1; } 50% { opacity: 0.5; } }

.hero-title {
  font-size: clamp(3rem, 8vw, 6rem);
  font-weight: 700;
  line-height: 1;
  margin: 0 0 var(--space-lg);
}

.gradient-text {
  background: var(--g-primary);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
  background-clip: text;
}

.hero-tagline {
  font-size: 1.5rem;
  font-weight: 500;
  margin: 0 0 var(--space-md);
  color: var(--c-text);
}

.hero-subtitle {
  font-size: 1.125rem;
  color: var(--c-text-muted);
  max-width: 600px;
  margin: 0 auto var(--space-2xl);
}

.hero-cta { display: flex; gap: var(--space-md); justify-content: center; flex-wrap: wrap; margin-bottom: var(--space-3xl); }

.hero-stats { display: flex; align-items: center; justify-content: center; gap: var(--space-2xl); }
.stat { text-align: center; }
.stat-value { display: block; font-size: 1.5rem; font-weight: 700; color: var(--c-text); }
.stat-label { font-size: 0.8125rem; color: var(--c-text-subtle); }
.stat-divider { width: 1px; height: 40px; background: var(--c-border); }

/* Buttons */
.btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-sm);
  padding: 0.875rem 1.5rem;
  font-family: var(--font);
  font-size: 0.9375rem;
  font-weight: 500;
  border-radius: var(--radius-md);
  text-decoration: none;
  transition: all 0.2s ease;
  border: none;
}

.btn-primary {
  color: #fff;
  background: var(--c-primary);
  box-shadow: 0 0 0 1px rgba(124, 58, 237, 0.5), var(--shadow-sm);
}

.btn-primary:hover {
  background: #6d28d9;
  box-shadow: 0 0 0 1px rgba(124, 58, 237, 0.7), var(--shadow-md), var(--shadow-glow);
  transform: translateY(-1px);
}

.btn-secondary {
  color: var(--c-text);
  background: var(--c-bg-muted);
  border: 1px solid var(--c-border);
}

.btn-secondary:hover { background: var(--c-bg-subtle); border-color: var(--c-border-hover); }

.btn-ghost { color: var(--c-text-muted); background: transparent; }
.btn-ghost:hover { color: var(--c-text); }

.btn-lg { padding: 1rem 2rem; font-size: 1rem; }

/* Sections */
.section { padding: var(--space-4xl) 0; }

.section-header { text-align: center; margin-bottom: var(--space-3xl); }

.section-label {
  display: inline-block;
  font-size: 0.75rem;
  font-weight: 600;
  letter-spacing: 0.1em;
  text-transform: uppercase;
  color: var(--c-primary-light);
  margin-bottom: var(--space-md);
}

.section-header h2 {
  font-size: clamp(1.75rem, 4vw, 2.5rem);
  font-weight: 700;
  letter-spacing: -0.02em;
  margin: 0;
  color: var(--c-text);
}

/* Pain Section */
.section-pain { background: var(--c-bg-subtle); }
.pain-content { max-width: 700px; margin: 0 auto; text-align: center; }
.pain-content p { font-size: 1.125rem; color: var(--c-text-muted); line-height: 1.7; margin: 0; }

/* Solution Cards */
.section-solution { background: var(--c-bg); }
.solution-grid { display: grid; grid-template-columns: repeat(2, 1fr); gap: var(--space-lg); }

.solution-card {
  padding: var(--space-xl);
  background: var(--c-bg-subtle);
  border: 1px solid var(--c-border);
  border-radius: var(--radius-lg);
  transition: all 0.3s ease;
}

.solution-card:hover { border-color: var(--c-border-hover); transform: translateY(-2px); }

.card-icon {
  width: 48px;
  height: 48px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: linear-gradient(135deg, rgba(124, 58, 237, 0.1) 0%, rgba(6, 182, 212, 0.1) 100%);
  border: 1px solid rgba(124, 58, 237, 0.2);
  border-radius: var(--radius-md);
  margin-bottom: var(--space-lg);
}

.card-icon svg { width: 24px; height: 24px; color: var(--c-primary-light); }
.solution-card h3 { font-size: 1.125rem; font-weight: 600; margin: 0 0 var(--space-sm); color: var(--c-text); }
.solution-card p { font-size: 0.9375rem; color: var(--c-text-muted); margin: 0; line-height: 1.6; }

/* Code Section */
.section-code { background: var(--c-bg-subtle); }

.code-block {
  max-width: 700px;
  margin: 0 auto var(--space-2xl);
  background: #0f172a;
  border: 1px solid var(--c-border);
  border-radius: var(--radius-lg);
  overflow: hidden;
}

.code-header {
  display: flex;
  align-items: center;
  gap: var(--space-sm);
  padding: var(--space-md) var(--space-lg);
  background: #1e293b;
}

.code-dot { width: 10px; height: 10px; border-radius: 50%; background: #334155; }
.code-dot:nth-child(1) { background: #ef4444; }
.code-dot:nth-child(2) { background: #eab308; }
.code-dot:nth-child(3) { background: #22c55e; }

.code-title { margin-left: auto; font-size: 0.75rem; color: #64748b; }

.code-block pre { margin: 0; padding: var(--space-lg); overflow-x: auto; }
.code-block code { color: var(--c-green); font-family: var(--font-mono); font-size: 1rem; }

.code-options { display: grid; grid-template-columns: repeat(2, 1fr); gap: var(--space-lg); max-width: 700px; margin: 0 auto; }

.code-option {
  padding: var(--space-lg);
  background: var(--c-bg);
  border: 1px solid var(--c-border);
  border-radius: var(--radius-lg);
}

.code-option h4 { font-size: 1rem; font-weight: 600; margin: 0 0 var(--space-md); color: var(--c-text); }

.code-mini {
  background: #0f172a;
  border-radius: var(--radius-sm);
  padding: var(--space-md);
  margin-bottom: var(--space-md);
  overflow-x: auto;
}

.code-mini code { color: var(--c-green); font-family: var(--font-mono); font-size: 0.875rem; }
.code-option p { font-size: 0.875rem; color: var(--c-text-muted); margin: 0; }

/* Features Section */
.section-features { background: var(--c-bg); }
.features-grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: var(--space-lg); }

.feature-card {
  padding: var(--space-lg);
  background: var(--c-bg-subtle);
  border: 1px solid var(--c-border);
  border-radius: var(--radius-lg);
  transition: all 0.3s ease;
}

.feature-card:hover { border-color: var(--c-border-hover); }
.feature-card h3 { font-size: 1rem; font-weight: 600; margin: 0 0 var(--space-sm); color: var(--c-text); }
.feature-card p { font-size: 0.875rem; color: var(--c-text-muted); margin: 0; }

/* Docs Section */
.section-docs { background: var(--c-bg-subtle); }
.docs-grid { display: grid; grid-template-columns: repeat(4, 1fr); gap: var(--space-lg); }

.doc-card {
  display: flex;
  flex-direction: column;
  padding: var(--space-lg);
  background: var(--c-bg);
  border: 1px solid var(--c-border);
  border-radius: var(--radius-lg);
  text-decoration: none;
  color: inherit;
  transition: all 0.3s ease;
}

.doc-card:hover { border-color: var(--c-primary); box-shadow: var(--shadow-md), 0 0 40px rgba(124, 58, 237, 0.1); transform: translateY(-4px); }
.doc-card h3 { font-size: 1rem; font-weight: 600; margin: 0 0 var(--space-xs); color: var(--c-text); }
.doc-card p { font-size: 0.875rem; color: var(--c-text-muted); margin: 0 0 var(--space-md); flex-grow: 1; }

.doc-link {
  display: inline-flex;
  align-items: center;
  gap: var(--space-xs);
  font-size: 0.8125rem;
  font-weight: 500;
  color: var(--c-primary-light);
}

.doc-card:hover .doc-link { color: #fff; }

/* CTA Section */
.section-cta { background: var(--c-bg); padding: var(--space-4xl) 0; }

.cta-card {
  text-align: center;
  padding: var(--space-3xl);
  background: linear-gradient(135deg, rgba(124, 58, 237, 0.1) 0%, rgba(6, 182, 212, 0.05) 100%);
  border: 1px solid rgba(124, 58, 237, 0.2);
  border-radius: var(--radius-xl);
}

.cta-card h2 { font-size: 2rem; font-weight: 700; margin: 0 0 var(--space-md); }
.cta-card > p { font-size: 1.125rem; color: var(--c-text-muted); margin: 0 0 var(--space-xl); }
.cta-buttons { display: flex; gap: var(--space-md); justify-content: center; flex-wrap: wrap; }

/* Responsive */
@media (max-width: 1024px) {
  .features-grid, .docs-grid { grid-template-columns: repeat(2, 1fr); }
}

@media (max-width: 768px) {
  .hero { min-height: auto; padding: var(--space-3xl) var(--space-md); }
  .hero-title { font-size: 3rem; }
  .hero-stats { flex-direction: column; gap: var(--space-lg); }
  .stat-divider { display: none; }
  .solution-grid, .code-options, .features-grid, .docs-grid { grid-template-columns: 1fr; }
  .section { padding: var(--space-3xl) 0; }
  .container { padding: 0 var(--space-md); }
  .cta-card { padding: var(--space-xl); }
}

/* Light Mode */
html:not(.dark) .probe-page {
  --c-bg: #ffffff;
  --c-bg-subtle: #fafafa;
  --c-bg-muted: #f4f4f5;
  --c-border: rgba(0, 0, 0, 0.08);
  --c-border-hover: rgba(0, 0, 0, 0.15);
  --c-text: #18181b;
  --c-text-muted: #52525b;
  --c-text-subtle: #71717a;
}

html:not(.dark) .hero-gradient { background: radial-gradient(ellipse at center, rgba(124, 58, 237, 0.08) 0%, transparent 60%); }
html:not(.dark) .hero-grid { background-image: linear-gradient(rgba(0, 0, 0, 0.03) 1px, transparent 1px), linear-gradient(90deg, rgba(0, 0, 0, 0.03) 1px, transparent 1px); }
html:not(.dark) .btn-primary { box-shadow: 0 0 0 1px rgba(124, 58, 237, 0.3), var(--shadow-sm); }
html:not(.dark) .card-icon { background: linear-gradient(135deg, rgba(124, 58, 237, 0.08) 0%, rgba(6, 182, 212, 0.08) 100%); border-color: rgba(124, 58, 237, 0.15); }
html:not(.dark) .cta-card { background: linear-gradient(135deg, rgba(124, 58, 237, 0.05) 0%, rgba(6, 182, 212, 0.02) 100%); border-color: rgba(124, 58, 237, 0.1); }
</style>
