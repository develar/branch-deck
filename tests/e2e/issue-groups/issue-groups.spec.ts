// @spec spec/branch/issue-groups.spec.md
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow, verifyCommitCount } from "../helpers/branch-helpers"
import { submitInlineForm } from "../helpers/inline-form-helpers"
import { clickContextMenuItem, openContextMenu } from "../helpers/selection-helpers"

test.describe("Issue groups", () => {
  test.beforeEach(async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("issue_groups")
    await syncAndWaitForBranches()
    await verifyCommitCount(findBranchRow(page, "parser"), 2)
  })

  test("should rename an issue group without a rewrite and undo the rename", async ({ page }) => {
    await expect(findBranchRow(page, "ABC-1")).toHaveCount(0)
    await openContextMenu(page, findBranchRow(page, "parser"))
    await clickContextMenuItem(page, "Rename…")
    const renameInput = page.getByTestId("inline-rename-input").locator('input[type="text"]')
    await expect(renameInput).toHaveValue("parser")
    await expect(page.getByTestId("inline-rename-input")).toContainText("Renames the issue group: the commit messages stay as they are")
    await submitInlineForm(renameInput, "grammar")
    await verifyCommitCount(findBranchRow(page, "grammar"), 2)
    await expect(findBranchRow(page, "parser")).toHaveCount(0)

    await page.getByRole("button", { name: "Undo" }).click()
    await verifyCommitCount(findBranchRow(page, "parser"), 2)
    await expect(findBranchRow(page, "grammar")).toHaveCount(0)
  })
})
