// @spec spec/branch/conflict-fix.spec.md
import { test, expect } from "../fixtures/test-fixtures"
import { collapseBranch, expandBranch, findBranchRow, getBranchCommitRow, getBranchDetailsRow, verifyCommitCount } from "../helpers/branch-helpers"
import { clickContextMenuItem, openContextMenu } from "../helpers/selection-helpers"
import { runAndWaitForSync } from "../helpers/sync-helpers"

test.describe("Conflict fix", () => {
  test.beforeEach(async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("conflict_fix")
    await syncAndWaitForBranches()
    await expect(findBranchRow(page, "ui").getByTestId("branch-status")).toHaveText("merge conflict")
  })

  test("should offer the move a trial build verified, apply it and undo it", async ({ page }) => {
    const uiRow = findBranchRow(page, "ui")
    if ((await uiRow.getAttribute("data-state")) !== "open") {
      await expandBranch(page, uiRow)
    }
    const fix = getBranchDetailsRow(page, "ui").getByTestId("conflict-fix")
    // the search runs when the conflict is shown
    await expect(fix).toHaveAttribute("data-state", "found")
    await expect(fix).toContainText("Move “Show the timeout in seconds” to api")
    await expect(fix.getByTestId("fix-result")).toHaveText("Afterwards ui applies cleanly.")
    // the conflict itself: the merge of settings.js in three columns, named by the sides. Both sides changed the same
    // word of the timeout, so the proposed result keeps the base there
    const file = getBranchDetailsRow(page, "ui").getByTestId("conflicting-files").getByTestId("file-diff")
    await expect(file.getByTestId("file-conflict-count")).toHaveText("1 conflict")
    await expect(file.getByTestId("merge-columns")).toContainText(/^(origin\/)?master/)
    await expect(file.getByTestId("merge-columns")).toContainText("“Show the timeout in seconds”")
    const conflict = file.locator(".dv-row[data-kind=merge]")
    await expect(conflict.locator('.dv-half[data-side="0"]')).toHaveText(/timeout = 10$/)
    await expect(conflict.locator('.dv-half[data-side="1"]')).toHaveText(/timeout = 30$/)
    await expect(conflict.locator('.dv-half[data-side="2"]')).toHaveText(/timeout = 30 \/\/ seconds$/)
    await expect(file.getByTestId("merge-notice")).toHaveCount(0)

    await runAndWaitForSync(page, () => fix.getByTestId("apply-conflict-fix").click())
    await expect(page.getByText("Fixed the conflict of ui")).toBeVisible()
    await expect(findBranchRow(page, "ui").getByTestId("branch-status")).not.toHaveText("merge conflict")
    await verifyCommitCount(findBranchRow(page, "api"), "2 commits")
    // the fix shows where the commit went
    await expect(getBranchCommitRow(page, "api", "Show the timeout in seconds")).toHaveAttribute("data-selected", "true")

    await runAndWaitForSync(page, () => page.getByRole("button", { name: "Undo" }).click())
    await verifyCommitCount(findBranchRow(page, "api"), "1 commit")
    await expect(findBranchRow(page, "ui").getByTestId("branch-status")).toHaveText("merge conflict")
  })

  test("should open the fix from the menu of the branch", async ({ page }) => {
    const uiRow = findBranchRow(page, "ui")
    if ((await uiRow.getAttribute("data-state")) === "open") {
      await collapseBranch(page, uiRow)
    }
    await openContextMenu(page, uiRow)
    await clickContextMenuItem(page, "Fix Conflict…")
    await expect(uiRow).toHaveAttribute("data-state", "open")
    await expect(getBranchDetailsRow(page, "ui").getByTestId("conflict-fix")).toHaveAttribute("data-state", "found")
  })
})
