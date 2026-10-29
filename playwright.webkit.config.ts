import { defineConfig, devices } from "@playwright/test"
import base from "./playwright.config"

// The drag and drop tests in WebKit, the engine of the app on macOS (WKWebView), beside Chromium of `pnpm e2e`, and the
// tests tagged `@webkit`, of the focus and the popups, where WebKit differs (it drops the focus of a hidden element
// only at a later layout): `pnpm e2e:webkit`, by hand after a change of a drag, of the focus or of a popup. The snapshots
// are Chromium's, so none is compared here. What only WKWebView does (the drag image, the autoscroll, the outline of a
// row) is the check of the real app (docs/guidelines/real-app-checks.md).
export default defineConfig({
  ...base,
  projects: [{ name: "webkit", use: { ...devices["Desktop Safari"] } }],
  testMatch: [
    "move-commits/**/*.spec.ts",
    "parked-branches/**/*.spec.ts",
    "stacked-branches/stacked-branches.spec.ts",
    "landing/statistics.spec.ts",
    "command-palette/command-palette.spec.ts",
    "youtrack/issue-popover.spec.ts",
  ],
  grep: [/drag|dropped|new branch zone/i, /@webkit/],
  ignoreSnapshots: true,
})
