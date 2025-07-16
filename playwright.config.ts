import { defineConfig, devices } from "@playwright/test"

export default defineConfig({
  testDir: "./tests/e2e",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  globalSetup: "./tests/e2e/setup/global-setup.ts",
  use: {
    baseURL: "http://localhost:1420",
    trace: "on-first-retry",
  },

  projects: [
    {
      name: "webkit",
      use: { ...devices["Desktop Chrome"] },
    },
  ],

  webServer: [
    {
      name: "backend",
      command: "cargo run -p test-server",
      port: 3030,
      reuseExistingServer: !process.env.CI,
      stdout: "pipe",
      env: {
        RUST_BACKTRACE: "test_server=debug",
      },
    },
    {
      name: "frontend",
      command: "pnpm nuxt dev -p 1421",
      env: {
        NUXT_PUBLIC_TEST_MODE: "true",
      },
      port: 1421,
      reuseExistingServer: !process.env.CI,
    },
  ],
})