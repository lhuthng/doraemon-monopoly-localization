import js from '@eslint/js';
import { defineConfig } from 'eslint/config';
import svelte from 'eslint-plugin-svelte';
import globals from 'globals';
import svelteConfig from './svelte.config.js';

export default defineConfig(
  { ignores: ['.svelte-kit/**', 'build/**', 'dist/**', 'node_modules/**'] },
  js.configs.recommended,
  svelte.configs.recommended,
  svelte.configs.prettier,
  {
    languageOptions: {
      globals: {
        ...globals.browser,
        ...globals.node,
        ...globals.bunBuiltin
      }
    }
  },
  {
    rules: {
      // Temporary Maps/Sets are intentionally non-reactive; reactive collections
      // are replaced rather than mutated throughout this SPA.
      'svelte/prefer-svelte-reactivity': 'off'
    }
  },
  {
    files: ['**/*.svelte'],
    languageOptions: {
      parserOptions: {
        extraFileExtensions: ['.svelte'],
        svelteConfig
      }
    }
  }
);
