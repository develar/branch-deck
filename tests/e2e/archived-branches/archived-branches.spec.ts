// @spec spec/branch/branch-row.spec.md
import type { Locator, Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { captureHtmlSnapshot } from "../helpers/aria-snapshot-helpers"
import { expandBranch } from "../helpers/branch-helpers"
import { pressCopyKey, readClipboard, testCopyButtonOnHover } from "../helpers/clipboard-helpers"
import { inlineDeleteConfirmation, testInputAutoFocus, submitInlineForm } from "../helpers/inline-form-helpers"
import { openContextMenu, clickContextMenuItem } from "../helpers/selection-helpers"
import { finishedSyncCount, runAndWaitForUpdate, syncBranches } from "../helpers/sync-helpers"
import { holdCommand, recordCommand, waitForCollapsibleCardExpanded } from "../helpers/wait-helpers"

// Basic foundation test for ArchivedBranchTableCard
// Sets up a repo with archived branches and snapshots the card

/** Expands the archived card: the row of the partially merged `feature-partial` */
async function expandToPartialRow(page: Page): Promise<Locator> {
  const card = page.getByTestId("archived-branches-card")
  await card.getByText("Archived Branches", { exact: true }).click()
  await waitForCollapsibleCardExpanded(card)
  return page.locator('tr[data-branch-name="user-name/archived/2025-01-11/feature-partial"]')
}

test.describe("Archived Branches Table", () => {
  test.beforeEach(async ({ setupRepo, page }) => {
    await setupRepo("archived_branches")
    // Custom sync that waits for archived branches instead of regular branches
    await syncBranches(page, ['[data-testid="archived-branches-card"]'])
  })

  test("should render archived branches card", async ({ page }) => {
    const card = page.getByTestId("archived-branches-card")
    await expect(card).toBeVisible()

    // Initial collapsed snapshot
    await captureHtmlSnapshot(card, "archived-branches-card-initial")

    // Expand the card by clicking the header title
    await card.getByText("Archived Branches", { exact: true }).click()

    // Wait for the card to be fully expanded (animation complete)
    await waitForCollapsibleCardExpanded(card)

    // Capture expanded snapshot
    await captureHtmlSnapshot(card, "archived-branches-card-expanded")
  })

  test("should explain archiving and cleanup in the help after the title, the header still toggling around it", async ({ page }) => {
    const card = page.getByTestId("archived-branches-card")
    const help = card.getByTestId("archived-branches-help-icon")
    await expect(help).toHaveAccessibleName("About Archived Branches")
    await help.hover()
    const popover = page.locator("[data-slot=popover-content]")
    await expect(popover.locator("[data-slot=popover-title]")).toHaveText("Archived Branches")
    await expect(popover).toContainText("A merged one is deleted by the first sync 7 days after it was archived; the others stay until you delete them.")
    // the help is no part of the toggle: hovering or clicking it leaves the card collapsed (a click keeps it open)
    await help.click()
    await expect(card).toHaveAttribute("data-state", "closed")
    await page.keyboard.press("Escape")
    await expect(popover).toBeHidden()
    // the empty end of the header row toggles too: one toggle button covers the row
    const header = card.locator("[data-slot=collapsible-trigger]")
    await expect(header).toHaveCount(1)
    const box = (await card.boundingBox())!
    await page.mouse.click(box.x + box.width - 40, box.y + 20)
    await waitForCollapsibleCardExpanded(card)
    await expect(header).toHaveAttribute("aria-expanded", "true")
  })

  test("should name the integration status in words, with the counts in the tooltip", async ({ page }) => {
    const card = page.getByTestId("archived-branches-card")
    // the collapsed card says that work may be lost: the branches the baseline has none of the commits of
    await expect(card.getByTestId("archived-not-merged")).toHaveText("1 not merged")
    await expect(card.getByTestId("archived-not-merged")).toHaveClass(/text-warning/)
    await card.getByText("Archived Branches", { exact: true }).click()
    await waitForCollapsibleCardExpanded(card)

    const status = (name: string) => card.locator(`tr[data-branch-name="user-name/archived/2025-01-11/${name}"]`).getByTestId("archived-branch-status")
    await expect(status("feature-full")).toHaveAttribute("data-status", "merged")
    await expect(status("feature-full")).toHaveText("Merged")
    await expect(status("feature-partial")).toHaveAttribute("data-status", "partial")
    await expect(status("feature-partial")).toHaveText("Partially merged · 1 of 2")
    await expect(status("feature-pending")).toHaveAttribute("data-status", "not-merged")
    await expect(status("feature-pending")).toHaveText("Not merged")

    await status("feature-pending").getByText("Not merged").hover()
    await expect(page.getByTestId("archived-branch-status-tooltip")).toHaveText("1 commit, none merged; the original commits are no longer in HEAD")
  })

  test("should show empty state alert when only archived branches exist", async ({ page }) => {
    // The beforeEach already sets up "archived_branches" template and syncs

    // Verify empty state appears and capture its HTML structure
    const emptyState = page.locator('[data-testid="empty-state-alert"]')
    await expect(emptyState).toBeVisible()

    // Capture HTML snapshot of the empty state alert
    await captureHtmlSnapshot(emptyState, "empty-state-with-archived-branches")

    // Also capture the full page to show the empty state in context with archived branches below
    await captureHtmlSnapshot(page.locator('[data-testid="branch-creator-root"]'), "branch-creator-with-empty-state-and-archived")
  })

  test("should delete an archived branch from its menu, the row gone with no sync after it", async ({ page }) => {
    const card = page.getByTestId("archived-branches-card")
    await expect(card).toBeVisible()

    // Expand the card by clicking the header title
    await card.getByText("Archived Branches", { exact: true }).click()

    // Wait for the card to be fully expanded (animation complete)
    await waitForCollapsibleCardExpanded(card)

    // Find the branch row for feature-partial (this branch exists in the archived_branches template)
    const branchRow = page.locator('tr[data-branch-name="user-name/archived/2025-01-11/feature-partial"]')
    await expect(branchRow).toBeVisible()

    const syncs = await recordCommand(page, "sync")
    const synced = await finishedSyncCount(page)
    // Right-click to open context menu
    await openContextMenu(page, branchRow)

    // Click "Delete Archived Branch…" from the context menu
    await clickContextMenuItem(page, "Delete Archived Branch…")

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

    // the patch of the session removes the row: no sync runs for a deletion
    await expect(page.locator("[data-slot=toast-title]", { hasText: "Deleted the archived branch feature-partial" })).toBeVisible()
    await expect(page.locator('tr[data-branch-name="user-name/archived/2025-01-11/feature-partial"]')).not.toBeVisible()
    await expect(page.locator('tr[data-branch-name="user-name/archived/2025-01-11/feature-full"]')).toBeVisible()
    expect(syncs).toEqual([])
    await expect(page.getByTestId("sync-button")).toHaveAttribute("data-sync-count", String(synced))

    // Capture HTML snapshot of the archived branches card after deletion
    await captureHtmlSnapshot(card, "archived-branches-after-deletion")
  })

  test("should show the loading of the commits of an expanded row once, below it", async ({ page }) => {
    const card = page.getByTestId("archived-branches-card")
    await card.getByText("Archived Branches", { exact: true }).click()
    await waitForCollapsibleCardExpanded(card)
    const row = card.locator("tr[data-branch-name]").first()
    // a detected row (a placeholder has no status yet): no spinner in it
    await expect(row.getByTestId("archived-branch-status")).toBeVisible()
    await expect(row.getByRole("status")).toHaveCount(0)

    const releaseLoad = await holdCommand(page, "get_archived_branch_commits")
    await expandBranch(page, row)
    await expect(card.getByText("Loading commits…")).toBeVisible()
    // the line below is the progress: the row itself doesn't pulse
    await expect(row).not.toHaveClass(/animate-pulse/)
    releaseLoad()
    await expect(card.getByText("Loading commits…")).toHaveCount(0)
  })

  test("should open the menu of an archived row from its more actions button", async ({ page }) => {
    const branchRow = await expandToPartialRow(page)

    await branchRow.getByRole("button", { name: "More actions for feature-partial" }).click()
    // the menu of the right-click
    await expect(page.getByRole("menuitem", { name: "Copy Full Branch Name" })).toBeVisible()
    await page.getByRole("menuitem", { name: /^Delete Archived Branch/ }).click()
    await inlineDeleteConfirmation.waitForVisible(page)
    // neither the button nor the menu toggles the row
    await expect(branchRow).toHaveAttribute("data-state", "closed")
  })

  test("should copy the full name of an archived branch with ⌘C on its focused row", async ({ page }) => {
    const branchRow = await expandToPartialRow(page)
    // no copy button: the key and the menu copy it
    await expect(branchRow.getByRole("button", { name: "More actions for feature-partial" })).toBeVisible()
    await expect(branchRow.getByRole("button", { name: /^Copy/ })).toHaveCount(0)

    await branchRow.focus()
    await pressCopyKey(page)
    await expect.poll(() => readClipboard(page)).toBe("user-name/archived/2025-01-11/feature-partial")
  })
})

test.describe("Archived branch merged by a baseline commit", () => {
  test("should show the copy button of the merged-by commit on hover", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // the `landed_upstream` template: the update lands feature-auth (a robot pushed it, committer `Patronus`), so it
    // moves to the archive, merged by the robot's commit
    await setupRepo("landed_upstream")
    await syncAndWaitForBranches()
    await runAndWaitForUpdate(page, () => page.getByTestId("update-project-button").click())

    const card = page.getByTestId("archived-branches-card")
    await card.getByText("Archived Branches", { exact: true }).click()
    await waitForCollapsibleCardExpanded(card)
    const row = card.locator('tr[data-branch-name$="/feature-auth"]')
    // a placeholder can't expand until the integration status came
    await expect(row.getByTestId("archived-branch-status")).toHaveAttribute("data-status", "merged")
    await expandBranch(page, row)

    const line = card.getByTestId("archived-branch-integration-commit")
    await expect(line).toHaveText(/^Merged as [0-9a-f]{8} by Patronus$/)
    await testCopyButtonOnHover(page, line, /^[0-9a-f]{40}$/, "Copy the commit hash")
  })
})
