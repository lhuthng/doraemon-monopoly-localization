import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vite';

export default defineConfig({
  base: '/',
  build: {
    copyPublicDir: false
  },
  server: {
    proxy: {
      '/api': 'http://127.0.0.1:5184'
    }
  },
  plugins: [svelte()]
});
