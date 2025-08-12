import { test, expect } from "../fixtures/test-fixtures"
import { captureHtmlSnapshot } from "../helpers/aria-snapshot-helpers"
import { syncBranches, waitForSyncComplete } from "../helpers/sync-helpers"
import { openContextMenu, clickContextMenuItem } from "../helpers/selection-helpers"
import { inlineDeleteConfirmation, testInputAutoFocus, submitInlineForm } from "../helpers/inline-form-helpers"

// Basic foundation test for ArchivedBranchTableCard
// Sets up a repo with archived branches and snapshots the card

test.describe("Archived Branches Table", () => {
  test.beforeEach(async ({ setupRepo, page }) => {
    await setupRepo("archived_branches")
    // Custom sync that waits for archived branches instead of regular branches
    await syncBranches(page, ["[data-testid=\"archived-branches-card\"]"])
  })

  test("should render archived branches card", async ({ page }) => {
    const card = page.getByTestId("archived-branches-card")
    await expect(card).toBeVisible()

    // Initial collapsed snapshot
    await captureHtmlSnapshot(card, "archived-branches-card-initial")

    // Expand the card by clicking the header title
    await card.getByText("Archived Branches", { exact: true }).click()

    // Wait for table body to render (header cells become visible)
    await expect(card.locator("thead")).toBeVisible()

    // Capture expanded snapshot
    await captureHtmlSnapshot(card, "archived-branches-card-expanded")
  })

  test("should delete archived branch via context menu", async ({ page }) => {
    const card = page.getByTestId("archived-branches-card")
    await expect(card).toBeVisible()

    // Expand the card by clicking the header title
    await card.getByText("Archived Branches", { exact: true }).click()

    // Wait for table body to render
    await expect(card.locator("thead")).toBeVisible()

    // Find the branch row for feature-partial (this branch exists in the archived_branches template)
    const branchRow = page.locator("tr[data-branch-name=\"user-name/archived/2025-01-11/feature-partial\"]")
    await expect(branchRow).toBeVisible()

    // Right-click to open context menu
    await openContextMenu(page, branchRow)

    // Click "Delete Archived Branch" from context menu
    await clickContextMenuItem(page, "Delete Archived Branch")

    // Verify the inline delete confirmation input appears
    await inlineDeleteConfirmation.waitForVisible(page)

    // Capture HTML snapshot of the delete confirmation form
    const deleteForm = inlineDeleteConfirmation.getPortal(page)
    await captureHtmlSnapshot(deleteForm, "archived-branch-delete-confirmation-form")

    // Test that input is focused
    const branchNameInput = inlineDeleteConfirmation.getInput(page)
    await testInputAutoFocus(branchNameInput)

    // Type the branch name to confirm deletion (use simple branch name without prefix)
    await submitInlineForm(branchNameInput, "feature-partial")

    // Wait for form to disappear after deletion
    await inlineDeleteConfirmation.waitForHidden(page)

    // Wait for the automatic sync to complete
    await waitForSyncComplete(page)

    // Capture HTML snapshot of the archived branches card after deletion
    await captureHtmlSnapshot(card, "archived-branches-after-deletion")

    // Verify the deleted branch is no longer in the list
    await expect(page.locator("tr[data-branch-name=\"user-name/archived/2025-01-11/feature-partial\"]")).not.toBeVisible()
  })
})
