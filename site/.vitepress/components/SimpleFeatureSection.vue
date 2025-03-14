<template>
  <div class="simple-feature-section">
    <div class="content-side">
      <div class="content-wrapper vp-doc">
        <slot name="content"></slot>
      </div>
    </div>
    <div class="code-side">
      <div class="terminal">
        <div class="terminal-header">
          <div class="terminal-buttons">
            <span class="button red"></span>
            <span class="button yellow"></span>
            <span class="button green"></span>
          </div>
          <div class="terminal-title" v-if="title">{{ title }}</div>
        </div>
        <div class="terminal-body vp-doc">
          <slot name="code"></slot>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
defineProps({
  title: {
    type: String,
    default: ''
  }
})
</script>

<style scoped>
.simple-feature-section {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 2rem;
  margin: 3rem 0;
}

.content-side {
  padding-right: 1rem;
}

.content-wrapper {
  height: 100%;
}

.content-wrapper.vp-doc {
  width: 100%;
  max-width: 100%;
}

.content-wrapper :deep(h2) {
  margin-top: 0;
  font-size: 1.8rem;
  line-height: 1.4;
  color: var(--vp-c-text-1);
  border-top: none;
  padding-top: 0;
}

.content-wrapper :deep(p) {
  margin: 1rem 0;
  font-size: 1.1rem;
  line-height: 1.6;
  color: var(--vp-c-text-2);
  text-align: left;
}

.content-wrapper p {
  text-align: left !import;
}

.code-side {
  height: 100%;
}

.terminal {
  background-color: var(--vp-code-block-bg);
  border-radius: 8px;
  overflow: hidden;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
  height: 100%;
  display: flex;
  flex-direction: column;
}

.terminal-header {
  background-color: var(--vp-c-bg-soft);
  padding: 0.5rem 1rem;
  display: flex;
  align-items: center;
  border-bottom: 1px solid var(--vp-c-divider);
}

.terminal-buttons {
  display: flex;
  gap: 8px;
}

.button {
  width: 12px;
  height: 12px;
  border-radius: 50%;
  display: inline-block;
}

.red { background-color: #ff5f56; }
.yellow { background-color: #ffbd2e; }
.green { background-color: #27c93f; }

.terminal-title {
  margin-left: 1rem;
  font-size: 0.8rem;
  color: var(--vp-c-text-2);
}

.terminal-body {
  padding: 1rem;
  flex: 1;
  overflow: auto;
}

.terminal-body.vp-doc {
  width: 100%;
  max-width: 100%;
}

.terminal-body :deep(.language-typescript),
.terminal-body :deep(.language-bash),
.terminal-body :deep(.language-javascript),
.terminal-body :deep(.language-python),
.terminal-body :deep(.language-go),
.terminal-body :deep(.language-rust),
.terminal-body :deep(.language-java),
.terminal-body :deep(.language-c),
.terminal-body :deep(.language-cpp) {
  margin: 0;
  background-color: transparent !important;
  border-radius: 0;
  box-shadow: none;
}

.terminal-body :deep(pre) {
  margin: 0;
  background: transparent !important;
}

.terminal-body :deep(code) {
  font-family: 'Fira Code', monospace;
  font-size: 0.95rem;
  line-height: 1.5;
}

@media (max-width: 768px) {
  .simple-feature-section {
    grid-template-columns: 1fr;
  }
}
</style> 