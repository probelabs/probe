<template>
  <div class="full-width-feature-section">
    <div class="content-wrapper vp-doc">
      <slot name="title"></slot>
      <slot name="content"></slot>
    </div>
  </div>
</template>

<script setup>
// No props needed for this simplified component
</script>

<style scoped>
.full-width-feature-section {
  margin: 3rem 0;
}

.content-wrapper {
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
  text-align: center;
}

.content-wrapper :deep(p) {
  margin: 1rem 0;
  font-size: 1.0rem;
  line-height: 1.6;
  color: var(--vp-c-text-2);
  text-align: left;
  max-width: 800px;
  margin-left: auto;
  margin-right: auto;
}
</style>