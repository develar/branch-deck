import { defineConfig, devices } from "@playwright/test"
import { cpus } from "os"

export default defineConfig({
  testDir: "./tests/e2e",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : cpus().length,
  globalSetup: "./tests/e2e/setup/global-setup.ts",

  // Output directories for test artifacts
  outputDir: "./tests/results",

  use: {
    baseURL: "http://localhost:1421",
    trace: "on-first-retry",
    // Set fixed timezone and locale for consistent test results
    timezoneId: "Europe/Berlin",
    locale: "en-US",
  },

  projects: [
    {
      name: "chromium",
      use: {
        ...devices["Desktop Chrome"],
      },
      snapshotPathTemplate: "{testDir}/{testFileDir}/snapshots/{arg}{ext}",
    },
  ],

  webServer: [
    {
      name: "backend",
      command: process.env.CI ? "cargo run -p test-server" : "pnpm test-server:dev",
      port: 3030,
      reuseExistingServer: !process.env.CI,
      stdout: "pipe",
      env: {
        RUST_LOG: "test_server=debug,tower_http=debug",
        RUST_BACKTRACE: "1",
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