import { test as base, expect } from "@playwright/test"
import { tauriMockScript } from "../setup/tauri-mocks"

/**
 * Custom test fixture that automatically sets up Tauri mocks
 */
export const test = base.extend({
  page: async ({ page, context }, use, testInfo) => {
    // the console of the pages: every line live with DEBUG_E2E=1; otherwise their errors and uncaught exceptions, attached
    // to a failed test only, because an error-path test makes the app log its expected errors
    const pageErrors: string[] = []
    context.on("console", message => {
      if (process.env.DEBUG_E2E) {
        console.log("[Browser]", message.type(), message.text())
      } else if (message.type() === "error") {
        pageErrors.push(message.text())
      }
    })
    context.on("weberror", error => pageErrors.push(`Uncaught: ${error.error().stack ?? error.error().message}`))

    // Set debug flag if environment variable is set
    if (process.env.DEBUG_E2E) {
      await page.addInitScript(() => {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        ;(window as any).__DEBUG_E2E__ = true
      })
    }

    // Inject Tauri mocks before each navigation
    await page.addInitScript(tauriMockScript)

    // Set a fixed time for all tests to ensure consistent date/time display
    // Using Jan 2, 2024 10:00 AM UTC to match our test commits from Jan 1, 2024
    await context.clock.install({
      time: new Date("2024-01-02T10:00:00Z"),
    })

    // Use the page
    await use(page)

    if (testInfo.status !== testInfo.expectedStatus && pageErrors.length > 0) {
      await testInfo.attach("page errors", { body: pageErrors.join("\n"), contentType: "text/plain" })
    }
  },
})

export { expect }
