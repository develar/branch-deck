// @spec spec/commit/drop.spec.md
import type { Locator, Page } from "@playwright/test"
import { expect, test } from "../fixtures/test-fixtures"
import { captureHtmlSnapshot } from "../helpers/aria-snapshot-helpers"
import { expandBranch, findBranchRow, getBranchCommitRow, getUnassignedCommitRow, verifyCommitCount } from "../helpers/branch-helpers"
import { clickContextMenuItem, openContextMenu, selectCommit } from "../helpers/selection-helpers"
import { runAndWaitForSync } from "../helpers/sync-helpers"
import { closeToast } from "../helpers/toast-helpers"
import { routeCommandError } from "../helpers/wait-helpers"

function toast(page: Page, title: string): Locator {
  return page.locator("[data-slot=toast]").filter({ hasText: title })
}

/** The confirmation of the drop, its button focused, so Enter confirms */
async function confirmation(page: Page, button: string): Promise<Locator> {
  const form = page.getByTestId("drop-commits-form")
  await expect(form.getByRole("button", { name: button })).toBeFocused()
  return form
}

test.describe("Drop commits", () => {
  test("should drop a commit and its file from its menu and bring them back with the Undo of its toast", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("drop_commits")
    await syncAndWaitForBranches()
    const authRow = findBranchRow(page, "auth")
    await verifyCommitCount(authRow, 2)
    await expandBranch(page, authRow)

    await openContextMenu(page, getBranchCommitRow(page, "auth", "Check the password"))
    await clickContextMenuItem(page, "Drop Commit…")
    const form = await confirmation(page, "Drop Commit")
    await expect(form.getByTestId("drop-commits").locator("li")).toHaveText([/Check the password/])
    await expect(form).toContainText("This removes 1 commit and its changes from HEAD")
    await captureHtmlSnapshot(form, "drop-commit-confirmation")

    await runAndWaitForSync(page, () => page.keyboard.press("Enter"))
    const dropped = toast(page, "Dropped 1 commit")
    await expect(dropped).toBeVisible()
    await expect(form).toHaveCount(0)
    await verifyCommitCount(authRow, 1)
    await expect(getBranchCommitRow(page, "auth", "Check the password")).toHaveCount(0)
    // the file left with the commit: nothing is left as an uncommitted change
    await expect(page.getByTestId("uncommitted-changes-card")).toHaveCount(0)

    await runAndWaitForSync(page, () => dropped.getByRole("button", { name: "Undo" }).click())
    await verifyCommitCount(authRow, 2)
    await expect(page.getByTestId("uncommitted-changes-card")).toHaveCount(0)
  })

  test("should drop the selected commit with the key, and say which branch it leaves without a commit", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("drop_commits")
    await syncAndWaitForBranches()
    // the revert of the docs commit has no prefix: it waits in the unassigned commits
    const revert = getUnassignedCommitRow(page, "Describe the accounts")
    await selectCommit(revert)
    await page.keyboard.press("Backspace")
    await confirmation(page, "Drop Commit")
    await runAndWaitForSync(page, () => page.keyboard.press("Enter"))
    await closeToast(toast(page, "Dropped 1 commit"))
    await expect(revert).toHaveCount(0)

    // the docs commit is the only one of its branch
    const docsRow = findBranchRow(page, "docs")
    await expandBranch(page, docsRow)
    await openContextMenu(page, getBranchCommitRow(page, "docs", "Describe the accounts"))
    await clickContextMenuItem(page, "Drop Commit…")
    const form = await confirmation(page, "Drop Commit")
    // Escape closes the confirmation and drops nothing
    await page.keyboard.press("Escape")
    await expect(form).toHaveCount(0)
    await expect(docsRow).toHaveCount(1)

    await openContextMenu(page, getBranchCommitRow(page, "docs", "Describe the accounts"))
    await clickContextMenuItem(page, "Drop Commit…")
    await confirmation(page, "Drop Commit")
    await runAndWaitForSync(page, () => page.keyboard.press("Enter"))
    await expect(toast(page, "Dropped 1 commit")).toContainText("docs has no commits left")
    await expect(docsRow).toHaveCount(0)
  })

  test("should keep the commits and say why when the drop is refused", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("drop_commits")
    await syncAndWaitForBranches()
    const authRow = findBranchRow(page, "auth")
    await expandBranch(page, authRow)
    // the drop is the next rewrite of the engine the page runs
    await routeCommandError(
      page,
      "execute",
      { code: "drop_conflict", message: "Removing commit 1a2b3c4 conflicts with later commits on master: they depend on its changes" },
      { times: 1 },
    )
    await openContextMenu(page, getBranchCommitRow(page, "auth", "Check the password"))
    await clickContextMenuItem(page, "Drop Commit…")
    await confirmation(page, "Drop Commit")
    await page.keyboard.press("Enter")
    await expect(toast(page, "Cannot drop the commits")).toContainText("they depend on its changes")
    await verifyCommitCount(authRow, 2)
  })
})
