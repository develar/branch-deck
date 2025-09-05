import { test, expect } from "../fixtures/test-fixtures"
import { waitForBranchSyncComplete } from "../helpers/sync-helpers"
import { openContextMenu, clickContextMenuItem } from "../helpers/selection-helpers"
import { findBranchRow, isBranchProcessing } from "../helpers/branch-helpers"
import { captureModalSnapshot } from "../helpers/aria-snapshot-helpers"

test.describe("Amend Changes Feature", () => {
  test.beforeEach(async ({ setupRepo, syncAndWaitForBranches }) => {
    // Setup test repository with amend_changes template
    await setupRepo("amend_changes")

    // Sync branches and wait for them to load
    await syncAndWaitForBranches()
  })

  test("should open amend changes dialog via context menu", async ({ page }) => {
    // Find the branch row
    const branchRow = findBranchRow(page, "feature-auth")

    // Wait for the branch to finish syncing
    await waitForBranchSyncComplete(page, "feature-auth")

    // Right-click to open context menu
    await openContextMenu(page, branchRow)

    // Wait for the menu item to appear - it only shows if commits are loaded
    await page.waitForSelector("text=Amend Changes")

    // Click the menu item
    await clickContextMenuItem(page, "Amend Changes")

    // The amend changes dialog should appear
    await page.waitForSelector("[data-testid=\"uncommitted-changes-card\"]", { timeout: 10000 })

    // Wait for uncommitted changes to finish loading (wait for spinner to disappear)
    await page.waitForSelector("text=Loading uncommitted changes...", { state: "detached", timeout: 10000 })

    // Verify the dialog title
    const dialogTitle = await page.textContent("h1, h2, [role=\"heading\"]")
    expect(dialogTitle).toContain("Amend Changes to feature-auth")

    // Verify the dialog appears with uncommitted changes loaded
    // Wait for the Amend Changes button to be enabled (indicates data is loaded)
    const amendButton = page.getByRole("dialog").locator("button:has-text(\"Amend Changes\")")
    await expect(amendButton).toBeEnabled({ timeout: 10000 })

    // Capture HTML snapshot of the dialog structure
    await captureModalSnapshot(page, "amend-changes-dialog")
  })

  test("should submit amend changes and verify success", async ({ page }) => {
    // Find the branch row
    const branchRow = findBranchRow(page, "feature-api")

    // Wait for the branch to finish syncing
    await waitForBranchSyncComplete(page, "feature-api")

    // Right-click to open context menu and select amend changes
    await openContextMenu(page, branchRow)
    await page.waitForSelector("text=Amend Changes")
    await clickContextMenuItem(page, "Amend Changes")

    // Wait for the dialog to appear
    await page.waitForSelector("[data-testid=\"uncommitted-changes-card\"]")

    // The Amend Changes button should be enabled when there are uncommitted changes
    const amendButton = page.getByRole("dialog").locator("button:has-text(\"Amend Changes\")")
    await amendButton.waitFor({ state: "visible", timeout: 10000 })
    await expect(amendButton).toBeEnabled() // Should be enabled with changes

    // Test submission by clicking the button
    await amendButton.click()

    // Wait for the operation to complete and dialog to close
    await page.waitForSelector("[data-testid=\"uncommitted-changes-card\"]", { state: "detached", timeout: 10000 })
    await page.keyboard.press("Escape")

    // Verify the dialog is closed
    await page.waitForSelector("[data-testid=\"uncommitted-changes-card\"]", { state: "detached", timeout: 10000 })

    // Check that the branch is no longer processing
    const isProcessing = await isBranchProcessing(branchRow)
    expect(isProcessing).toBe(false)
  })

  test("should cancel amend changes with Escape key", async ({ page }) => {
    // Find the branch row
    const branchRow = findBranchRow(page, "feature-auth")

    // Wait for the branch to finish syncing
    await waitForBranchSyncComplete(page, "feature-auth")

    // Right-click to open context menu and select amend changes
    await openContextMenu(page, branchRow)
    await page.waitForSelector("text=Amend Changes")
    await clickContextMenuItem(page, "Amend Changes")

    // Wait for the dialog to appear
    await page.waitForSelector("[data-testid=\"uncommitted-changes-card\"]")

    // Press Escape key to cancel
    await page.keyboard.press("Escape")

    // Verify the dialog is closed (give it more time as dialog animations might take longer)
    await page.waitForSelector("[data-testid=\"uncommitted-changes-card\"]", { state: "detached", timeout: 10000 })

    // Branch row should not show processing state
    const isProcessing = await isBranchProcessing(branchRow)
    expect(isProcessing).toBe(false)
  })

  test("should cancel amend changes with Cancel button", async ({ page }) => {
    // Find the branch row
    const branchRow = findBranchRow(page, "feature-api")

    // Wait for the branch to finish syncing
    await waitForBranchSyncComplete(page, "feature-api")

    // Right-click to open context menu and select amend changes
    await openContextMenu(page, branchRow)
    await page.waitForSelector("text=Amend Changes")
    await clickContextMenuItem(page, "Amend Changes")

    // Wait for the dialog to appear
    await page.waitForSelector("[data-testid=\"uncommitted-changes-card\"]")

    // Click the Cancel button
    const cancelButton = page.locator("button:has-text(\"Cancel\")")
    await cancelButton.click()

    // Verify the dialog is closed
    await page.waitForSelector("[data-testid=\"uncommitted-changes-card\"]", { state: "detached", timeout: 5000 })

    // Branch row should not show processing state
    const isProcessing = await isBranchProcessing(branchRow)
    expect(isProcessing).toBe(false)
  })

  test("should show uncommitted changes details", async ({ page }) => {
    // Find the branch row
    const branchRow = findBranchRow(page, "feature-auth")

    // Wait for the branch to finish syncing
    await waitForBranchSyncComplete(page, "feature-auth")

    // Right-click to open context menu and select amend changes
    await openContextMenu(page, branchRow)
    await page.waitForSelector("text=Amend Changes")
    await clickContextMenuItem(page, "Amend Changes")

    // Wait for the dialog to appear
    await page.waitForSelector("[data-testid=\"uncommitted-changes-card\"]")

    // Verify the test repo has uncommitted changes
    const hasChangesElements = await page.locator("[data-testid=\"uncommitted-changes-card\"]").isVisible()
    expect(hasChangesElements).toBe(true) // Should show changes content

    // Verify the Amend Changes button is enabled when changes are present
    const amendButton = page.getByRole("dialog").locator("button:has-text(\"Amend Changes\")")
    await expect(amendButton).toBeEnabled() // Should be enabled with changes
  })
})