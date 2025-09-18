---
layout: home
title: Probe - Local, AI-Ready Code Intelligence
hero:
  name: Probe
  text: "Local, AI-Ready\nCode Intelligence"
  tagline: Make AI work with large codebases, and natively understand it.
  image:
    src: /logo.png
    alt: Probe Logo
  actions:
    - theme: brand
      text: Get Started →
      link: /quick-start
    - theme: alt
      text: GitHub Repo
      link: https://github.com/probelabs/probe
    - theme: alt
      text: Join Discord
      link: https://discord.gg/t5QmP9EWSS
---

<div class="quick-start-section">
  <div class="container">
    <div class="quick-start-grid">
      <div class="quick-start-option">
              <h3>Core Search</h3>
              <div class="pre-wrapper">
                <div class="language-bash"><pre><code>npx -y @probelabs/probe@latest "+stemming -lexer"</code></pre></div>
              </div>
              <p class="option-description">Fast semantic code search with elastic search syntax</p>
            </div>
            <div class="quick-start-option">
              <h3>AI Chat</h3>
              <div class="pre-wrapper">
                <div class="language-bash"><pre><code>npx -y @probelabs/probe-chat@latest --web</code></pre></div>
              </div>
              <p class="option-description">Built-in AI agent. <small>OpenAI or Anthropic (API key required)</small></p>
            </div>
            <div class="quick-start-option">
              <h3>Docker</h3>
              <div class="pre-wrapper">
                <div class="language-bash"><pre><code>docker run --rm -v $(pwd):/workspace buger/probe search "function"</code></pre></div>
              </div>
              <p class="option-description">Containerized deployment with multi-platform support</p>
            </div>
    </div>
  </div>
</div>

<StarsBackground />


<div class="main-content">
  <HomeFeatures />

</div>

<style>
.quick-start-section,
.vision-section {
  padding: 1rem 0;
}

.quick-start-section {
  text-align: center;
  margin-top: -1rem;
}

.quick-start-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 2rem;
  max-width: 1200px;
  margin: 0 auto;
}

.quick-start-option {
  text-align: center;
}

.quick-start-option h3 {
  margin-bottom: 0.5rem;
  color: var(--vp-c-text-1);
  margin-top: 0px;
}

.pre-wrapper {
  display: flex;
  justify-content: center;
  margin: 0.5rem auto;
  width: auto;
  overflow-x: auto;
}

.pre-wrapper pre {
  width: auto;
  padding: 0.75rem 0.75rem;
  border-radius: 6px;
  box-shadow: 0 2px 4px rgba(0,0,0,0.05);
  transition: box-shadow 0.2s ease;
  display: inline-block;
  overflow-x: auto;
}

.pre-wrapper pre:hover {
  box-shadow: 0 4px 8px rgba(0,0,0,0.1);
}

.option-description {
  font-size: 0.9rem;
  color: var(--vp-c-text-2);
  margin-top: 0.5rem;
}

.container {
  margin: 0 auto;
  padding: 0 0.5rem;
  max-width: 1200px;
}

.vision-section p {
  font-size: 1rem;
  line-height: 1.6;
  margin: 0.75rem 0;
  color: var(--vp-c-text-2);
}

.vision-section h2 {
  margin-top: 1.5rem;
}

h2 {
  font-size: 1.5rem;
  margin-bottom: 0.75rem;
  color: var(--vp-c-text-1);
}

@media (max-width: 640px) {
  .quick-start-grid {
    grid-template-columns: 1fr;
    gap: 1rem;
  }
  
  .quick-start-option {
    width: 100%;
  }
  
  .container {
    padding: 0 0.25rem;
  }
  
  .pre-wrapper {
    padding: 0;
  }
  
  .pre-wrapper pre {
    padding: 0.5rem 0.25rem;
    font-size: 0.9rem;
    width: 100%;
  }
  
  .quick-start-section {
    padding: 0.5rem 0;
  }
}
</style>
