<template>
  <div class="command-example" :class="{ 'with-output': !!output }">
    <div class="command-line">
      <span class="prompt">$</span>
      <slot></slot>
    </div>
    <div v-if="output" class="command-output">
      <pre><code>{{ output }}</code></pre>
    </div>
  </div>
</template>

<script setup>
defineProps({
  output: {
    type: String,
    default: ''
  }
})
</script>

<style scoped>
.command-example {
  background: var(--bg-code);
  border-radius: var(--radius-md);
  overflow: hidden;
}

.command-line {
  padding: var(--space-md);
  font-family: var(--font-mono);
  font-size: 14px;
  display: flex;
  gap: var(--space-sm);
  background: #2C2C2C;
  color: #fff;
}

.prompt {
  color: #64DD17;
  user-select: none;
}

.command-output {
  padding: var(--space-md);
  font-family: var(--font-mono);
  font-size: 14px;
  color: var(--text-secondary);
  background: var(--bg-code);
  border-top: 1px solid rgba(0, 0, 0, 0.1);
}

.command-output pre {
  margin: 0;
  white-space: pre-wrap;
}
</style> 