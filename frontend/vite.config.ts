import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';
export default defineConfig({
  root: 'frontend',
  plugins: [vue()],
  clearScreen: false,
  server: { host: '127.0.0.1', port: 1421, strictPort: true },
  build: { outDir: '../dist', emptyOutDir: true },
});
