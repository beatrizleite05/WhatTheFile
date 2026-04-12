import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  test: {
    include: ['test/**/*.test.ts', 'test/**/*.test.tsx'],
    environmentMatchGlobs: [
      ['test/components/**', 'jsdom'],
      ['test/hooks/**', 'jsdom'],
      ['test/App.test.tsx', 'jsdom'],
    ],
    environment: 'node',
    setupFiles: ['test/setup.ts'],
    globals: true,
  },
});
