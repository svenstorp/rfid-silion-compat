import { defineConfig } from 'vite';

export default defineConfig({
  root: '.',
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    target: 'ES2020',
    minify: 'terser',
    sourcemap: true,
  },
  server: {
    port: 5173,
    strictPort: false,
    open: true,
  },
  preview: {
    port: 8080,
    strictPort: false,
  },
});
