// @spec openspec/specs/commit/commit-files/spec.md
import type { Locator, Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { expandBranch, findBranchRow, getBranchCommitRow } from "../helpers/branch-helpers"
import { changedParts, diffRows } from "../helpers/diff-helpers"
import { openCommitFilesSection, openCommitPreview } from "../helpers/preview-helpers"

/** The preview of the tree commit with the diff of RowMapper.kt open under its row of the tree */
async function openRowMapper(page: Page): Promise<{ preview: Locator; file: Locator }> {
  await expandBranch(page, findBranchRow(page, "session"))
  const preview = await openCommitPreview(page, getBranchCommitRow(page, "session", "a prompt that waits for its turn keeps the session row running"))
  const tree = await openCommitFilesSection(preview)
  const file = tree.getByRole("treeitem", { name: "RowMapper.kt" })
  await file.getByText("RowMapper.kt").click()
  return { preview, file }
}

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
    await expect(page.getByTestId("diff-view").first().locator("[data-testid=diff-row]").first()).toBeVisible()
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
        cut: width('a !=<span class="dv-changed dv-change-0" data-change="0">=</span> b') / cell,
        changed: 0,
      }
      const changed = host.lastElementChild!.querySelector("[data-change]")!
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

  test("shows the number of a changed line in a colour that reads on the colour of its line, light and dark", async ({ page }) => {
    const { file } = await openRowMapper(page)
    // Inline, the default: the line of the class is a deleted and an added row, the line of `map` the row of a pair
    await expect(diffRows(file, "deleted")).toHaveCount(1)
    /** The colour of the line number of the first row of a kind, and its contrast against the colour of its line (WCAG) */
    const number = (kind: "deleted" | "added" | "inline" | "context") =>
      diffRows(file, kind)
        .first()
        .evaluate(row => {
          const luminance = (colour: string) => {
            const [red, green, blue] = (colour.match(/[\d.]+/g) ?? []).slice(0, 3).map(value => {
              const channel = Number(value) / 255
              return channel <= 0.03928 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4
            })
            return 0.2126 * red! + 0.7152 * green! + 0.0722 * blue!
          }
          const side = row.querySelector("[data-testid=diff-side]")!
          const shown = [...side.querySelectorAll("[data-testid=diff-line-number]")].find(element => element.textContent !== "")!
          const colour = getComputedStyle(shown).color
          const [light, dark] = [luminance(colour), luminance(getComputedStyle(side).backgroundColor)].sort((a, b) => b - a)
          return { colour, contrast: (light! + 0.05) / (dark! + 0.05) }
        })
    for (const colorScheme of ["light", "dark"] as const) {
      await page.emulateMedia({ colorScheme })
      for (const kind of ["deleted", "added", "inline"] as const) {
        await expect.poll(async () => (await number(kind)).contrast, `${kind}, ${colorScheme}`).toBeGreaterThanOrEqual(3)
      }
      // the number of an unchanged line stays the quiet one of the editor
      expect((await number("context")).colour, colorScheme).not.toBe((await number("deleted")).colour)
    }
  })

  test("strikes the deleted parts of an Inline row through, and no change of Split or Unified", async ({ page }) => {
    const { preview, file } = await openRowMapper(page)
    // found and read in one call: the painter replaces a row when its tokens arrive, so an element found by one call
    // (`evaluateAll`) can be detached in the next, where its computed style is ""
    const decoration = (selector: string) =>
      file.evaluate((root, selector) => [...root.querySelectorAll(selector)].map(element => getComputedStyle(element).textDecorationLine), selector)

    // Inline, the default: nothing chooses it
    await expect(preview.getByRole("button", { name: "Inline", exact: true })).toHaveAttribute("aria-pressed", "true")
    // `Row = Row` becomes `SessionRow = SessionRow`: the deleted part stands before the inserted one
    const deleted = "[data-testid=diff-row][data-kind=inline] [data-inline-deleted]"
    await expect(file.locator(deleted).first()).toBeVisible()
    expect(new Set(await decoration(deleted))).toEqual(new Set(["line-through"]))

    for (const mode of ["Split", "Unified"]) {
      await preview.getByRole("button", { name: mode, exact: true }).click()
      // the rows of the mode, not the Inline rows before the switch
      await expect(diffRows(file, "inline")).toHaveCount(0)
      await expect(changedParts(file).first()).toBeVisible()
      expect(new Set(await decoration("[data-change]")), mode).toEqual(new Set(["none"]))
    }
  })
})
