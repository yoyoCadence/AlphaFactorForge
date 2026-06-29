import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';

// Vite config tuned for Tauri: fixed port, no auto-open (Tauri opens the window).
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
  build: {
    target: 'es2020',
    outDir: 'dist',
  },
  // Vitest runs only the src unit tests; the Playwright e2e/ suite is separate.
  test: {
    include: ['src/**/*.{test,spec}.{ts,tsx}'],
  },
});
