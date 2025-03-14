<template>
  <div 
    class="vp-feature" 
    :class="{ 
      'vp-feature-with-link': feature.link, 
      'vp-feature-alt': feature.theme === 'alt',
      'vp-feature-brand': feature.theme === 'brand',
      'vp-feature-highlight': feature.highlight
    }"
  >
    <div v-if="feature.icon" class="vp-feature-icon">
      <span v-if="!feature.image">{{ feature.icon }}</span>
      <img v-else :src="feature.image" alt="" />
    </div>
    <h2 class="vp-feature-title">{{ feature.title }}</h2>
    <p class="vp-feature-details">{{ feature.details }}</p>
    <div v-if="feature.link" class="vp-feature-link">
      <a :href="feature.link">{{ feature.linkText || 'Learn more' }}</a>
    </div>
  </div>
</template>

<script setup>
defineProps({
  feature: {
    type: Object,
    required: true
  }
})
</script>

<style scoped>
.vp-feature {
  border: 1px solid var(--vp-c-divider);
  border-radius: 12px;
  padding: 24px;
  height: 100%;
  background-color: var(--vp-c-bg-soft);
  transition: all 0.3s ease;
  display: flex;
  flex-direction: column;
}

.vp-feature:hover {
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
  transform: translateY(-2px);
}

.vp-feature-icon {
  font-size: 32px;
  margin-bottom: 16px;
}

.vp-feature-icon img {
  width: 32px;
  height: 32px;
}

.vp-feature-title {
  font-size: 18px;
  font-weight: 600;
  margin-bottom: 8px;
}

.vp-feature-details {
  flex-grow: 1;
  font-size: 14px;
  color: var(--vp-c-text-2);
  line-height: 1.6;
  margin-bottom: 16px;
}

.vp-feature-link a {
  font-size: 14px;
  font-weight: 500;
  color: var(--vp-c-brand);
  text-decoration: none;
}

.vp-feature-link a:hover {
  text-decoration: underline;
}

.vp-feature-alt {
  background-color: var(--vp-c-bg-alt);
}

.vp-feature-brand {
  background-color: var(--vp-c-brand-dimm);
  border-color: var(--vp-c-brand-light);
}

.vp-feature-brand .vp-feature-title {
  color: var(--vp-c-brand-dark);
}

.vp-feature-highlight {
  border: 2px solid var(--vp-c-brand);
  box-shadow: 0 0 8px rgba(var(--vp-c-brand-rgb), 0.2);
}
</style> 