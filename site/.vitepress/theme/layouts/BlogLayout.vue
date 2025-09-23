<script setup>
import { useData } from 'vitepress'

const { page, frontmatter } = useData()
</script>

<template>
  <div class="blog-theme">
    <header class="blog-header">
      <div class="container">
        <a href="/" class="home-link">
          <img src="/logo.png" alt="Probe" class="logo" />
          <span class="site-title">Probe</span>
        </a>
        <nav class="nav">
          <a href="/" class="nav-link">Home</a>
          <a href="/quick-start" class="nav-link">Docs</a>
          <a href="/blog/" class="nav-link active">Blog</a>
          <a href="https://github.com/probelabs/probe" class="nav-link" target="_blank">GitHub</a>
        </nav>
      </div>
    </header>

    <main class="blog-content container">
      <slot></slot>
    </main>

    <footer class="blog-footer">
      <div class="container">
        <div class="footer-content">
          <div class="footer-copyright">
            Released under the Apache 2.0 License.<br>
            Copyright © 2025 Leonid Bugaev
          </div>
          <div class="footer-links">
            <a href="https://github.com/probelabs/probe" target="_blank" class="footer-link">
              <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24"><path fill="currentColor" d="M12 2C6.475 2 2 6.475 2 12a9.994 9.994 0 0 0 6.838 9.488c.5.087.687-.213.687-.476c0-.237-.013-1.024-.013-1.862c-2.512.463-3.162-.612-3.362-1.175c-.113-.288-.6-1.175-1.025-1.413c-.35-.187-.85-.65-.013-.662c.788-.013 1.35.725 1.538 1.025c.9 1.512 2.338 1.087 2.912.825c.088-.65.35-1.087.638-1.337c-2.225-.25-4.55-1.113-4.55-4.938c0-1.088.387-1.987 1.025-2.688c-.1-.25-.45-1.275.1-2.65c0 0 .837-.262 2.75 1.026a9.28 9.28 0 0 1 2.5-.338c.85 0 1.7.112 2.5.337c1.912-1.3 2.75-1.024 2.75-1.024c.55 1.375.2 2.4.1 2.65c.637.7 1.025 1.587 1.025 2.687c0 3.838-2.337 4.688-4.562 4.938c.362.312.675.912.675 1.85c0 1.337-.013 2.412-.013 2.75c0 .262.188.574.688.474A10.016 10.016 0 0 0 22 12c0-5.525-4.475-10-10-10z"/></svg>
            </a>
            <a href="https://discord.gg/hBN4UsTZ" target="_blank" class="footer-link">
              <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24"><path fill="currentColor" d="M19.27 5.33C17.94 4.71 16.5 4.26 15 4a.09.09 0 0 0-.07.03c-.18.33-.39.76-.53 1.09a16.09 16.09 0 0 0-4.8 0c-.14-.34-.35-.76-.54-1.09c-.01-.02-.04-.03-.07-.03c-1.5.26-2.93.71-4.27 1.33c-.01 0-.02.01-.03.02c-2.72 4.07-3.47 8.03-3.1 11.95c0 .02.01.04.03.05c1.8 1.32 3.53 2.12 5.24 2.65c.03.01.06 0 .07-.02c.4-.55.76-1.13 1.07-1.74c.02-.04 0-.08-.04-.09c-.57-.22-1.11-.48-1.64-.78c-.04-.02-.04-.08-.01-.11c.11-.08.22-.17.33-.25c.02-.02.05-.02.07-.01c3.44 1.57 7.15 1.57 10.55 0c.02-.01.05-.01.07.01c.11.09.22.17.33.26c.04.03.04.09-.01.11c-.52.31-1.07.56-1.64.78c-.04.01-.05.06-.04.09c.32.61.68 1.19 1.07 1.74c.03.01.06.02.09.01c1.72-.53 3.45-1.33 5.25-2.65c.02-.01.03-.03.03-.05c.44-4.53-.73-8.46-3.1-11.95c-.01-.01-.02-.02-.04-.02zM8.52 14.91c-1.03 0-1.89-.95-1.89-2.12s.84-2.12 1.89-2.12c1.06 0 1.9.96 1.89 2.12c0 1.17-.84 2.12-1.89 2.12zm6.97 0c-1.03 0-1.89-.95-1.89-2.12s.84-2.12 1.89-2.12c1.06 0 1.9.96 1.89 2.12c0 1.17-.83 2.12-1.89 2.12z"/></svg>
            </a>
          </div>
        </div>
      </div>
    </footer>
  </div>
</template>

<style>
/* Blog theme styles - isolated with a parent class to avoid affecting documentation */
.blog-theme {
  --blog-primary-color: #3eaf7c;
  --blog-text-color: #2c3e50;
  --blog-bg-color: #ffffff;
  --blog-header-bg: #f8f8f8;
  --blog-border-color: #eaecef;
  --blog-code-bg: #f8f8f8;
  --blog-code-color: #476582;
  --blog-link-color: #3eaf7c;
  --blog-link-hover-color: #4abf8a;
  
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Oxygen, Ubuntu, Cantarell, "Fira Sans", "Droid Sans", "Helvetica Neue", sans-serif;
  color: var(--blog-text-color);
  background-color: var(--blog-bg-color);
  line-height: 1.6;
  margin: 0;
  padding: 0;
  min-height: 100vh;
  display: flex;
  flex-direction: column;
}

.dark .blog-theme {
  --blog-text-color: #f0f0f0;
  --blog-bg-color: #1a1a1a;
  --blog-header-bg: #252525;
  --blog-border-color: #333;
  --blog-code-bg: #282c34;
  --blog-code-color: #a8b1c2;
  --blog-link-color: #4abf8a;
  --blog-link-hover-color: #5ecf9a;
}

.blog-theme .container {
  max-width: 800px;
  margin: 0 auto;
  padding: 0 1.5rem;
}

.blog-theme .blog-header {
  background-color: var(--blog-header-bg);
  border-bottom: 1px solid var(--blog-border-color);
  padding: 1rem 0;
  position: sticky;
  top: 0;
  z-index: 10;
}

.blog-theme .blog-header .container {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.blog-theme .home-link {
  display: flex;
  align-items: center;
  text-decoration: none;
  color: var(--blog-text-color);
}

.blog-theme .logo {
  height: 2rem;
  margin-right: 0.5rem;
}

.blog-theme .site-title {
  font-weight: 600;
  font-size: 1.3rem;
}

.blog-theme .nav {
  display: flex;
  gap: 1.5rem;
}

.blog-theme .nav-link {
  color: var(--blog-text-color);
  text-decoration: none;
  font-weight: 500;
  transition: color 0.2s;
}

.blog-theme .nav-link:hover, 
.blog-theme .nav-link.active {
  color: var(--blog-primary-color);
}

.blog-theme .blog-content {
  padding: 2rem 0;
  flex: 1;
}

.blog-theme .blog-footer {
  background-color: var(--blog-header-bg);
  border-top: 1px solid var(--blog-border-color);
  padding: 2rem 0;
  font-size: 0.9rem;
}

.blog-theme .footer-content {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.blog-theme .footer-links {
  display: flex;
  gap: 1rem;
}

.blog-theme .footer-link {
  color: var(--blog-text-color);
  transition: color 0.2s;
}

.blog-theme .footer-link:hover {
  color: var(--blog-primary-color);
}

/* Typography for blog theme */
.blog-theme h1, 
.blog-theme h2, 
.blog-theme h3, 
.blog-theme h4, 
.blog-theme h5, 
.blog-theme h6 {
  margin-top: 1.5em;
  margin-bottom: 0.5em;
  font-weight: 600;
  line-height: 1.25;
}

.blog-theme h1 {
  font-size: 2.2rem;
  border-bottom: 1px solid var(--blog-border-color);
  padding-bottom: 0.5rem;
}

.blog-theme h2 {
  font-size: 1.65rem;
  padding-bottom: 0.3rem;
  border-bottom: 1px solid var(--blog-border-color);
}

.blog-theme h3 {
  font-size: 1.35rem;
}

.blog-theme a {
  color: var(--blog-link-color);
  text-decoration: none;
}

.blog-theme a:hover {
  color: var(--blog-link-hover-color);
  text-decoration: underline;
}

.blog-theme p {
  margin: 1em 0;
}

.blog-theme code {
  font-family: source-code-pro, Menlo, Monaco, Consolas, "Courier New", monospace;
  background-color: var(--blog-code-bg);
  color: var(--blog-code-color);
  padding: 0.25rem 0.5rem;
  border-radius: 3px;
  font-size: 0.85em;
}

.blog-theme pre {
  background-color: var(--blog-code-bg);
  border-radius: 6px;
  padding: 1rem;
  overflow: auto;
}

.blog-theme pre code {
  background-color: transparent;
  padding: 0;
}

.blog-theme blockquote {
  border-left: 4px solid var(--blog-primary-color);
  margin: 1rem 0;
  padding: 0.5rem 0 0.5rem 1rem;
  color: #666;
}

.blog-theme img {
  max-width: 100%;
}

.blog-theme table {
  border-collapse: collapse;
  width: 100%;
  margin: 1rem 0;
}

.blog-theme table, 
.blog-theme th, 
.blog-theme td {
  border: 1px solid var(--blog-border-color);
}

.blog-theme th, 
.blog-theme td {
  padding: 0.5rem 1rem;
  text-align: left;
}

.blog-theme th {
  background-color: var(--blog-header-bg);
}

/* Blog post specific styles */
.blog-theme .blog-post-meta {
  margin-bottom: 2rem;
  padding-bottom: 1.5rem;
  border-bottom: 1px solid var(--blog-border-color);
  display: flex;
  flex-wrap: wrap;
  gap: 1rem;
  align-items: center;
}

.blog-theme .blog-post-date {
  font-size: 0.9rem;
  color: var(--blog-text-color);
  opacity: 0.8;
}

.blog-theme .blog-post-author {
  font-size: 0.9rem;
  color: var(--blog-text-color);
  opacity: 0.8;
}

.blog-theme .blog-post-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem;
  margin-left: auto;
}

.blog-theme .blog-post-tag {
  font-size: 0.8rem;
  padding: 0.2rem 0.6rem;
  border-radius: 4px;
  background-color: var(--blog-primary-color);
  color: white;
  opacity: 0.8;
}

/* Blog index page styles */
.blog-theme .blog-post-list {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
  gap: 2rem;
  margin-top: 2rem;
}

.blog-theme .blog-post-card {
  border: 1px solid var(--blog-border-color);
  border-radius: 8px;
  padding: 1.5rem;
  transition: transform 0.2s, box-shadow 0.2s;
}

.blog-theme .blog-post-card:hover {
  transform: translateY(-5px);
  box-shadow: 0 5px 15px rgba(0, 0, 0, 0.1);
}

.blog-theme .blog-post-card h3 {
  margin-top: 0;
  font-size: 1.3rem;
}

.blog-theme .blog-post-card h3 a {
  color: var(--blog-primary-color);
  text-decoration: none;
}

.blog-theme .blog-post-read-more {
  display: inline-block;
  margin-top: 1rem;
  color: var(--blog-primary-color);
  font-weight: 500;
  text-decoration: none;
}

/* Responsive styles */
@media (max-width: 768px) {
  .blog-theme .blog-header .container {
    flex-direction: column;
    gap: 1rem;
  }
  
  .blog-theme .nav {
    width: 100%;
    justify-content: space-between;
  }
  
  .blog-theme .footer-content {
    flex-direction: column;
    gap: 1rem;
    text-align: center;
  }
  
  .blog-theme .footer-links {
    justify-content: center;
  }
  
  .blog-theme .blog-post-list {
    grid-template-columns: 1fr;
  }
  
  .blog-theme .blog-post-meta {
    flex-direction: column;
    align-items: flex-start;
    gap: 0.5rem;
  }
  
  .blog-theme .blog-post-tags {
    margin-left: 0;
    margin-top: 0.5rem;
  }
}
</style>