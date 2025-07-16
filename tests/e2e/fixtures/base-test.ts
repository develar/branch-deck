import { test as base } from "@playwright/test"
import { tauriMockScript } from "../setup/tauri-mocks"

/**
 * Custom test fixture that automatically sets up Tauri mocks
 */
export const test = base.extend({
  page: async ({ page }, use) => {
    // Inject Tauri mocks before each navigation
    await page.addInitScript(tauriMockScript)

    // Use the page
    await use(page)
  },
})

export { expect } from "@playwright/test"