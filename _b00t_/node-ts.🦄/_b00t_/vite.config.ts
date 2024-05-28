import { defineConfig } from 'vite'

// https://github.com/vitejs/vite/tree/main/packages/plugin-vue
import vue from '@vitejs/plugin-vue'

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [vue({
    template: {
      compilerOptions: {
        // isProduction: false
      }
    }
  })]
})
