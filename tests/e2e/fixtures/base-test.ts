import { test as base } from "@playwright/test"
import { tauriMockScript } from "../setup/tauri-mocks"

/**
 * Custom test fixture that automatically sets up Tauri mocks
 */
export const test = base.extend({
  page: async ({ page, context }, use) => {
    // Set debug flag if environment variable is set
    if (process.env.DEBUG_E2E) {
      await page.addInitScript(() => {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        (window as any).__DEBUG_E2E__ = true
      })
    }

    // Set test configuration for faster timeouts
    await page.addInitScript(() => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (globalThis as any).__BRANCH_DECK_TEST_CONFIG__ = {
        copyTimeout: 200, // Use 200ms timeout in tests instead of 2000ms
      }
    })

    // Inject Tauri mocks before each navigation
    await page.addInitScript(tauriMockScript)

    // Set a fixed time for all tests to ensure consistent date/time display
    // Using Jan 2, 2024 10:00 AM UTC to match our test commits from Jan 1, 2024
    await context.clock.install({
      time: new Date("2024-01-02T10:00:00Z"),
    })

    // Use the page
    await use(page)
  },
})

export { expect } from "@playwright/test"
