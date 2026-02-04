import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],
  server: {
    port: 3000,
    host: '0.0.0.0',
    strictPort: false,
    allowedHosts: true,
    hmr: {
      clientPort: 3000,
    },
  },
  build: {
    outDir: 'dist',
    assetsDir: 'assets',
  }
})
