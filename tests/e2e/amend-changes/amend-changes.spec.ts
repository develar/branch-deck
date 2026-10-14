// @spec spec/commit/commit-row.spec.md
import type { Locator, Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { waitForBranchSyncComplete } from "../helpers/sync-helpers"
import { openContextMenu, clickContextMenuItem } from "../helpers/selection-helpers"
import { expandBranch, findBranchRow, isBranchProcessing } from "../helpers/branch-helpers"
import { waitForTooltip } from "../helpers/wait-helpers"
import { captureModalSnapshot } from "../helpers/aria-snapshot-helpers"

/** Opens the amend changes dialog of `branchName` from its context menu and returns the branch row */
async function openAmendDialog(page: Page, branchName: string): Promise<Locator> {
  const branchRow = findBranchRow(page, branchName)
  await waitForBranchSyncComplete(page, branchName)

  await openContextMenu(page, branchRow)
  // the menu item only shows once the commits are loaded
  await page.waitForSelector("text=Amend Changes")
  await clickContextMenuItem(page, "Amend Changes")

  await page.waitForSelector("[data-testid=\"uncommitted-changes-card\"]", { timeout: 10000 })
  return branchRow
}

test.describe("Amend Changes Feature", () => {
  test.beforeEach(async ({ setupRepo, syncAndWaitForBranches }) => {
    // Setup test repository with amend_changes template
    await setupRepo("amend_changes")

    // Sync branches and wait for them to load
    await syncAndWaitForBranches()
  })

  test("should open amend changes dialog via context menu", async ({ page }) => {
    await openAmendDialog(page, "feature-auth")

    // Wait for uncommitted changes to finish loading (wait for spinner to disappear)
    await page.waitForSelector("text=Loading uncommitted changes…", { state: "detached", timeout: 10000 })

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
    const branchRow = await openAmendDialog(page, "feature-api")

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
    expect(await isBranchProcessing(branchRow)).toBe(false)
  })

  test("should cancel amend changes with Escape key", async ({ page }) => {
    const branchRow = await openAmendDialog(page, "feature-auth")

    // Press Escape key to cancel
    await page.keyboard.press("Escape")

    // Verify the dialog is closed (give it more time as dialog animations might take longer)
    await page.waitForSelector("[data-testid=\"uncommitted-changes-card\"]", { state: "detached", timeout: 10000 })

    // Branch row should not show processing state
    expect(await isBranchProcessing(branchRow)).toBe(false)
  })

  test("should cancel amend changes with Cancel button", async ({ page }) => {
    const branchRow = await openAmendDialog(page, "feature-api")

    // Click the Cancel button
    const cancelButton = page.locator("button:has-text(\"Cancel\")")
    await cancelButton.click()

    // Verify the dialog is closed
    await page.waitForSelector("[data-testid=\"uncommitted-changes-card\"]", { state: "detached", timeout: 5000 })

    // Branch row should not show processing state
    expect(await isBranchProcessing(branchRow)).toBe(false)
  })

  test("should show uncommitted changes details", async ({ page }) => {
    await openAmendDialog(page, "feature-auth")

    // Verify the test repo has uncommitted changes
    const hasChangesElements = await page.locator("[data-testid=\"uncommitted-changes-card\"]").isVisible()
    expect(hasChangesElements).toBe(true) // Should show changes content

    // Verify the Amend Changes button is enabled when changes are present
    const amendButton = page.getByRole("dialog").locator("button:has-text(\"Amend Changes\")")
    await expect(amendButton).toBeEnabled() // Should be enabled with changes
  })

  test("should mark the first commit as the amend target while the form is open", async ({ page }) => {
    const branchRow = findBranchRow(page, "feature-auth")
    await waitForBranchSyncComplete(page, "feature-auth")
    await expandBranch(page, branchRow)

    // the commit list follows the branch row; while the form is open, the form row sits between them
    const commitRow = page.locator("tr[data-row-id]").filter({ hasText: "Add authentication service" })
    const badge = commitRow.getByText("Amend target", { exact: true })
    await expect(commitRow).toBeVisible()
    await expect(badge).toHaveCount(0)

    await openContextMenu(page, branchRow)
    await clickContextMenuItem(page, "Amend Changes")
    await page.waitForSelector("[data-testid=\"uncommitted-changes-card\"]")
    await expect(badge).toBeVisible()

    await badge.hover()
    await waitForTooltip(page, "Changes will be amended to this commit")

    await page.mouse.move(0, 0)
    await page.locator("button:has-text(\"Cancel\")").click()
    await page.waitForSelector("[data-testid=\"uncommitted-changes-card\"]", { state: "detached", timeout: 5000 })
    await expect(badge).toHaveCount(0)
  })
})
