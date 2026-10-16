// @spec spec/branch/branch-row.spec.md
import { test, expect } from "../fixtures/test-fixtures"
import { expandBranch, findBranchRow, getBranchDetailsRow } from "../helpers/branch-helpers"

test.describe("Branch summary", () => {
  test("shows the description title of a branch as written, in the row and the preview", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("long_subjects")
    await syncAndWaitForBranches()

    // `docs` has a description (`bd describe`): its title is the summary, its conventional prefix not dimmed
    const docsRow = findBranchRow(page, "docs")
    const title = docsRow.getByTestId("branch-summary")
    await expect(title).toHaveText("docs: fix the typo in the README")
    await expect(title.locator(".font-mono")).toHaveCount(0)
    // a branch without one shows its subject, the prefix dimmed
    await expect(findBranchRow(page, "IJPL-215166").getByTestId("branch-summary").locator(".font-mono")).toHaveText("test:")

    await docsRow.hover()
    await page.keyboard.press("Space")
    const preview = page.getByTestId("branch-preview")
    await expect(preview.getByRole("heading")).toHaveText("docs")
    await expect(preview.getByTestId("branch-summary")).toHaveText("docs: fix the typo in the README")
  })

  test("leaves out the issue key the row shows already, in the title and in the commits", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("issue_split")
    await syncAndWaitForBranches()

    // the title of ABC-123 starts with its key, as a pull request title does: the name shows it
    const row = findBranchRow(page, "ABC-123")
    await expect(row.getByTestId("branch-summary")).toHaveText("Parse the source in one pass")
    await expandBranch(page, row)
    const commits = getBranchDetailsRow(page, "ABC-123").locator("[data-row-id]")
    await expect(commits).toHaveCount(3)
    for (const [index, subject] of ["Add tokens", "Add lexer", "Add parser"].entries()) {
      await expect(commits.nth(index)).toContainText(subject)
      await expect(commits.nth(index)).not.toContainText("ABC-123")
    }

    // the preview has room: the title as written
    await row.hover()
    await page.keyboard.press("Space")
    await expect(page.getByTestId("branch-preview").getByTestId("branch-summary")).toHaveText("ABC-123 Parse the source in one pass")
  })
})
