import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

const engineTarget = 'http://127.0.0.1:8080'

export default defineConfig({
  plugins: [react()],
  server: {
    proxy: {
      '/health': {
        target: engineTarget,
        changeOrigin: true,
      },
      '/system': {
        target: engineTarget,
        changeOrigin: true,
      },
      '/ingest': {
        target: engineTarget,
        changeOrigin: true,
      },
      '/sessions': {
        target: engineTarget,
        changeOrigin: true,
      },
      '/chat': {
        target: engineTarget,
        changeOrigin: true,
        ws: true,
      },
      '/index': {
        target: engineTarget,
        changeOrigin: true,
        ws: true,
      },
    },
  },
})
