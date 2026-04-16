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
    coverage: {
      provider: 'v8',
      reporter: ['text', 'html', 'json', 'lcov'],
      reportsDirectory: './coverage',
      exclude: [
        '**/node_modules/**',
        'test/**',
        'src/vite-env.d.ts',
        'src/main.tsx',
        'src/settings-main.tsx',
      ],
      thresholds: {
        lines: 70,
        functions: 70,
        branches: 65,
        statements: 70,
      },
    },
  },
});
