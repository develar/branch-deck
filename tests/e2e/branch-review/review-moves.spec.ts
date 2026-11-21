// @spec openspec/specs/branch/review-units/spec.md
import type { Locator, Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { captureHtmlSnapshot } from "../helpers/aria-snapshot-helpers"
import { moveTargets } from "../helpers/diff-helpers"
import { openPlanWindow, reviewedPaths, reviewSidebar, turnOnReviewPlan } from "../helpers/review-plan-helpers"

// the `review_moves` template: the branch `moves`, two commits that move declarations whole between files, each the
// same code. Step 1 copies `formatTotal` of Orders.kt into Report.kt and extracts `parseTotal` and `parseSum`, one block
// of Orders.kt, into the new Parsing.kt; step 2 drops the old `formatTotal` and moves `legacyTotal` out of Legacy.kt,
// which it deletes, to the top of Orders.kt

/** The card of the section `key` of the story (`NarrativeSection.key`) */
function section(window: Page, key: string): Locator {
  return window.locator(`[data-testid="diff-view-file"][data-section="${key}"]`)
}

test.describe("Declarations moved whole between files", () => {
  test("should read a declaration moved to another file at its new place, its old place one row that opens it, in another step too, and name what moved into a new file and out of a deleted one", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    turnOnReviewPlan(await setupRepo("review_moves"))
    const window = await openPlanWindow(page, syncAndWaitForBranches, "moves")
    // step 1: the place parseTotal and parseSum left, the new file, the report; step 2: the deleted file, the orders
    await expect.poll(() => reviewedPaths(window)).toEqual(["src/a/Orders.kt", "src/a/Parsing.kt", "src/a/Report.kt", "src/a/Legacy.kt", "src/a/Orders.kt"])
    const [extracted, parsing, report, legacy, orders] = [
      section(window, "file:src/a/Orders.kt"),
      section(window, "src/a/Parsing.kt"),
      section(window, "file:src/a/Report.kt"),
      section(window, "src/a/Legacy.kt"),
      section(window, "file:src/a/Orders.kt#1"),
    ]
    const tree = reviewSidebar(window).tree
    const position = window.getByTestId("review-position")

    // the row of a file in the tree shows its section, whose rows the diffs paint once it is in view
    const show = async (name: string | RegExp) => {
      const row = tree.getByRole("treeitem", typeof name === "string" ? { name, exact: true } : { name })
      await row.click()
      await expect(row).toHaveAttribute("aria-selected", "true")
    }
    const extractedRow = /^src\/a\/Orders\.kt, also in 1 · /

    // where the code stands now it is read, under a heading that says where it came from: the moved lines show, with
    // the note of the marks of the diff where they start ("Moved from Orders.kt:7"); a note that starts the block of
    // the heading names the line it left alone, the heading says the file
    await show("src/a/Report.kt")
    await expect(report.getByTestId("diff-unit-heading")).toHaveText("formatTotal — moved from Orders.kt")
    await expect(report).toContainText("fun formatTotal(total: Int): String {")
    await expect(moveTargets(report)).toHaveText(["from Orders.kt:7"])
    await show("src/a/Parsing.kt")
    await expect(parsing.getByTestId("diff-unit-heading")).toHaveText("parseTotal and parseSum — moved from Orders.kt")
    await expect(parsing).toContainText("fun parseSum(text: String): Int {")
    await show("src/a/Orders.kt")
    await expect(position).toHaveText(/· Step 2 · .* · part 2 of 2$/)
    await expect(orders.getByTestId("diff-unit-heading")).toHaveText("legacyTotal — moved from Legacy.kt")
    await expect(orders).toContainText("fun legacyTotal(items: List<Int>): Int {")
    await expect(moveTargets(orders)).toHaveText(["line 4"])

    // where it left, one row and none of its lines: the two functions of one block, the deleted file whose comment,
    // package and function went with the move; the old format, after the change of the step that dropped it, is in
    // the fold at its end, which says the words of its row
    const droppedFormat = orders.getByTestId("diff-fold-elsewhere").filter({ hasText: "Moved" })
    await expect(droppedFormat).toHaveText("Moved formatTotal to Report.kt in step 1")
    await expect(orders).not.toContainText("fun formatTotal")
    await show("src/a/Legacy.kt")
    await expect(legacy.getByTestId("diff-unit-elsewhere")).toHaveText("Moved legacyTotal to Orders.kt")
    await expect(legacy).not.toContainText("The totals of the first release")
    await show(extractedRow)
    await expect(position).toHaveText(/· Step 1 · .* · part 1 of 2$/)
    const extractedMove = extracted.getByTestId("diff-unit-elsewhere").filter({ hasText: "Moved" })
    await expect(extractedMove).toHaveText("Moved parseTotal and parseSum to Parsing.kt")
    await expect(extracted).not.toContainText("fun parseTotal")
    await captureHtmlSnapshot(tree, "review-moves-tree")

    // N passes a row: from the top of the place parseTotal and parseSum left, the next change is the new file's
    await window.getByTestId("commit-review-code").locator("[aria-label='Changes of the commit']").focus()
    await window.keyboard.press("n")
    await expect(tree.getByRole("treeitem", { name: "src/a/Parsing.kt", exact: true })).toHaveAttribute("aria-selected", "true")

    // a click on each opens the section of where the code went, which the tree and the footer follow
    await show(extractedRow)
    await extractedMove.click()
    await expect(tree.getByRole("treeitem", { name: "src/a/Parsing.kt", exact: true })).toHaveAttribute("aria-selected", "true")
    await expect(position).toHaveText(/· Step 1 · /)
    await show("src/a/Legacy.kt")
    await legacy.getByTestId("diff-unit-elsewhere").click()
    await expect(tree.getByRole("treeitem", { name: "src/a/Orders.kt", exact: true })).toHaveAttribute("aria-selected", "true")
    await expect(position).toHaveText(/· Step 2 · .* · part 2 of 2$/)
    await droppedFormat.click()
    await expect(tree.getByRole("treeitem", { name: "src/a/Report.kt", exact: true })).toHaveAttribute("aria-selected", "true")
    await expect(position).toHaveText(/· Step 1 · /)
  })
})
