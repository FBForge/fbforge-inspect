import { defineConfig, loadEnv } from 'vite';
import vue from '@vitejs/plugin-vue';
export default defineConfig({
  root: 'frontend',
  plugins: [vue()],
  define: {
    __INSPECT_BUILD_COMMIT__: JSON.stringify(loadEnv('production', '.', '').GITHUB_SHA || 'local'),
  },
  clearScreen: false,
  server: { host: '127.0.0.1', port: 1421, strictPort: true },
  build: { outDir: '../dist', emptyOutDir: true },
});
