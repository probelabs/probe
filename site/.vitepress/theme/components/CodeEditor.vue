<template>
  <div class="code-editor">
    <div class="code-editor-header">
      <div class="window-button close"></div>
      <div class="window-button minimize"></div>
      <div class="window-button maximize"></div>
      <div class="code-editor-path" v-if="filePath">{{ filePath }}</div>
    </div>
    <div class="code-editor-content" :class="{ 'has-matches': hasMatches }">
      <pre><code><slot></slot></code></pre>
      <div v-if="hasMatches" class="code-matches">
        <div v-for="(match, index) in matches" :key="index" class="match">
          <div class="match-highlight" :style="{ top: `${match.line * 24}px` }"></div>
          <div class="match-indicator" :style="{ top: `${match.line * 24}px` }">
            <span class="match-line">Line {{ match.line }}</span>
            <span class="match-text">{{ match.text }}</span>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed } from 'vue'

const props = defineProps({
  filePath: {
    type: String,
    default: ''
  },
  matches: {
    type: Array,
    default: () => []
  }
})

const hasMatches = computed(() => props.matches.length > 0)
</script>

<style scoped>
.code-editor-content {
  padding: var(--space-md);
  position: relative;
  font-family: var(--font-mono);
  font-size: 14px;
  line-height: 1.5;
  overflow-x: auto;
}

.code-editor-content pre {
  margin: 0;
}

.code-editor-content code {
  background: none;
  padding: 0;
}

.match-highlight {
  position: absolute;
  left: 0;
  right: 0;
  height: 24px;
  background: rgba(var(--vp-c-brand-rgb), 0.1);
  pointer-events: none;
}

.match-indicator {
  position: absolute;
  right: var(--space-md);
  display: flex;
  align-items: center;
  gap: var(--space-sm);
  font-size: 12px;
  color: var(--text-secondary);
}

.match-line {
  background: rgba(var(--vp-c-brand-rgb), 0.2);
  padding: 2px 6px;
  border-radius: var(--radius-sm);
}

.match-text {
  opacity: 0.7;
}

/* Syntax highlighting */
:deep(.token.string) { color: var(--syntax-string); }
:deep(.token.variable) { color: var(--syntax-variable); }
:deep(.token.function) { color: var(--syntax-function); }
:deep(.token.number) { color: var(--syntax-number); }
:deep(.token.comment) { color: var(--syntax-comment); }
:deep(.token.keyword) { color: var(--syntax-keyword); }
</style> 