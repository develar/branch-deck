import { cpus } from "os"
import { defineConfig, devices } from "@playwright/test"
import { TEST_SERVER_PORT, TEST_SERVER_URL } from "./tests/e2e/helpers/test-server-url"

// DEBUG_E2E=1: the debug log of the mock server and every console line of the pages. Without it the server logs only
// warnings and errors: its debug log is 60k lines per run, and a full pipe blocks the server's threads on stdout
const debugE2E = !!process.env.DEBUG_E2E

export default defineConfig({
  testDir: "./tests/e2e",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : Math.floor(cpus().length * 0.8),
  globalSetup: "./tests/e2e/setup/global-setup.ts",

  // Output directories for test artifacts
  outputDir: "./tests/results",

  use: {
    baseURL: TEST_SERVER_URL,
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
      name: "test-server",
      // a running server is reused locally, e.g. `pnpm test-server:dev` that restarts on changes (needs watchexec)
      command: "cargo run -p test-server",
      port: TEST_SERVER_PORT,
      reuseExistingServer: !process.env.CI,
      stdout: "pipe",
      env: {
        // not RUST_LOG: .cargo/config.toml forces it to debug for `cargo run`
        TEST_SERVER_LOG: debugE2E ? "test_server=debug,tower_http=debug" : "warn",
        E2E_PORT: String(TEST_SERVER_PORT),
        RUST_BACKTRACE: "1",
      },
    },
  ],
})
