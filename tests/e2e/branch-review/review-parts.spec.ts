// @spec openspec/specs/branch/review-tour/spec.md
import type { Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { openPlanWindow, reviewSidebar, turnOnReviewPlan } from "../helpers/review-plan-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"

// the `review_parts` template: the branch `parts`, one commit that adds `Loader.probe`, uses it in `Config.kt`, removes
// `Loader.legacy` and adds an import; the story shows Loader.kt in three parts: the probe (step 1), the removal (Other
// changes) and the import (Imports only)

const LOADER = "src/plugins/Loader.kt"

/** The review window of the branch `parts` with the review plan on */
async function openParts(page: Page, setupRepo: (template: string) => Promise<TestRepositoryBuilder>, syncAndWaitForBranches: () => Promise<unknown>): Promise<Page> {
  turnOnReviewPlan(await setupRepo("review_parts"))
  return openPlanWindow(page, syncAndWaitForBranches, "parts")
}

test.describe("Parts of a file", () => {
  test("should show a file in parts as a card each, its part and its own lines, the footer the part at the top as J walks them, its row the first part, and a fold the changes other parts show, which it opens", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const window = await openParts(page, setupRepo, syncAndWaitForBranches)
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
    // the group of the imports comes after the story, in no step
    await expect(position).toHaveText("File 2 / 2 · part 3 of 3")

    // the fold of the second part names the probe of the first and the import of the third, the group by its name
    // alone, and opens each
    const targets = fold(1).getByTestId("diff-fold-elsewhere")
    await expect(targets).toHaveText(["1 change in Imports only", "1 change in step 1"])
    await targets.nth(1).click()
    await expect(position).toHaveText("Step 1 · File 2 / 2 · part 1 of 3")
    // the third part, a group of the imports, starts collapsed: its fold target opens it
    await expect(cards.nth(2)).not.toContainText("import java.util.Locale")
    await targets.first().click()
    await expect(cards.nth(2)).toContainText("import java.util.Locale")
  })

  test("should name in the bar above the diffs a step by its place, Other changes and the group of the imports after the story by their name alone, a click showing the opening of Other changes", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const window = await openParts(page, setupRepo, syncAndWaitForBranches)
    const { tree } = reviewSidebar(window)
    const bar = window.getByTestId("review-step-bar")
    const position = window.getByTestId("review-position")
    // the overview counts the one step, Other changes not
    await expect(bar).toHaveText("Overview·1 step")

    // the first part is of step 1: its place among the steps and its title
    await tree.getByRole("treeitem", { name: LOADER, exact: true }).click()
    await expect(bar.getByTestId("review-step-position")).toHaveText("Step 1 / 1")
    await expect(bar.getByTestId("review-step-title")).toHaveText("probe")

    // Other changes, at its opening and at its card, has no place: its name alone, and the footer no step
    await window.keyboard.press("j")
    await expect(bar).toHaveText("Other changes")
    await expect(bar.getByTestId("review-step-position")).toHaveCount(0)
    await window.keyboard.press("j")
    await expect(position).toHaveText("File 2 / 2 · part 2 of 3")
    await expect(bar).toHaveText("Other changes")

    // the group of the imports comes after the story, in no step: its name, without the count of its heading
    await window.keyboard.press("j")
    await expect(position).toHaveText("File 2 / 2 · part 3 of 3")
    await expect(bar).toHaveText("Imports only")
    await expect(bar).toHaveAttribute("title", "Imports only")

    // back at the second part, a click on the bar shows the opening of Other changes, where the footer names no part
    await window.keyboard.press("k")
    await expect(position).toHaveText("File 2 / 2 · part 2 of 3")
    await bar.click()
    await expect(position).toHaveText("File 2 / 2")
    await expect(window.getByTestId("review-concern-heading").filter({ hasText: "Other changes" })).toBeInViewport()
  })

  test(
    "should take a click on the lower half of the fold at the end of a card, the scrollbar of its long lines in a strip of its own below it",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const window = await openParts(page, setupRepo, syncAndWaitForBranches)
      // the old side narrower than the line of `load`: the first part has its scrollbar
      await window.setViewportSize({ width: 1100, height: 900 })
      const position = window.getByTestId("review-position")
      await reviewSidebar(window).tree.getByRole("treeitem", { name: LOADER, exact: true }).click()
      await expect(position).toHaveText("Step 1 · File 2 / 2 · part 1 of 3")
      const card = window.locator(`[data-testid=diff-view-file][data-path="${LOADER}"]`).first()
      const scrollbars = card.getByTestId("diff-view-scrollbars")
      await expect(scrollbars).toBeVisible()

      // the fold is the last row of the card: the scrollbars stand below it, so its lower half takes the click
      const place = card.getByTestId("diff-fold-elsewhere")
      await place.scrollIntoViewIfNeeded()
      const box = (await place.boundingBox())!
      expect((await scrollbars.boundingBox())!.y).toBeGreaterThanOrEqual(box.y + box.height)
      await place.click({ position: { x: box.width / 2, y: box.height * 0.85 } })
      // the removal of the second part is shown, under the opening of Other changes
      await expect(position).toHaveText("File 2 / 2")
      await expect(window.getByTestId("review-step-bar")).toHaveText("Other changes")
    },
  )

  test("should move the focus of the tree with the selection J and K move, so one row is marked", { tag: "@webkit" }, async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const window = await openParts(page, setupRepo, syncAndWaitForBranches)
    const { tree } = reviewSidebar(window)
    const selected = tree.getByRole("treeitem", { selected: true })
    const first = tree.getByRole("treeitem", { name: LOADER, exact: true })
    await first.click()
    await expect(first).toBeFocused()
    await expect(selected).toHaveCount(1)

    // J from the row: the opening of Other changes selects the row of Loader.kt there, which takes the focus
    await window.keyboard.press("j")
    const other = tree.getByRole("treeitem", { name: "Loader.kt, also in Other changes", exact: true })
    await expect(other).toHaveAttribute("aria-selected", "true")
    await expect(other).toBeFocused()
    await expect(first).not.toBeFocused()
    await expect(selected).toHaveCount(1)

    // K back to the first part: the focus with it
    await window.keyboard.press("k")
    await expect(first).toHaveAttribute("aria-selected", "true")
    await expect(first).toBeFocused()
    await expect(selected).toHaveCount(1)
  })

  test(
    "should scroll past the end of a short document, so J brings each section to the top, the bar, the footer and the tree naming it, and a wheel to the end shows the space below the last card",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const window = await openParts(page, setupRepo, syncAndWaitForBranches)
      // a view taller than all of the document below the first part
      await window.setViewportSize({ width: 1280, height: 1100 })
      const { tree } = reviewSidebar(window)
      const view = window.getByTestId("diff-view")
      const bar = window.getByTestId("review-step-bar")
      const position = window.getByTestId("review-position")
      const selected = tree.getByRole("treeitem", { selected: true })
      const cards = window.locator(`[data-testid=diff-view-file][data-path="${LOADER}"]`)
      // where a section stands below the top of the view, and how far that is from the top of a section, 10 px (the space
      // above the first card), to a pixel of the engine's rounding
      const below = async (element: typeof view) => (await element.boundingBox())!.y - (await view.boundingBox())!.y
      const offTop = async (element: typeof view) => Math.abs((await below(element)) - 10)
      await tree.getByRole("treeitem", { name: LOADER, exact: true }).click()
      await expect(bar).toHaveText("Step 1 / 1·probe")
      await expect.poll(() => offTop(cards.nth(0))).toBeLessThanOrEqual(1)

      // J: the opening of Other changes, the second part and the imports, each at the top, named by all three
      await window.keyboard.press("j")
      await expect(position).toHaveText("File 2 / 2")
      await expect.poll(() => offTop(window.getByTestId("diff-view-opening").filter({ hasText: "Other changes" }))).toBeLessThanOrEqual(1)
      await window.keyboard.press("j")
      await expect(position).toHaveText("File 2 / 2 · part 2 of 3")
      await expect(bar).toHaveText("Other changes")
      await expect(selected).toHaveAccessibleName("Loader.kt, also in Other changes")
      await expect.poll(() => offTop(cards.nth(1))).toBeLessThanOrEqual(1)
      await window.keyboard.press("j")
      await expect(position).toHaveText("File 2 / 2 · part 3 of 3")
      await expect(bar).toHaveText("Imports only")
      await expect(selected).toHaveAccessibleName(LOADER)
      await expect.poll(() => offTop(cards.nth(2))).toBeLessThanOrEqual(1)

      // the reader scrolls up, then to the end: the last card at the top again, the space past the end empty below it
      await view.hover()
      await window.mouse.wheel(0, -300)
      await expect(position).not.toHaveText("File 2 / 2 · part 3 of 3")
      await window.mouse.wheel(0, 100_000)
      await expect(position).toHaveText("File 2 / 2 · part 3 of 3")
      await expect(bar).toHaveText("Imports only")
      await expect.poll(() => offTop(cards.nth(2))).toBeLessThanOrEqual(1)
      const viewBottom = (await view.boundingBox())!.height
      const cardBottom = (await below(cards.nth(2))) + (await cards.nth(2).boundingBox())!.height
      expect(viewBottom - cardBottom).toBeGreaterThan(300)
    },
  )

  test(
    "should show the opening of the first step on the first J of a review just opened on its overview",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const window = await openParts(page, setupRepo, syncAndWaitForBranches)
      const bar = window.getByTestId("review-step-bar")
      await expect(bar).toHaveText("Overview·1 step")
      // nothing scrolled the diffs nor chose a section: J goes on from the overview at their top
      await window.getByRole("region", { name: "Changes of the commit" }).focus()
      await window.keyboard.press("j")
      await expect(bar).toHaveText("Step 1 / 1·probe")
      await expect(reviewSidebar(window).tree.getByRole("treeitem", { name: "1 · probe", exact: true })).toHaveAttribute("aria-selected", "true")
    },
  )
})
