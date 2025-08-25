/**
 * Tests for saved repository path validation.
 * These tests simulate scenarios where a previously saved repository path becomes invalid
 * (e.g., directory deleted, moved, or inaccessible) and verify proper error handling.
 */
import { test, expect } from "../fixtures/test-fixtures"
import { captureHtmlSnapshot } from "../helpers/aria-snapshot-helpers"

test.describe("Repository Validation", () => {
  test("should display error for non-existent repository path", async ({ page, setupRepo }) => {
    // Use NO_REPO template which creates a path that doesn't exist on disk
    // This will cause get_branch_prefix_from_git_config to return an error
    await setupRepo("NO_REPO", {
      prepopulateStore: true, // Auto-select the non-existent repository
      createRecentProject: true, // Create recent project entry for validation testing
    })

    // Wait for error alert to appear

    // Debug: Check current page state
    const allTestIds = await page.locator("[data-testid]").all()
    console.log(`Found ${allTestIds.length} elements with data-testid`)
    for (const element of allTestIds) {
      const testId = await element.getAttribute("data-testid")
      const text = await element.textContent()
      console.log(`Element with testid="${testId}": "${text?.substring(0, 100)}"`)
    }

    // Debug: Check if error alert exists but is hidden
    const errorAlert = page.getByTestId("error-alert")
    const isVisible = await errorAlert.isVisible().catch(() => false)
    const exists = await errorAlert.count()
    console.log(`Error alert - exists: ${exists > 0}, visible: ${isVisible}`)

    // Debug: Check the alert conditions more directly
    const allAlerts = await page.locator("div[role='alert'], .alert, [data-testid*='alert']").all()
    console.log(`Found ${allAlerts.length} alert-like elements`)
    for (const alert of allAlerts) {
      const text = await alert.textContent()
      const classes = await alert.getAttribute("class")
      console.log(`Alert: "${text?.substring(0, 100)}" (classes: ${classes})`)
    }

    // Check that the error alert is displayed
    await expect(errorAlert).toBeVisible({ timeout: 10000 })

    // Check that the error message mentions repository not being accessible
    await expect(errorAlert).toContainText("Repository not accessible")

    // Capture snapshot showing the error state
    await captureHtmlSnapshot(page.locator("body"), "repository-validation-error")

    // Verify that the sync button is disabled when repository is invalid
    const syncButton = page.getByTestId("sync-button")
    await expect(syncButton).toBeDisabled()
  })

  test("should clear error when valid repository is selected", async ({ page, setupRepo }) => {
    // First, setup an invalid repository to create error state
    await setupRepo("NO_REPO", {
      prepopulateStore: true,
      createRecentProject: true, // Create recent project entry for validation testing
    })

    // Wait for error to appear
    const errorAlert = page.getByTestId("error-alert")
    await expect(errorAlert).toBeVisible({ timeout: 5000 })

    // Now switch to a valid repository
    await setupRepo("simple", {
      prepopulateStore: true,
    })

    // Error should be gone and sync button should be enabled
    await expect(errorAlert).not.toBeVisible()
    const syncButton = page.getByTestId("sync-button")
    await expect(syncButton).toBeEnabled()

    // Capture snapshot showing the cleared error state
    await captureHtmlSnapshot(page.locator("body"), "repository-validation-cleared")
  })
})