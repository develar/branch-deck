import { defineConfig, devices } from "@playwright/test"
import base from "./playwright.config"

// The images of the README (`pnpm screenshots:readme`): the setup and the server of the e2e tests, WebKit, the engine
// of the app on macOS, at 2x; tests/screenshots/readme.shot.ts writes them into docs/screenshots
export default defineConfig({
  ...base,
  projects: [{ name: "webkit", use: { ...devices["Desktop Safari"], viewport: { width: 1100, height: 1000 } } }],
  testDir: "./tests/screenshots",
  testMatch: "**/*.shot.ts",
  fullyParallel: false,
  workers: 1,
  retries: 0,
})
