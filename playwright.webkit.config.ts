import { defineConfig, devices } from "@playwright/test"
import base from "./playwright.config"

// The drag and drop tests in WebKit, the engine of the app on macOS (WKWebView), beside Chromium of `pnpm e2e`:
// `pnpm e2e:webkit`, by hand after a change of a drag. The snapshots are Chromium's, so none is compared here. What only
// WKWebView does (the drag image, the autoscroll, the outline of a row) is the check of the real app
// (docs/guidelines/real-app-checks.md).
export default defineConfig({
  ...base,
  projects: [{ name: "webkit", use: { ...devices["Desktop Safari"] } }],
  testMatch: ["move-commits/**/*.spec.ts", "parked-branches/**/*.spec.ts", "stacked-branches/stacked-branches.spec.ts"],
  grep: /drag|dropped|new branch zone/i,
  ignoreSnapshots: true,
})
