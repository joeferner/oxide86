import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import type { Plugin } from 'vite'
import fs from 'node:fs'
import path from 'node:path'

// Prevent the SPA history-fallback from serving index.html for /images/* paths.
// The SPA fallback rewrites req.url to /index.html before any "post" middleware
// runs, so we must intercept in the "pre" position: check whether the requested
// file actually exists in publicDir and return 404 immediately when it doesn't.
function noSpaFallbackForImages(): Plugin {
  return {
    name: 'no-spa-fallback-for-images',
    configureServer(server) {
      server.middlewares.use((req, res, next) => {
        const url = (req.url ?? '').split('?')[0];
        if (!url.startsWith('/images/')) {
          next();
          return;
        }
        const filePath = path.join(server.config.publicDir, url);
        if (fs.existsSync(filePath)) {
          next(); // let Vite's static-file middleware serve it
        } else {
          res.statusCode = 404;
          res.end('Not found');
        }
      });
    },
  };
}

export default defineConfig({
  plugins: [
    noSpaFallbackForImages(),
    react({
      babel: {
        plugins: [['module:@preact/signals-react-transform']],
      },
    }),
  ],
  server: {
    port: 3000,
    host: '0.0.0.0',
    strictPort: false,
    allowedHosts: true,
    hmr: {
      clientPort: 3000,
    },
  },
  css: {
    preprocessorOptions: {
      scss: {
        api: 'modern-compiler',
      },
    },
  },
  build: {
    outDir: 'dist',
    assetsDir: 'assets',
  }
})
