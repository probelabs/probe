import { h } from 'vue'
import DefaultTheme from 'vitepress/theme'
import './custom.css'
import './home.css'
import FeatureList from './components/FeatureList.vue'
import CodeEditor from './components/CodeEditor.vue'
import CommandExample from './components/CommandExample.vue'
import FeatureSection from '../components/FeatureSection.vue'
import SimpleFeatureSection from '../components/SimpleFeatureSection.vue'
import StarsBackground from '../components/StarsBackground.vue'
import HomeFeatures from '../components/HomeFeatures.vue'

export default {
	...DefaultTheme,
	Layout() {
		return h(DefaultTheme.Layout, null, {
			'home-features-after': () => h(FeatureList)
		});
	},
	enhanceApp({ app }) {
		// Register global components
		app.component('FeatureList', FeatureList)
		app.component('CodeEditor', CodeEditor)
		app.component('CommandExample', CommandExample)
		app.component('FeatureSection', FeatureSection)
		app.component('SimpleFeatureSection', SimpleFeatureSection)
		app.component('StarsBackground', StarsBackground)
		app.component('HomeFeatures', HomeFeatures)
	}
}
