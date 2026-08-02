import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { createLogger, defineConfig } from 'vite';

const logger = createLogger();
const loggerWarn = logger.warn.bind(logger);
const loggerWarnOnce = logger.warnOnce.bind(logger);
const ignoreSourcemap = (msg) => typeof msg === 'string' && msg.includes('points to missing source files');
logger.warn = (msg, options) => {
  if (ignoreSourcemap(msg)) return;
  loggerWarn(msg, options);
};
logger.warnOnce = (msg, options) => {
  if (ignoreSourcemap(msg)) return;
  loggerWarnOnce(msg, options);
};

function stripDepSourceMaps() {
  return {
    name: 'strip-dep-source-maps',
    configureServer(server) {
      server.middlewares.use((req, res, next) => {
        const end = res.end.bind(res);
        res.end = (chunk, encoding, callback) => {
          if (req.url?.includes('/node_modules/')) {
            const type = res.getHeader('Content-Type');
            if (typeof type === 'string' && type.includes('javascript')) {
              const text = Buffer.isBuffer(chunk) ? chunk.toString('utf8') : String(chunk ?? '');
              const stripped = text.replace(/\/\/# sourceMappingURL=[^\n]*/g, '');
              if (stripped !== text) {
                chunk = stripped;
                res.removeHeader('Content-Length');
              }
            }
          }
          return end(chunk, encoding, callback);
        };
        next();
      });
    }
  };
}

export default defineConfig({
  customLogger: logger,
  plugins: [tailwindcss(), sveltekit(), stripDepSourceMaps()],
  server: { fs: { allow: ['../..'] } },
  optimizeDeps: { exclude: ['@bokuweb/zstd-wasm'] }
});
