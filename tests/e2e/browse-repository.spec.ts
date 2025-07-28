import { test, expect } from "./fixtures/test-fixtures"
import { captureAriaSnapshot } from "./helpers/aria-snapshot-helpers"
import type { Page } from "@playwright/test"

/**
 * Helper function to test browse repository functionality
 */
async function testBrowseRepository(
  page: Page,
  setupRepo: (template: string, options?: { prepopulateStore?: boolean }) => Promise<unknown>,
  template: string,
  expectedValid: boolean,
  snapshotSuffix: string,
) {
  // Don't prepopulate store for browse tests - we want to test the browsing functionality
  await setupRepo(template, { prepopulateStore: false })

  // Wait for the page to load
  await page.waitForLoadState("networkidle")

  // Look for the browse repository button
  const browseButton = page.getByTestId("browse-repository-button")
  await expect(browseButton).toBeVisible()

  // Capture initial state (before clicking browse) - same for both tests
  await captureAriaSnapshot(page.locator("body"), "browse-repository-initial")

  // Click the browse repository button
  await browseButton.click()

  if (expectedValid) {
    // For valid repository, wait for sync button to be enabled
    await expect(page.locator("[data-testid=\"sync-button\"]")).toBeEnabled({ timeout: 10000 })

    // Capture final state (valid repository selected)
    await captureAriaSnapshot(page.locator("body"), `browse-repository-${snapshotSuffix}`)
  }
  else {
    // For invalid repository, wait for error message
    await expect(page.locator("text=/Not a git repository/")).toBeVisible({ timeout: 10000 })

    // Capture final state (invalid repository error)
    await captureAriaSnapshot(page.locator("body"), `browse-repository-${snapshotSuffix}`)
  }
}

test.describe("Browse Repository", () => {
  test("should show error for non-git directory", async ({ page, setupRepo }) => {
    await testBrowseRepository(page, setupRepo, "empty-non-git", false, "invalid-selected")
  })

  test("should successfully select valid git repository", async ({ page, setupRepo }) => {
    await testBrowseRepository(page, setupRepo, "simple", true, "valid-selected")
  })
})