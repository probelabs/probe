<template>
  <div class="vp-features">
    <div class="container">
      <div class="vp-features-grid">
        <Feature 
          v-for="(feature, index) in features" 
          :key="index" 
          :feature="feature" 
        />
      </div>
    </div>
  </div>
</template>

<script setup>
import { useData } from 'vitepress'
import Feature from './Feature.vue'

const { frontmatter } = useData()
const features = frontmatter.value.features || []
</script>

<style scoped>
.vp-features {
  padding: 48px 24px;
}

@media (max-width: 640px) {
  .vp-features {
    padding: 32px 8px;
  }
}

@media (max-width: 480px) {
  .vp-features {
    padding: 24px 4px;
  }
}

.container {
  margin: 0 auto;
  max-width: 1152px;
}

.vp-features-grid {
  display: grid;
  grid-template-columns: repeat(1, 1fr);
  gap: 24px;
}

@media (min-width: 640px) {
  .vp-features-grid {
    grid-template-columns: repeat(2, 1fr);
  }
}

@media (min-width: 960px) {
  .vp-features-grid {
    grid-template-columns: repeat(3, 1fr);
  }
}
</style> 