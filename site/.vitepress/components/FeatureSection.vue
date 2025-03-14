<template>
  <div class="feature-section">
    <div class="feature-content vp-doc">
      <!-- Use the slot directly -->
      <slot name="content"></slot>
    </div>
    <div class="feature-code">
      <div class="terminal-header">
        <div class="terminal-buttons">
          <span class="terminal-button close"></span>
          <span class="terminal-button minimize"></span>
          <span class="terminal-button maximize"></span>
        </div>
        <div class="terminal-title" v-if="title">{{ title }}</div>
      </div>
      <div class="terminal-content vp-doc">
        <!-- Use the slot directly -->
        <slot name="code"></slot>
      </div>
    </div>
  </div>
</template>

<script setup>
import { useData } from 'vitepress'

// Optional title for the terminal window
const props = defineProps({
  title: {
    type: String,
    default: ''
  },
  language: {
    type: String,
    default: 'typescript'
  }
})

// No debug logs needed
</script>

<style scoped>
.feature-section {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  gap: 3rem;
  padding: 2rem 0;
  align-items: start;
  max-width: 1200px;
  margin: 0 auto;
}

.feature-content {
  padding-right: 1rem;
  min-width: 0;
}

/* Inherit VitePress markdown styles */
.feature-content.vp-doc,
.terminal-content.vp-doc {
  width: 100%;
  max-width: 100%;
}

.feature-content :deep(h2) {
  margin-top: 0;
  font-size: 1.8rem;
  line-height: 1.4;
  color: var(--vp-c-text-1);
  border-top: none;
  padding-top: 0;
}

.feature-content :deep(p) {
  margin: 1rem 0;
  font-size: 1.1rem;
  line-height: 1.6;
  color: var(--vp-c-text-2);
}

.feature-content :deep(code) {
  font-size: 0.9em;
  font-weight: 600;
}

.feature-content :deep(a) {
  font-weight: 500;
  color: var(--vp-c-brand);
  text-decoration-style: dotted;
  transition: color 0.25s;
}

.feature-content :deep(a:hover) {
  color: var(--vp-c-brand-dark);
}

.feature-code {
  background: var(--vp-code-block-bg);
  
  overflow: hidden;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
}

.terminal-header {
  background: var(--vp-c-bg-soft);
  padding: 0.8rem 1rem;
  border-bottom: 1px solid var(--vp-c-divider);
  display: flex;
  align-items: center;
}

.terminal-buttons {
  display: flex;
  gap: 8px;
}

.terminal-title {
  margin-left: 12px;
  font-size: 0.8rem;
  color: var(--vp-c-text-2);
  font-family: 'Fira Code', monospace;
}

.terminal-button {
  width: 12px;
  height: 12px;
  border-radius: 50%;
  display: inline-block;
}

.terminal-button.close {
  background-color: #ff5f56;
}

.terminal-button.minimize {
  background-color: #ffbd2e;
}

.terminal-button.maximize {
  background-color: #27c93f;
}

.terminal-content {
}

.terminal-content :deep(pre) {
  margin: 0;
}

.terminal-content :deep(code) {
  font-family: 'Fira Code', monospace;
  font-size: 0.95rem;
  line-height: 1.5;
}

/* Ensure code blocks don't overflow */
.terminal-content :deep(pre code) {
  white-space: pre-wrap;
  word-break: break-word;
}

/* Override VitePress styles for code blocks in terminal */
.terminal-content :deep(.language-typescript),
.terminal-content :deep(.language-bash),
.terminal-content :deep(.language-javascript),
.terminal-content :deep(.language-python),
.terminal-content :deep(.language-go),
.terminal-content :deep(.language-rust),
.terminal-content :deep(.language-java),
.terminal-content :deep(.language-c),
.terminal-content :deep(.language-cpp) {
  margin: 0;
  border-radius: 0;
  box-shadow: none;
}

.feature-content :deep(.language-typescript),
.feature-content :deep(.language-bash),
.feature-content :deep(.language-javascript),
.feature-content :deep(.language-python),
.feature-content :deep(.language-go),
.feature-content :deep(.language-rust),
.feature-content :deep(.language-java),
.feature-content :deep(.language-c),
.feature-content :deep(.language-cpp) {
  margin: 1rem 0;
  border-radius: 6px;
  overflow: hidden;
}

/* Add styles for markdown content */
.feature-content :deep(ul),
.feature-content :deep(ol) {
  padding-left: 1.5rem;
  margin: 1rem 0;
}

.feature-content :deep(li) {
  margin: 0.5rem 0;
}

.feature-content :deep(blockquote) {
  border-left: 4px solid var(--vp-c-divider);
  padding-left: 1rem;
  margin: 1rem 0;
  color: var(--vp-c-text-2);
}

@media (max-width: 768px) {
  .feature-section {
    grid-template-columns: 1fr;
    gap: 2rem;
    padding: 1rem;
  }

  .feature-content {
    padding-right: 0;
  }

  .feature-content :deep(h2) {
    font-size: 1.5rem;
  }

  .feature-content :deep(p) {
    font-size: 1rem;
  }
}
</style> 