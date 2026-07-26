import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';
import { resolve } from 'node:path';

export default defineConfig({
  plugins: [tailwindcss(), sveltekit()],
  resolve: {
    alias: { '@dubbing-core': resolve('../dubbing-core/src/index.ts') }
  },
  server: { fs: { allow: ['..'] } }
});
