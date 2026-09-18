import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './tests/e2e',
  timeout: 120_000,
  expect: { timeout: 15_000 },
  workers: 1,
  retries: 0,
  reporter: [['list'], ['json', { outputFile: '../../bin/test-artifacts/browser-results.json' }]],
  outputDir: '../../bin/test-artifacts/browser',
  use: {
    baseURL: process.env.VHA_TEST_BASE_URL ?? 'http://127.0.0.1:8420',
    viewport: { width: 1440, height: 1000 },
    headless: true,
    launchOptions: { executablePath: process.env.VHA_TEST_CHROME ?? '/usr/bin/google-chrome-stable', args: ['--no-sandbox'] },
    screenshot: 'only-on-failure',
    trace: 'retain-on-failure',
  },
});
