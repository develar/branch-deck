import { defineConfig, devices } from "@playwright/test"
import base from "./playwright.config"

// The benchmarks of tests/perf (`pnpm perf:diff`): the setup and the server of the e2e tests, one worker, so the runs do
// not share the CPU. Chromium and WebKit, the engine of the app on macOS (WKWebView): `--project=webkit` runs one
export default defineConfig({
  ...base,
  projects: [
    { name: "chromium", use: { ...devices["Desktop Chrome"] } },
    { name: "webkit", use: { ...devices["Desktop Safari"] } },
  ],
  testDir: "./tests/perf",
  testMatch: "**/*.perf.ts",
  fullyParallel: false,
  workers: 1,
  retries: 0,
})
