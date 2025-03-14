<!-- .vitepress/theme/components/CodeBlock.vue -->
<template>
  <div class="code-block-wrapper">
    <div class="code-block-header" v-if="showHeader">
      <div class="window-controls">
        <span class="window-button close"></span>
        <span class="window-button minimize"></span>
        <span class="window-button maximize"></span>
      </div>
      <div class="code-language" v-if="lang">{{ lang }}</div>
    </div>
    <div class="code-block" :class="{ 'no-header': !showHeader }">
      <pre class="language-{{lang}}"><code v-html="highlightedCode"></code></pre>
    </div>
  </div>
</template>

<script lang="ts" setup>
import { ref, onMounted } from 'vue'
import { useData } from 'vitepress'
import { getHighlighter } from 'shiki'

const props = defineProps<{
  code: string
  lang?: string
  showHeader?: boolean
}>()

const { isDark } = useData()
const highlightedCode = ref('')

onMounted(async () => {
  const highlighter = await getHighlighter({
    theme: isDark.value ? 'github-dark' : 'github-light',
    langs: [props.lang || 'text']
  })
  highlightedCode.value = highlighter.codeToHtml(props.code, { 
    lang: props.lang || 'text',
    theme: isDark.value ? 'github-dark' : 'github-light'
  })
})
</script>

<style scoped>
.code-block-wrapper {
  margin: var(--space-md) 0;
  border-radius: var(--radius-lg);
  overflow: hidden;
  background: var(--bg-code-block);
  box-shadow: 0 4px 12px var(--shadow-color);
}

.code-block-header {
  background: #2C2C2C;
  padding: var(--space-xs) var(--space-sm);
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.window-controls {
  display: flex;
  gap: var(--space-xs);
}

.window-button {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  display: inline-block;
}

.window-button.close {
  background: #FF5F56;
}

.window-button.minimize {
  background: #FFBD2E;
}

.window-button.maximize {
  background: #27C93F;
}

.code-language {
  font-family: var(--font-mono);
  font-size: 12px;
  color: #9E9E9E;
}

.code-block {
  margin: 0;
  padding: var(--space-xs);
  overflow-x: auto;
  background: transparent;
}

.code-block.no-header {
  border-radius: var(--radius-lg);
}

.code-block pre {
  margin: 0;
  padding: 0;
  background: transparent !important;
}

.code-block code {
  font-family: var(--font-mono);
  font-size: 14px;
  line-height: 1.5;
  padding: 0;
}

:deep(.shiki) {
  background: transparent !important;
  padding: 0 !important;
}

:deep(.line) {
  min-height: 1.5em;
}

/* Dark mode overrides */
:deep(.dark .shiki) {
  background: transparent !important;
}

:deep(.shiki .line) {
  min-height: 1.5em;
}

:deep(.shiki .comment) { color: var(--syntax-comment); }
:deep(.shiki .string) { color: var(--syntax-string); }
:deep(.shiki .number) { color: var(--syntax-constant); }
:deep(.shiki .keyword) { color: var(--syntax-keyword); }
:deep(.shiki .function) { color: var(--syntax-function); }
:deep(.shiki .class-name) { color: var(--syntax-class); }
:deep(.shiki .operator) { color: var(--syntax-operator); }
:deep(.shiki .punctuation) { color: var(--syntax-punctuation); }
</style> 