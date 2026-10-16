// @spec spec/branch/conflict-fix.spec.md
import { test, expect } from "../fixtures/test-fixtures"
import { expandBranch, findBranchRow, getBranchCommitRow, getBranchDetailsRow, verifyCommitCount } from "../helpers/branch-helpers"
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
})
