// @spec spec/branch/branch-row.spec.md
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow } from "../helpers/branch-helpers"

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
})
