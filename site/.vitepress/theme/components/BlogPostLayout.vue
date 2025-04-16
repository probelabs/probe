<template>
  <div class="blog-post">
    <div class="blog-post-meta">
      <div v-if="frontmatter.date" class="blog-post-date">
        {{ formatDate(frontmatter.date) }}
      </div>
      <div v-if="frontmatter.author" class="blog-post-author">
        By {{ frontmatter.author }}
      </div>
      <div v-if="frontmatter.tags && frontmatter.tags.length" class="blog-post-tags">
        <span v-for="tag in frontmatter.tags" :key="tag" class="blog-post-tag">
          {{ tag }}
        </span>
      </div>
    </div>
    
    <div class="blog-post-content">
      <slot></slot>
    </div>
  </div>
</template>

<script setup>
import { useData } from 'vitepress'

const { frontmatter } = useData()

function formatDate(dateString) {
  const date = new Date(dateString)
  return date.toLocaleDateString('en-US', {
    year: 'numeric',
    month: 'long',
    day: 'numeric'
  })
}
</script>

<style scoped>
/* All styles are now in the BlogLayout.vue file with proper scoping */
/* This component just provides the structure */
</style>