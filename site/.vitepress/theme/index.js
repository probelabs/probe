import { h, onMounted } from 'vue'
import DefaultTheme from 'vitepress/theme'
import './custom.css'


export default {
	...DefaultTheme,
	Layout() {
		return h(DefaultTheme.Layout, null, {
			// You can add custom layout slots here if needed
		});
	}
}
