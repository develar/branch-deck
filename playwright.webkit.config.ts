import { defineConfig, devices } from "@playwright/test"
import base from "./playwright.config"

// The drag and drop tests in WebKit, the engine of the app on macOS (WKWebView), beside Chromium of `pnpm e2e`, and the
// tests tagged `@webkit` for focus, popups, the item under a resting pointer, branch-column widths and review
// navigation, where WebKit differs. It drops focus of a hidden element only at a later layout and times
// `ResizeObserver` reports its own way.
// Run `pnpm e2e:webkit` by hand after changes to a drag, focus, popup, column or review navigation. Snapshots are
// Chromium's, so none is compared here. What only WKWebView does (the drag image, the autoscroll, the outline of a row)
// is the check of the real app (docs/contributing/real-app-checks.md).
export default defineConfig({
  ...base,
  projects: [{ name: "webkit", use: { ...devices["Desktop Safari"] } }],
  testMatch: [
    "move-commits/**/*.spec.ts",
    "parked-branches/**/*.spec.ts",
    "declared-needs/**/*.spec.ts",
    "stacked-branches/stacked-branches.spec.ts",
    "landing/statistics.spec.ts",
    "next-step/facts-without-answer.spec.ts",
    "command-palette/command-palette.spec.ts",
    "startup/warm-start.spec.ts",
    "youtrack/issue-popover.spec.ts",
    "branch-row-layout/branch-row-layout.spec.ts",
    "branch-row-layout/column-widths.spec.ts",
    "branch-row-layout/focus-outline.spec.ts",
    "repository/repository-lock.spec.ts",
    "repository/companion-links.spec.ts",
    "branch-review/review-plan.spec.ts",
    "branch-review/branch-review.spec.ts",
    "branch-review/review-tour.spec.ts",
    "branch-review/review-groups.spec.ts",
    "branch-review/review-parts.spec.ts",
    "review-requests/review-requests.spec.ts",
    "review-window/review-window.spec.ts",
    "commit-files/commit-review.spec.ts",
    "commit-files/quiet-changes.spec.ts",
    "commit-files/symbol-navigation.spec.ts",
    "commit-message/commit-message.spec.ts",
  ],
  grep: [/drag|dropped|new branch zone/i, /@webkit/],
  ignoreSnapshots: true,
})
