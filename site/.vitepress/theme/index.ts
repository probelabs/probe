import { h } from 'vue'
import { Theme } from 'vitepress'
import DefaultTheme from 'vitepress/theme'
import FeatureSection from './components/FeatureSection.vue'
import './custom.css'
import './home.css'

// @ts-ignore - Vue component type issues
import CodeBlock from './components/CodeBlock.vue'

export default {
  extends: DefaultTheme,
  enhanceApp({ app }) {
    app.component('FeatureSection', FeatureSection)
    app.component('CodeBlock', CodeBlock)
  }
} as Theme 