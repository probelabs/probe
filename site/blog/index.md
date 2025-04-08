---
title: Probe Blog
description: Latest news, updates, and insights about Probe - the AI-friendly code search tool
layout: doc
---

# Probe Blog

Welcome to the Probe blog! Here you'll find the latest news, updates, and insights about Probe, the AI-friendly code search tool designed to power the next generation of AI coding assistants.

## Latest Posts

<div class="blog-post-list">
  <div class="blog-post-card">
    <h3><a href="/blog/agentic-flow-custom-xml-protocol">Technical Guide: Agentic Flow via Custom XML Protocol</a></h3>
    <div class="blog-post-meta">
      <span class="blog-post-date">July 4, 2025</span>
      <span class="blog-post-author">By Probe Team</span>
    </div>
    <p>Learn how to implement agentic AI flows using a custom XML protocol for reliable tool interactions.</p>
    <div class="blog-post-tags">
      <span class="blog-post-tag">agentic-ai</span>
      <span class="blog-post-tag">xml-protocol</span>
      <span class="blog-post-tag">tool-use</span>
      <span class="blog-post-tag">llm</span>
      <span class="blog-post-tag">technical-guide</span>
    </div>
    <a href="/blog/agentic-flow-custom-xml-protocol" class="blog-post-read-more">Read More →</a>
  </div>
</div>

<style>
.blog-post-list {
  /* display: grid; */
  /* grid-template-columns: repeat(auto-fill, minmax(300px, 1fr)); */
  /* gap: 2rem; */
  /* margin-top: 2rem; */
}

.blog-post-card {
  margin-top: 1em;
  border: 1px solid var(--vp-c-divider);
  border-radius: 8px;
  padding: 1.5rem;
  transition: transform 0.2s, box-shadow 0.2s;
}

.blog-post-card:hover {
  transform: translateY(-5px);
  box-shadow: 0 5px 15px rgba(0, 0, 0, 0.1);
}

.blog-post-card h3 {
  margin-top: 0;
  font-size: 1.3rem;
}

.blog-post-card h3 a {
  color: var(--vp-c-brand);
  text-decoration: none;
}

.blog-post-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 1rem;
  margin-bottom: 1rem;
  font-size: 0.9rem;
  color: var(--vp-c-text-2);
}

.blog-post-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem;
  margin-top: 1rem;
}

.blog-post-tag {
  font-size: 0.8rem;
  padding: 0.2rem 0.6rem;
  border-radius: 4px;
  background-color: var(--vp-c-brand-soft);
  color: var(--vp-c-brand-dark);
}

.blog-post-read-more {
  display: inline-block;
  margin-top: 1rem;
  color: var(--vp-c-brand);
  font-weight: 500;
  text-decoration: none;
}

@media (max-width: 768px) {
  .blog-post-list {
    grid-template-columns: 1fr;
  }
}
</style>
