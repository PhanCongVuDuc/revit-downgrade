import { fileURLToPath, URL } from 'node:url';

import vue from '@vitejs/plugin-vue';
import { defineConfig } from 'vite';

import { prerender } from './build/prerender.ts';

export default defineConfig({
  plugins: [vue(), prerender()],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
  build: {
    // One stylesheet for every view: the prerendered pages link it, so their content is styled before the view's code loads.
    cssCodeSplit: false,
  },
  server: {
    proxy: {
      '/api': 'http://127.0.0.1:8080',
    },
  },
});
