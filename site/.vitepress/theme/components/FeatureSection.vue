<template>
  <div class="feature-section">
    <div class="feature-content">
      <slot name="content"></slot>
    </div>
    <div class="feature-code">
      <slot name="code"></slot>
    </div>
  </div>
</template>

<style scoped>
.feature-section {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 2rem;
  padding: 2rem 0;
  align-items: start;
}

.feature-content {
  padding-right: 2rem;
}

.feature-code {
  background: var(--vp-code-block-bg);
  border-radius: 8px;
  padding: 1rem;
}

@media (max-width: 768px) {
  .feature-section {
    grid-template-columns: 1fr;
    gap: 1rem;
  }

  .feature-content {
    padding-right: 0;
  }
}
</style> 