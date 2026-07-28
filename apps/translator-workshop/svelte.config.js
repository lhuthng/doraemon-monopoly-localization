import adapter from '@sveltejs/adapter-static';

export default {
  kit: {
    adapter: adapter({ fallback: '404.html' }),
    prerender: { entries: ['*'] },
    alias: { '@dubbing-core': '../../packages/dubbing-core/src/index.ts' }
  }
};
