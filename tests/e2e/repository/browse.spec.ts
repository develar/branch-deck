import type { Page } from "@playwright/test"
/**
 * Tests for the browse repository dialog functionality.
 * These tests simulate user clicking "Browse Repository" button and selecting directories.
 */
import { test, expect } from "../fixtures/test-fixtures"
import { captureHtmlSnapshot } from "../helpers/aria-snapshot-helpers"
import { waitForSessionView } from "../helpers/sync-helpers"

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

  // Wait for the browse repository button to be visible (indicates page is ready)
  const browseButton = page.getByTestId("browse-repository-button")
  await expect(browseButton).toBeVisible()

  // First run (no repository, no recent projects): the welcome card asks for the repository, it renders shortly after
  // the header
  await expect(page.getByTestId("welcome-card").getByRole("heading", { name: "Select a Git repository" })).toBeVisible()
  // the work on its way is of a selected repository, as Activity is: no landing indicator in the header yet
  const header = page.getByTestId("configuration-header")
  await expect(header.getByTestId("sync-button")).toBeVisible()
  await expect(header.getByTestId(/^landing-indicator/)).toHaveCount(0)

  // Capture initial state (before clicking browse) - same for both tests
  await captureHtmlSnapshot(page.locator("body"), "browse-repository-initial")

  // Click the browse repository button
  await browseButton.click()

  if (expectedValid) {
    // For valid repository, wait for sync button to be enabled
    await expect(page.locator('[data-testid="sync-button"]')).toBeEnabled()
    // the card and its line about coding agents come with the first view of the session of the repository
    await waitForSessionView(page)

    // The template configures a branch prefix: once it is loaded, the welcome card is gone
    await expect(page.getByTestId("welcome-card")).toHaveCount(0)
    // Sync is enabled while the branch prefix still loads; the card the snapshot shows comes once it is loaded, its line
    // about coding agents once the status of the Agent CLI and the rule of the repository answered
    await expect(page.getByTestId("ready-to-sync-card").getByTestId("agents-hint")).toBeVisible()
    // the selected repository has its landing indicator: empty, in its place, before the first work comes
    await expect(header.getByTestId("landing-indicator-empty")).toBeDisabled()

    // Capture final state (valid repository selected)
    await captureHtmlSnapshot(page.locator("body"), `browse-repository-${snapshotSuffix}`)
  } else {
    // For invalid repository, wait for error message
    await expect(page.getByTestId("error-alert")).toContainText("Not a Git repository")

    // the alert explains the problem: the welcome card does not repeat the steps next to it
    await expect(page.getByTestId("welcome-card")).toHaveCount(0)

    // Capture final state (invalid repository error)
    await captureHtmlSnapshot(page.locator("body"), `browse-repository-${snapshotSuffix}`)
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
