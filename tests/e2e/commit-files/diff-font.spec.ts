// @spec spec/commit/commit-files.spec.md
import { test, expect } from "../fixtures/test-fixtures"
import { expandBranch, findBranchRow, getBranchCommitRow } from "../helpers/branch-helpers"
import { openCommitFilesSection, openCommitPreview } from "../helpers/preview-helpers"

/**
 * The claim of docs/design/ui-theme.md ("Diffs") that makes the ligatures of JetBrains Mono safe in a diff: a ligature
 * keeps the width of its cells, so the layout arithmetic holds, and the bound of a changed range inside a ligature
 * splits the text run, so the changed character stays visible
 */
test.describe("The font of the diffs", () => {
  test.beforeEach(async ({ setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("commit_tree")
    await syncAndWaitForBranches()
  })

  test("keeps the cell width of each character with ligatures on, also where a changed range cuts a ligature", async ({ page }) => {
    // a diff on the page: the styles of the rows load with the view
    await expandBranch(page, findBranchRow(page, "session"))
    const preview = await openCommitPreview(page, getBranchCommitRow(page, "session", "a prompt that waits for its turn keeps the session row running"))
    const tree = await openCommitFilesSection(preview)
    await tree.getByRole("treeitem", { name: "RowMapper.kt" }).getByText("RowMapper.kt").click()
    await expect(page.getByTestId("diff-view").first().locator(".dv-row").first()).toBeVisible()
    const measured = await page.evaluate(async () => {
      await document.fonts.load('13px "JetBrains Mono Variable"')
      const host = document.createElement("div")
      host.className = "dv-rows"
      document.body.append(host)
      const width = (html: string) => {
        const span = document.createElement("span")
        span.style.whiteSpace = "pre"
        span.innerHTML = html
        host.append(span)
        return span.getBoundingClientRect().width
      }
      const cell = width("x")
      const result = {
        cell,
        features: getComputedStyle(host).fontFeatureSettings,
        notEqual: width("a != b") / cell,
        arrow: width("a -> b") / cell,
        strictNotEqual: width("a !== b") / cell,
        // `!=` to `!==`: the inserted `=` is a range of its own inside the ligature
        cut: width('a !=<span class="dv-changed dv-change-0">=</span> b') / cell,
        changed: 0,
      }
      const changed = host.lastElementChild!.querySelector(".dv-changed")!
      result.changed = changed.getBoundingClientRect().width / cell
      host.remove()
      return result
    })
    // the ligatures are on: the default features of the font
    expect(measured.features).toBe("normal")
    expect(measured.notEqual).toBeCloseTo(6, 1)
    expect(measured.arrow).toBeCloseTo(6, 1)
    expect(measured.strictNotEqual).toBeCloseTo(7, 1)
    expect(measured.cut).toBeCloseTo(7, 1)
    // the changed character has the width of a cell: it stays visible
    expect(measured.changed).toBeCloseTo(1, 1)
  })
})
