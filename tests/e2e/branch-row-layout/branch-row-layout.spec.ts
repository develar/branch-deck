import { test, expect } from "../fixtures/test-fixtures"
import { waitForBranchSyncComplete } from "../helpers/sync-helpers"
import { openContextMenu } from "../helpers/selection-helpers"
import { findBranchRow, getBranchCells, expandBranch, collapseBranch, verifyBranchState, getCopyButton, verifyCommitCount } from "../helpers/branch-helpers"
import { captureBranchTableSnapshot, captureContextMenuSnapshot } from "../helpers/aria-snapshot-helpers"
import { testCopyButton, testContextMenuCopy, readClipboard } from "../helpers/clipboard-helpers"

test.describe("Branch Row Layout and Copy Functionality", () => {
  test.beforeEach(async ({ setupRepo, syncAndWaitForBranches }) => {
    // Setup test repository with simple template
    await setupRepo("simple")

    // Sync branches and wait for them to load
    await syncAndWaitForBranches()
  })

  test("should display correct branch row structure", async ({ page }) => {
    // Find the branch row
    const branchRow = findBranchRow(page, "test-branch")

    // Wait for the branch to finish syncing
    await waitForBranchSyncComplete(page, "test-branch")

    // Verify all columns are present
    const { branchName, status, actions } = getBranchCells(branchRow)

    await expect(branchName).toContainText("test-branch")
    await verifyCommitCount(branchRow, "commit")
    await expect(status).toBeVisible()
    await expect(actions).toBeVisible()

    // Verify copy button is present in actions column
    const copyButton = getCopyButton(branchRow)
    await expect(copyButton).toBeVisible()
    await expect(copyButton).toHaveClass(/text-muted/)

    // Verify expand button is present
    const expandButton = branchName.locator("button").first()
    await expect(expandButton).toBeVisible()

    // The branch should be collapsed after sync (auto-expand is disabled)
    await verifyBranchState(branchRow, "closed")

    // Capture ARIA snapshot of standard (collapsed) branch row
    await captureBranchTableSnapshot(page, branchRow, "branch-row-standard")
  })

  test("should show expanded branch row with commits", async ({ page }) => {
    const branchRow = findBranchRow(page, "test-branch")

    await waitForBranchSyncComplete(page, "test-branch")

    // Manually expand the branch
    await expandBranch(page, branchRow)

    // Verify commits are visible in the expanded state
    const expandedRow = page.locator("tr").filter({ hasText: "foo" }).and(page.locator("tr:not([data-testid])"))
    await expect(expandedRow.first()).toBeVisible()

    // Capture ARIA snapshot of expanded branch row
    await captureBranchTableSnapshot(page, branchRow, "branch-row-expanded")

    // Test collapse/expand behavior
    await collapseBranch(page, branchRow)

    // Wait for content to be hidden
    await expect(expandedRow.first()).not.toBeVisible()

    // Expand again
    await expandBranch(page, branchRow)

    // Wait for content to be visible again
    await expect(expandedRow.first()).toBeVisible()
  })

  test("should display context menu with copy options", async ({ page }) => {
    const branchRow = findBranchRow(page, "test-branch")

    await waitForBranchSyncComplete(page, "test-branch")

    // Right-click to open context menu
    await openContextMenu(page, branchRow)

    // Verify both copy options are present
    const copyBranchName = page.getByRole("menuitem", { name: "Copy Branch Name" })
    const copyFullBranchName = page.getByRole("menuitem", { name: "Copy Full Branch Name" })

    await expect(copyBranchName).toBeVisible()
    await expect(copyFullBranchName).toBeVisible()

    // Capture ARIA snapshot of context menu
    await captureContextMenuSnapshot(page, "branch-row-context-menu")
  })

  test("should copy branch name via context menu", async ({ page }) => {
    const branchRow = findBranchRow(page, "test-branch")

    await waitForBranchSyncComplete(page, "test-branch")

    // Test Copy Branch Name
    await openContextMenu(page, branchRow)
    await testContextMenuCopy(page, "Copy Branch Name", "test-branch")

    // Test Copy Full Branch Name
    await openContextMenu(page, branchRow)
    await testContextMenuCopy(page, "Copy Full Branch Name", /^[\w-]+\/virtual\/test-branch$/)
  })

  test("should copy via copy button in actions column", async ({ page }) => {
    const branchRow = findBranchRow(page, "test-branch")

    await waitForBranchSyncComplete(page, "test-branch")

    // Find the copy button
    const copyButton = getCopyButton(branchRow)

    // Test copy button with tooltip feedback
    await testCopyButton(page, copyButton, /^[\w-]+\/virtual\/test-branch$/)
  })

  test("should verify copy button visual states and icon changes", async ({ page }) => {
    const branchRow = findBranchRow(page, "test-branch")

    await waitForBranchSyncComplete(page, "test-branch")

    // Find the copy button
    const copyButton = getCopyButton(branchRow)

    // Verify initial state - button should be visible (alwaysVisible: true)
    await expect(copyButton).toBeVisible()
    await expect(copyButton).toHaveClass(/text-muted/)

    // Verify initial icon is copy icon
    const iconElement = copyButton.locator("span[class*='i-lucide']")
    await expect(iconElement).toHaveClass(/i-lucide:copy/)
    await expect(iconElement).not.toHaveClass(/i-lucide:copy-check/)

    // Click the button to trigger copy
    await copyButton.click()

    // Verify icon changes to check mark
    await expect(iconElement).toHaveClass(/i-lucide:copy-check/)
    await expect(iconElement).not.toHaveClass(/i-lucide:copy(?!-check)/)

    // Verify clipboard content was copied
    const clipboardText = await readClipboard(page)
    expect(clipboardText).toMatch(/^[\w-]+\/virtual\/test-branch$/)

    // Wait for timeout + buffer (200ms + 50ms buffer)
    await page.waitForTimeout(250)

    // Verify button reverts to initial state after timeout
    await expect(iconElement).toHaveClass(/i-lucide:copy/)
    await expect(iconElement).not.toHaveClass(/i-lucide:copy-check/)

    // First verify the "Copied!" tooltip is no longer visible
    const copiedTooltip = page.locator("[role=\"tooltip\"]:has-text(\"Copied!\")")
    await expect(copiedTooltip).not.toBeVisible()

    // Move mouse away from button first
    await page.mouse.move(0, 0)

    // Then hover again to verify original tooltip shows
    await copyButton.hover()

    // Wait for tooltip with proper text
    const revertedTooltip = page.locator("[role=\"tooltip\"]:has-text(\"Copy full branch name\")")
    await expect(revertedTooltip).toBeVisible()
  })

  test("should handle branch with commits state", async ({ page }) => {
    const branchRow = findBranchRow(page, "test-branch")

    await waitForBranchSyncComplete(page, "test-branch")

    // Check actions column - should have both copy and push buttons
    const { actions } = getBranchCells(branchRow)
    const buttons = actions.locator("button")

    // Should have 2 buttons: copy and push
    await expect(buttons).toHaveCount(2)

    // Capture HTML snapshot of the branch row with commits state
    await captureBranchTableSnapshot(page, branchRow, "branch-with-commits-state")
  })

  test("should verify final branch state after all tests", async ({ page }) => {
    const branchRow = findBranchRow(page, "test-branch")

    await waitForBranchSyncComplete(page, "test-branch")

    // Verify the branch row is still functional
    await expect(branchRow).toBeVisible()

    // Verify we can still access the context menu
    await openContextMenu(page, branchRow)
    const copyOption = page.getByRole("menuitem", { name: "Copy Branch Name" })
    await expect(copyOption).toBeVisible()

    // Close context menu
    await page.keyboard.press("Escape")
  })
})
