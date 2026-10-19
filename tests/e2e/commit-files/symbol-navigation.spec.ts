// @spec spec/commit/symbol-navigation.spec.md
import type { Locator, Page } from "@playwright/test"
import type { BranchReviewWindowData } from "../../../src/types/sub-window"
import { test, expect } from "../fixtures/test-fixtures"
import { expandBranch, findBranchRow, getBranchCommitRow } from "../helpers/branch-helpers"
import { openCommitPreview } from "../helpers/preview-helpers"
import { getOpenedSubWindows, openSubWindowPage } from "../helpers/sub-window-helpers"

// the `symbol_navigation` template: Uses.kt calls `store.add` of an untyped receiver (an `add` in Store.kt and in
// Queue.kt) and `helper`, declared at the end of Store.kt, which its change at the top leaves in a fold
// the browser of the e2e tests says it runs on Windows ("Desktop Chrome"): ⌘B and Back are Ctrl+B and Ctrl+Alt+← there
// (⌘B and ⌘[ on macOS); a click and a hover take Meta, which a Windows build takes beside Ctrl, so a Ctrl-click of a
// macOS host does not open the context menu
const MODIFIER = "Meta"
const COMMAND = "Control"
const BACK = "Control+Alt+ArrowLeft"

async function openNavigationReview(page: Page): Promise<Locator> {
  await expandBranch(page, findBranchRow(page, "nav"))
  await openCommitPreview(page, getBranchCommitRow(page, "nav", "use the store and the queue"))
  await page.keyboard.press("d")
  await expect.poll(async () => (await getOpenedSubWindows(page)).length).toBe(1)
  const [opened] = await getOpenedSubWindows<BranchReviewWindowData>(page)
  await openSubWindowPage(page, opened!)
  const code = page.getByTestId("commit-review-code")
  await expect(code).toContainText("store.add(1)")
  return code
}

/** The middle of the first `word` in the code of the file `path`, on the new side, the file shown by its row of the tree */
async function pointOf(code: Locator, path: string, word: string): Promise<{ x: number; y: number }> {
  const page = code.page()
  await page.getByRole("treeitem", { name: path.split("/").at(-1) }).click()
  const section = code.locator(`[data-testid=diff-view-file][data-path="${path}"]`)
  await expect(section).toBeInViewport()
  return section.evaluate((element, name) => {
    const walker = document.createTreeWalker(element, NodeFilter.SHOW_TEXT)
    for (let text = walker.nextNode(); text !== null; text = walker.nextNode()) {
      const content = text.textContent ?? ""
      const index = content.search(new RegExp(`\\b${name}\\b`))
      const cell = text.parentElement?.closest<HTMLElement>(".dv-code[data-side]")
      if (index >= 0 && cell?.dataset.side === "1") {
        const range = document.createRange()
        range.setStart(text, index)
        range.setEnd(text, index + name.length)
        const rect = range.getBoundingClientRect()
        return { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 }
      }
    }
    throw new Error(`no ${name} in ${element.dataset.path}`)
  }, word)
}

/** The selected text and the file of the view it is in */
async function selected(page: Page): Promise<{ text: string; path: string | null }> {
  return page.evaluate(() => {
    const selection = document.getSelection()
    const node = selection?.anchorNode ?? null
    const element = node instanceof Element ? node : (node?.parentElement ?? null)
    return { text: selection?.toString() ?? "", path: element?.closest<HTMLElement>("[data-testid=diff-view-file]")?.dataset.path ?? null }
  })
}

test.describe("Go to Declaration", () => {
  test.beforeEach(async ({ setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("symbol_navigation")
    await syncAndWaitForBranches()
  })

  test("should underline a name under the pointer with the modifier, go to its declaration in a fold on a click, and back", async ({ page }) => {
    const code = await openNavigationReview(page)
    const scroller = code.getByTestId("diff-view")
    // the declaration of `helper` is in the fold of Store.kt
    await expect(code).not.toContainText("fun helper(value: Int)")
    const at = await pointOf(code, "src/Uses.kt", "helper")
    await page.mouse.move(at.x, at.y)
    await expect(scroller).not.toHaveCSS("cursor", "pointer")
    await page.keyboard.down(MODIFIER)
    await page.mouse.move(at.x + 1, at.y)
    await expect(scroller).toHaveCSS("cursor", "pointer")
    await page.mouse.click(at.x, at.y)
    await page.keyboard.up(MODIFIER)
    await expect(code).toContainText("fun helper(value: Int)")
    await expect.poll(() => selected(page)).toEqual({ text: "helper", path: "src/Store.kt" })
    // Back: the name the jump started from
    await page.keyboard.press(BACK)
    await expect.poll(() => selected(page)).toEqual({ text: "helper", path: "src/Uses.kt" })
  })

  test("should offer the declarations of an ambiguous name and show the one chosen, from ⌘B and from the menu of the code", async ({ page }) => {
    const code = await openNavigationReview(page)
    const at = await pointOf(code, "src/Uses.kt", "add")
    // ⌘B on the selection
    await page.mouse.dblclick(at.x, at.y)
    await expect.poll(async () => (await selected(page)).text).toBe("add")
    await page.keyboard.press(`${COMMAND}+B`)
    const chooser = page.getByRole("listbox", { name: "Declarations of add" })
    await expect(chooser.getByRole("option")).toHaveText(["Queue.addsrc/Queue.kt:2", "Store.addsrc/Store.kt:33"])
    await chooser.getByRole("option", { name: /Store\.add/ }).click()
    await expect(chooser).toBeHidden()
    await expect.poll(() => selected(page)).toEqual({ text: "add", path: "src/Store.kt" })

    // the item of the menu of the code, Enter on the first declaration
    const again = await pointOf(code, "src/Uses.kt", "add")
    await page.mouse.click(again.x, again.y, { button: "right" })
    await page.getByRole("menuitem", { name: "Go to Declaration" }).click()
    await expect(chooser.getByRole("option").first()).toBeFocused()
    await page.keyboard.press("Enter")
    await expect.poll(() => selected(page)).toEqual({ text: "add", path: "src/Queue.kt" })
  })

  test("should say when the review has no declaration of a name", async ({ page }) => {
    const code = await openNavigationReview(page)
    const at = await pointOf(code, "src/Uses.kt", "Any")
    await page.keyboard.down(MODIFIER)
    await page.mouse.click(at.x, at.y)
    await page.keyboard.up(MODIFIER)
    await expect(page.getByText("No declaration of Any in the review")).toBeVisible()
  })
})
