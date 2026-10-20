// @spec spec/branch/branch-row.spec.md
import { test, expect } from "../fixtures/test-fixtures"
import { captureBranchTableSnapshot, captureContextMenuSnapshot } from "../helpers/aria-snapshot-helpers"
import { findBranchRow, getBranchCells, expandBranch, collapseBranch, verifyBranchState, verifyBranchStatus, getMoreButton, verifyCommitCount } from "../helpers/branch-helpers"
import { pressCopyKey, testContextMenuCopy, readClipboard } from "../helpers/clipboard-helpers"
import { openBranchPreview } from "../helpers/review-status-helpers"
import { openContextMenu } from "../helpers/selection-helpers"
import { getOpenedSubWindows } from "../helpers/sub-window-helpers"
import { waitForBranchSyncComplete } from "../helpers/sync-helpers"
import { holdCommand } from "../helpers/wait-helpers"

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
    await verifyBranchStatus(branchRow, "not pushed")
    await expect(actions).toBeVisible()

    // no copy button holds a place in the actions: ⌘C and the menu copy the name
    await expect(getMoreButton(branchRow)).toBeVisible()
    await expect(actions.getByRole("button", { name: /^Copy/ })).toHaveCount(0)

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

  test("should copy the full branch name with ⌘C on the focused row, the key hint of Copy Full Branch Name", async ({ page }) => {
    const branchRow = findBranchRow(page, "test-branch")
    await waitForBranchSyncComplete(page, "test-branch")

    await openContextMenu(page, branchRow)
    await expect(page.getByRole("menuitem", { name: "Copy Full Branch Name" }).locator("kbd")).not.toHaveCount(0)
    await page.keyboard.press("Escape")
    await expect(page.getByRole("menu")).toHaveCount(0)

    await branchRow.focus()
    await pressCopyKey(page)
    await expect.poll(() => readClipboard(page)).toMatch(/^[\w-]+\/virtual\/test-branch$/)
  })

  test("should open the branch menu from the more actions button without toggling the row", async ({ page }) => {
    const branchRow = findBranchRow(page, "test-branch")
    await waitForBranchSyncComplete(page, "test-branch")

    await branchRow.getByRole("button", { name: "More actions for test-branch" }).click()
    // the menu of the right-click
    await expect(page.getByRole("menuitem", { name: "Rename…" })).toBeVisible()
    await testContextMenuCopy(page, "Copy Branch Name", "test-branch")
    await expect(page.getByRole("menu")).toHaveCount(0)
    await verifyBranchState(branchRow, "closed")
  })

  test("should keep the actions of the branch menu during a sync, the rewrites disabled with the reason", async ({ page }) => {
    const branchRow = findBranchRow(page, "test-branch")
    await waitForBranchSyncComplete(page, "test-branch")
    // a sync that runs while the menu is open
    const releaseSync = await holdCommand(page, "sync_branches")
    const syncButton = page.getByTestId("sync-button")
    await syncButton.click()
    await expect(syncButton).toHaveAttribute("aria-busy", "true")

    await openContextMenu(page, branchRow)
    const rename = page.getByRole("menuitem", { name: "Rename…" })
    await expect(rename).toBeDisabled()
    await rename.hover()
    await expect(page.getByRole("tooltip")).toHaveText("Wait for the sync to finish")
    // a review rewrites nothing: it opens during the sync
    await page.getByRole("menuitem", { name: "Review Changes" }).click()
    await expect.poll(async () => (await getOpenedSubWindows(page)).map(window => window.url)).toEqual([expect.stringMatching(/branch-review/)])
    releaseSync()
    await expect(syncButton).not.toHaveAttribute("aria-busy", "true")
  })

  test("should offer the menu of the row in the branch preview, and close the preview for a form of the row", async ({ page }) => {
    await waitForBranchSyncComplete(page, "test-branch")
    const preview = await openBranchPreview(page, "test-branch")
    await preview.getByRole("button", { name: "More actions for test-branch" }).click()
    // the items of the row's menu; M is no key of the preview: no hint
    const move = page.getByRole("menuitem", { name: "Move to…" })
    await expect(move).toBeVisible()
    await expect(move.locator("kbd")).toHaveCount(0)
    await page.getByRole("menuitem", { name: "Rename…" }).click()
    await expect(preview).toHaveCount(0)
    await expect(page.getByTestId("inline-rename-input")).toBeVisible()
  })

  test("should handle branch with commits state", async ({ page }) => {
    const branchRow = findBranchRow(page, "test-branch")

    await waitForBranchSyncComplete(page, "test-branch")

    // Check actions column: the push and the branch menu ("Safe Push" only on Space)
    const { actions } = getBranchCells(branchRow)
    const buttons = actions.locator("button")
    await expect(buttons).toHaveCount(2)
    await expect(buttons.nth(1)).toHaveAccessibleName("More actions for test-branch")

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
