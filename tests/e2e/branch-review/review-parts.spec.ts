// @spec openspec/specs/branch/review-tour/spec.md
import { test, expect } from "../fixtures/test-fixtures"
import { openPlanWindow, reviewSidebar, turnOnReviewPlan } from "../helpers/review-plan-helpers"

// the `review_parts` template: the branch `parts`, one commit that adds `Loader.probe`, uses it in `Config.kt`, removes
// `Loader.legacy` and adds an import; the story shows Loader.kt in three parts: the probe (step 1), the removal (Other
// changes) and the import (Imports only)

const LOADER = "src/plugins/Loader.kt"

test.describe("Parts of a file", () => {
  test("should show a file in parts as a card each, its part and its own lines, the footer the part at the top as J walks them, its row the first part, and a fold the changes other parts show, which it opens", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("review_parts")
    turnOnReviewPlan(repo)
    const window = await openPlanWindow(page, syncAndWaitForBranches, "parts")
    const cards = window.locator(`[data-testid=diff-view-file][data-path="${LOADER}"]`)
    // each card its part and the lines of its own blocks: the probe added, the legacy count removed alone, the import
    await expect(cards.getByTestId("review-file-part")).toHaveText(["part 1 of 3+5 −0", "part 2 of 3+0 −4", "part 3 of 3+1 −0"])
    // a file the story shows in one section keeps the counts of the file
    await expect(window.locator('[data-testid=diff-view-file][data-path="src/plugins/Config.kt"]').getByTestId("review-file-part")).toHaveCount(0)
    const position = window.getByTestId("review-position")

    // the row of the file shows its first part, the footer names it
    await reviewSidebar(window).tree.getByRole("treeitem", { name: LOADER, exact: true }).click()
    await expect(position).toHaveText("Step 1 · File 2 / 2 · part 1 of 3")
    // the fold at the end of the first part counts its unchanged lines and names the removal the second part shows
    const fold = (card: number) =>
      cards
        .nth(card)
        .locator(".dv-fold")
        .filter({ has: window.getByTestId("diff-fold-elsewhere") })
    await expect(fold(0).getByRole("button", { name: "Expand 9 unchanged lines in Loader" })).toBeVisible()
    await expect(fold(0).getByTestId("diff-fold-elsewhere")).toHaveText("1 change in Other changes")

    // J walks the parts, past the opening of Other changes, the footer with them
    await window.keyboard.press("j")
    await window.keyboard.press("j")
    await expect(position).toHaveText(/File 2 \/ 2 · part 2 of 3$/)
    await window.keyboard.press("j")
    await expect(position).toHaveText("Step 1 · File 2 / 2 · part 3 of 3")

    // the fold of the second part names the probe of the first and the import of the third, and opens each
    const targets = fold(1).getByTestId("diff-fold-elsewhere")
    await expect(targets).toHaveText(["1 change in Imports only (1)", "1 change in step 1"])
    await targets.nth(1).click()
    await expect(position).toHaveText("Step 1 · File 2 / 2 · part 1 of 3")
    // the third part, a group of the imports, starts collapsed: its fold target opens it
    await expect(cards.nth(2)).not.toContainText("import java.util.Locale")
    await targets.first().click()
    await expect(cards.nth(2)).toContainText("import java.util.Locale")
  })
})
