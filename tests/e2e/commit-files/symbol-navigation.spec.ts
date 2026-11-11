// @spec openspec/specs/commit/symbol-navigation/spec.md
import { writeFileSync } from "node:fs"
import path from "node:path"
import type { Locator, Page } from "@playwright/test"
import type { FindDeclarationParams } from "../../../src/lib/bindings"
import type { BranchReviewWindowData } from "../../../src/types/sub-window"
import { test, expect } from "../fixtures/test-fixtures"
import { expandBranch, findBranchRow, getBranchCommitRow } from "../helpers/branch-helpers"
import { changedParts, deletedParts, diffLineNumbers, diffRows, expectOverviewLaidOut } from "../helpers/diff-helpers"
import { openCommitPreview } from "../helpers/preview-helpers"
import { selectedInDiffs } from "../helpers/selection-helpers"
import { getOpenedSubWindows, openSubWindowPage } from "../helpers/sub-window-helpers"
import { holdCommandCalls, holdNextAnswer, recordCommand, expectClosed } from "../helpers/wait-helpers"

// the `symbol_navigation` template: Uses.kt calls `store.add` of an untyped receiver (an `add` in Store.kt and in
// Queue.kt) and `helper`, declared at the end of Store.kt, which its change at the top leaves in a fold
// the browser of the e2e tests says it runs on Windows ("Desktop Chrome"): ⌘B and Back are Ctrl+B and Ctrl+Alt+← there
// (⌘B and ⌘[ on macOS); a click and a hover take Meta, which a Windows build takes beside Ctrl, so a Ctrl-click of a
// macOS host does not open the context menu
const MODIFIER = "Meta"
const COMMAND = "Control"
const BACK = "Control+Alt+ArrowLeft"

/** The code of the review window of a commit of `nav`: the commit of the template unless another `subject` is given */
async function openNavigationReview(page: Page, subject = "use the store and the queue"): Promise<Locator> {
  await expandBranch(page, findBranchRow(page, "nav"))
  await openCommitPreview(page, getBranchCommitRow(page, "nav", subject))
  await page.keyboard.press("d")
  await expect.poll(async () => (await getOpenedSubWindows(page)).length).toBe(1)
  const [opened] = await getOpenedSubWindows<BranchReviewWindowData>(page)
  await openSubWindowPage(page, opened!)
  const code = page.getByTestId("commit-review-code")
  await expect(code).toContainText("store.add(1)")
  // the heading of the commit comes above the files after them: the places of the names are read once they stay
  await expectOverviewLaidOut(code)
  return code
}

/** The index of the repository, as accio answers `symbol outsider`: its declaration in src/Base.kt, outside the review */
function writeOutsiderIndex(repositoryPath: string): void {
  writeFileSync(
    path.join(repositoryPath, ".git", "branch-deck-accio.json"),
    // as accio answers a written name: its columns in UTF-16 units from 0 (`fun outsider(…)`: 4 to 12), which the review
    // selects; a hit without a column goes to its line, nothing selected
    JSON.stringify({ symbol: { outsider: [{ file: "src/Base.kt", line: 1, column: 4, end_column: 12, kind: "function", name: "outsider" }] } }),
  )
}

/** Where a word is drawn: its middle, where a press lands, and the corner under its start, where the menu of a key opens */
interface WordPlace {
  x: number
  y: number
  left: number
  bottom: number
}

/** The first `word` in the code of the file `path`, on the new side, the file shown by its row of the tree */
async function pointOf(code: Locator, path: string, word: string): Promise<WordPlace> {
  await code
    .page()
    .getByRole("treeitem", { name: path.split("/").at(-1) })
    .click()
  return wordIn(code, path, word)
}

/** The first `word` in the code of the file `path` of `view`, on the new side */
async function wordIn(view: Locator, path: string, word: string): Promise<WordPlace> {
  const section = view.locator(`[data-testid=diff-view-file][data-path="${path}"]`)
  await expect(section).toBeInViewport()
  return section.evaluate((element, name) => {
    const walker = document.createTreeWalker(element, NodeFilter.SHOW_TEXT)
    for (let text = walker.nextNode(); text !== null; text = walker.nextNode()) {
      const content = text.textContent ?? ""
      const index = content.search(new RegExp(`\\b${name}\\b`))
      const cell = text.parentElement?.closest<HTMLElement>("[data-testid=diff-code][data-side]")
      if (index >= 0 && cell?.dataset.side === "1") {
        const range = document.createRange()
        range.setStart(text, index)
        range.setEnd(text, index + name.length)
        const rect = range.getBoundingClientRect()
        return { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2, left: rect.left, bottom: rect.bottom }
      }
    }
    throw new Error(`no ${name} in ${element.dataset.path}`)
  }, word)
}

/** The number of the first line of the file `path`: `class Uses {` of Uses.kt, a line that starts with a word */
function firstLineNumber(code: Locator, path: string): Locator {
  return diffLineNumbers(code.locator(`[data-testid=diff-view-file][data-path="${path}"]`)).first()
}

/** `helper` is shown at its declaration in Store.kt, out of its fold; Back returns to its call in Uses.kt */
async function expectHelperShownAndBack(code: Locator): Promise<void> {
  const page = code.page()
  await expect(code).toContainText("fun helper(value: Int)")
  await expect.poll(() => selectedInDiffs(page)).toEqual({ text: "helper", path: "src/Store.kt" })
  await page.keyboard.press(BACK)
  await expect.poll(() => selectedInDiffs(page)).toEqual({ text: "helper", path: "src/Uses.kt" })
}

/** The file of `outsider` is open over the review once: one Back closes it and returns to the name in Uses.kt */
async function expectOutsiderFileAndBack(page: Page): Promise<void> {
  const fileView = page.getByTestId("declaration-file-view")
  await expect(fileView).toContainText("src/Base.kt")
  await page.keyboard.press(BACK)
  await expect(fileView).toBeHidden()
  await expect.poll(() => selectedInDiffs(page)).toEqual({ text: "outsider", path: "src/Uses.kt" })
}

/** ⌥F7 on `word` of the file `path`, selected by a double click */
async function findUsagesOf(code: Locator, path: string, word: string): Promise<void> {
  const page = code.page()
  const at = await pointOf(code, path, word)
  await page.mouse.dblclick(at.x, at.y)
  await expect.poll(async () => (await selectedInDiffs(page)).text).toBe(word)
  await page.keyboard.press("Alt+F7")
}

/** Every command of the page has its answer: a held one the test released has reached the page */
async function answered(page: Page): Promise<void> {
  await expect.poll(() => page.evaluate(() => (window as unknown as { __ipcStats: { pending: number } }).__ipcStats.pending)).toBe(0)
}

test.describe("Go to Declaration", () => {
  /** The test repository of the test, for its fixture of the index */
  let repositoryPath = ""
  test.beforeEach(async ({ setupRepo, syncAndWaitForBranches }) => {
    repositoryPath = (await setupRepo("symbol_navigation")).path
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
    // beside the text the pointer is on no name: right of the number of `class Uses {`, where the nearest text is the
    // word `class`
    const number = (await firstLineNumber(code, "src/Uses.kt").boundingBox())!
    await page.mouse.move(number.x + number.width + 4, number.y + number.height / 2)
    await expect(scroller).not.toHaveCSS("cursor", "pointer")
    await page.mouse.move(at.x + 1, at.y)
    await expect(scroller).toHaveCSS("cursor", "pointer")
    await page.mouse.click(at.x, at.y)
    await page.keyboard.up(MODIFIER)
    // Back: the name the jump started from
    await expectHelperShownAndBack(code)
  })

  test("should offer the declarations of an ambiguous name and show the one chosen, from ⌘B and from the menu of the code", async ({ page }) => {
    const code = await openNavigationReview(page)
    const at = await pointOf(code, "src/Uses.kt", "add")
    // ⌘B on the selection
    await page.mouse.dblclick(at.x, at.y)
    await expect.poll(async () => (await selectedInDiffs(page)).text).toBe("add")
    await page.keyboard.press(`${COMMAND}+B`)
    const chooser = page.getByRole("listbox", { name: "Declarations of add" })
    await expect(chooser.getByRole("option")).toHaveText(["Queue.addsrc/Queue.kt:2", "Store.addsrc/Store.kt:33"])
    await chooser.getByRole("option", { name: /Store\.add/ }).click()
    await expectClosed(chooser)
    await expect.poll(() => selectedInDiffs(page)).toEqual({ text: "add", path: "src/Store.kt" })

    // the item of the menu of the code, Enter on the first declaration
    const again = await pointOf(code, "src/Uses.kt", "add")
    await page.mouse.click(again.x, again.y, { button: "right" })
    await page.getByRole("menuitem", { name: "Go to Declaration" }).click()
    await expect(chooser.getByRole("option").first()).toBeFocused()
    await page.keyboard.press("Enter")
    await expect.poll(() => selectedInDiffs(page)).toEqual({ text: "add", path: "src/Queue.kt" })
  })

  test(
    "should open the menu of the code by the context menu key and Shift+F10 on the name at the selection, under that name, and by the pointer on no name selected elsewhere",
    { tag: "@webkit" },
    async ({ page }) => {
      const code = await openNavigationReview(page)
      const view = code.getByTestId("diff-view")
      const menu = page.getByRole("menu")
      const goToDeclaration = menu.getByRole("menuitem", { name: "Go to Declaration" })
      const findUsages = menu.getByRole("menuitem", { name: "Find Usages" })
      /** The menu is open under the start of the name: not at the middle of the view, where the engine opens the menu of a key */
      const expectMenuAt = async (name: WordPlace) => {
        await expect(menu).toBeVisible()
        const box = (await menu.boundingBox())!
        expect(box.x - name.left).toBeGreaterThanOrEqual(-1)
        expect(box.x - name.left).toBeLessThan(12)
        expect(box.y - name.bottom).toBeGreaterThanOrEqual(-1)
        expect(box.y - name.bottom).toBeLessThan(12)
      }

      // a click leaves the caret in `helper`, nothing selected, and the focus on the view
      const helper = await pointOf(code, "src/Uses.kt", "helper")
      await page.mouse.click(helper.x, helper.y)
      await expect(view).toBeFocused()
      expect((await selectedInDiffs(page)).text).toBe("")
      await page.keyboard.press("ContextMenu")
      await expectMenuAt(helper)
      await expect(goToDeclaration).toBeEnabled()
      await expect(findUsages).toBeEnabled()
      // Escape gives the focus back to the view, the caret where it was: the next key opens the menu on the name again
      await page.keyboard.press("Escape")
      await expectClosed(menu)
      await expect(view).toBeFocused()

      // Shift+F10, and the item chosen by the keys of a menu: the usages of the name at the caret
      await page.keyboard.press("Shift+F10")
      await expectMenuAt(helper)
      // the menu takes the focus a frame after it shows: its keys before that are the view's
      await expect(menu).toBeFocused()
      await page.keyboard.press("ArrowDown")
      await expect(goToDeclaration).toBeFocused()
      await page.keyboard.press("ArrowDown")
      await expect(findUsages).toBeFocused()
      await page.keyboard.press("Enter")
      const usages = page.getByRole("listbox", { name: "Usages of helper" })
      await expect(usages.getByRole("option")).toHaveText(["callhelper(2)src/Uses.kt:4", "declarationfun helper(value: Int) = valuesrc/Store.kt:36"])
      await page.keyboard.press("Escape")
      await expectClosed(page.getByTestId("symbol-chooser"))

      // a selected name, not a caret: the menu of a key is of that name, wherever the pointer is
      const outsider = await pointOf(code, "src/Uses.kt", "outsider")
      await page.mouse.dblclick(outsider.x, outsider.y)
      await expect.poll(async () => (await selectedInDiffs(page)).text).toBe("outsider")
      await page.mouse.move(helper.x, helper.y)
      await page.keyboard.press("ContextMenu")
      await expectMenuAt(outsider)
      await goToDeclaration.click()
      // nothing declares it in the review, and the repository has no index
      await expect(page.getByText("No declaration of outsider found")).toBeVisible()
      // the menu closed: the name is selected as it was, also where the engine cleared the selection as the menu took the focus
      await expectClosed(menu)
      await expect(view).toBeFocused()
      await expect.poll(async () => (await selectedInDiffs(page)).text).toBe("outsider")

      // the pointer where no name is (the number of a line, beside the word `class`), `outsider` still selected
      // elsewhere: the menu has no name
      await firstLineNumber(code, "src/Uses.kt").click({ button: "right" })
      await expect(menu).toBeVisible()
      await expect(goToDeclaration).toBeDisabled()
      await expect(findUsages).toBeDisabled()
    },
  )

  test("should say when the review has no declaration of a name", async ({ page }) => {
    const code = await openNavigationReview(page)
    const at = await pointOf(code, "src/Uses.kt", "Any")
    await page.keyboard.down(MODIFIER)
    await page.mouse.click(at.x, at.y)
    await page.keyboard.up(MODIFIER)
    await expect(page.getByText("No declaration of Any found")).toBeVisible()
  })

  test("should open a declaration outside the review that the index of the repository knows, go back to the review, and from that file to a declaration of the review", async ({
    page,
  }) => {
    writeOutsiderIndex(repositoryPath)
    const code = await openNavigationReview(page)
    const fileView = page.getByTestId("declaration-file-view")
    await expect(fileView).toBeHidden()
    const at = await pointOf(code, "src/Uses.kt", "outsider")
    await page.keyboard.down(MODIFIER)
    await page.mouse.click(at.x, at.y)
    await page.keyboard.up(MODIFIER)
    // the file at the revision of the commit, over the review
    await expect(fileView).toContainText("src/Base.kt")
    await expect(fileView).toContainText("outside the review")
    await expect(fileView.getByRole("region", { name: "File src/Base.kt" })).toContainText("fun outsider(value: Int) = helper(value)")
    await expect.poll(() => selectedInDiffs(page)).toEqual({ text: "outsider", path: "src/Base.kt" })
    // Back: the review, at the name
    await page.keyboard.press(BACK)
    await expect(fileView).toBeHidden()
    await expect.poll(() => selectedInDiffs(page)).toEqual({ text: "outsider", path: "src/Uses.kt" })

    // the button Back, then from the file outside the review to `helper` of Store.kt, in the review
    await page.keyboard.down(MODIFIER)
    await page.mouse.click(at.x, at.y)
    await page.keyboard.up(MODIFIER)
    await expect(fileView).toContainText("src/Base.kt")
    await fileView.getByRole("button", { name: /Back/ }).click()
    await expect(fileView).toBeHidden()
    await page.keyboard.down(MODIFIER)
    await page.mouse.click(at.x, at.y)
    await page.keyboard.up(MODIFIER)
    const helper = await wordIn(fileView, "src/Base.kt", "helper")
    await page.keyboard.down(MODIFIER)
    await page.mouse.click(helper.x, helper.y)
    await page.keyboard.up(MODIFIER)
    await expect(fileView).toBeHidden()
    await expect(code).toContainText("fun helper(value: Int)")
    await expect.poll(() => selectedInDiffs(page)).toEqual({ text: "helper", path: "src/Store.kt" })

    // a file of the tree, K (Uses.kt is the last file) and N show in the review: the file over it closes
    const openOutsider = async () => {
      const again = await pointOf(code, "src/Uses.kt", "outsider")
      await page.keyboard.down(MODIFIER)
      await page.mouse.click(again.x, again.y)
      await page.keyboard.up(MODIFIER)
      await expect(fileView).toContainText("src/Base.kt")
    }
    await openOutsider()
    await page.getByRole("treeitem", { name: "Queue.kt" }).click()
    await expect(fileView).toBeHidden()
    await expect(code.locator('[data-testid=diff-view-file][data-path="src/Queue.kt"]')).toBeInViewport()
    for (const key of ["k", "n"]) {
      await openOutsider()
      await page.keyboard.press(key)
      await expect(fileView).toBeHidden()
    }
  })

  test("should open one file for a name clicked twice before the answer, so one Back returns to the review", async ({ page }) => {
    writeOutsiderIndex(repositoryPath)
    const code = await openNavigationReview(page)
    const at = await pointOf(code, "src/Uses.kt", "outsider")
    const held = await holdCommandCalls(page, "find_declaration")
    for (let click = 0; click < 2; click++) {
      await page.keyboard.down(MODIFIER)
      await page.mouse.click(at.x, at.y)
      await page.keyboard.up(MODIFIER)
    }
    // the second click joins the first: one question reached the backend
    await expect.poll(() => held.calls.length).toBe(1)
    held.release()
    await expectOutsiderFileAndBack(page)
    expect(held.calls).toHaveLength(1)
  })

  test("should say that it searches while the backend answers, and stop the search on Escape and on Cancel", async ({ page }) => {
    writeOutsiderIndex(repositoryPath)
    const code = await openNavigationReview(page)
    const cancels = await recordCommand(page, "cancel_symbol_search")
    const at = await pointOf(code, "src/Uses.kt", "outsider")
    const goToOutsider = async () => {
      await page.keyboard.down(MODIFIER)
      await page.mouse.click(at.x, at.y)
      await page.keyboard.up(MODIFIER)
    }

    // Go to Declaration, its answer held: the search says so at the name, Escape stops it in the backend
    const declaration = await holdNextAnswer(page, "find_declaration")
    await goToOutsider()
    const searching = page.getByRole("status", { name: "Looking for the declaration…" })
    await expect(searching).toBeVisible()
    await declaration.ran
    await page.keyboard.press("Escape")
    await expect(searching).toBeHidden()
    await expect.poll(() => cancels.length).toBe(1)
    declaration.release()
    await answered(page)

    // Find Usages, its answer held: Cancel stops it
    const usages = await holdNextAnswer(page, "find_usages")
    await findUsagesOf(code, "src/Uses.kt", "helper")
    const finding = page.getByRole("status", { name: "Finding usages…" })
    await expect(finding).toBeVisible()
    await usages.ran
    await finding.getByRole("button", { name: "Cancel" }).click()
    await expect(finding).toBeHidden()
    await expect.poll(() => cancels.length).toBe(2)
    usages.release()
    await answered(page)
    await expectClosed(page.getByTestId("symbol-chooser"))

    // the canceled answers opened nothing: the next jump opens one file, and one Back returns to the review
    await goToOutsider()
    await expectOutsiderFileAndBack(page)
  })
})

test(
  "should underline a struck-through name of an Inline row alone under the pointer with the modifier, and go to the declaration of that old name on a click",
  { tag: "@webkit" },
  async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("symbol_navigation")
    // a call of `helper` in Uses.kt becomes a call of `helperScaled`, which nothing declares: in Inline, the default,
    // its row is the new line with the old name struck through before the new one
    const uses = (name: string) => `class Uses {\n  fun run(store: Any) {\n    store.add(1)\n    val sum = ${name}(2) + outsider(3) + outsider(4) + outsider(5)\n  }\n}\n`
    await repo.commit("(nav) sum the calls", { "src/Uses.kt": uses("helper") }, 1704120600)
    await repo.commit("(nav) call the scaled helper", { "src/Uses.kt": uses("helperScaled") }, 1704121200)
    // the index of the repository, as accio answers `symbol helper`: its declaration in src/Store.kt, outside this review
    writeFileSync(
      path.join(repo.path, ".git", "branch-deck-accio.json"),
      JSON.stringify({ symbol: { helper: [{ file: "src/Store.kt", line: 36, column: 4, end_column: 10, kind: "function", name: "helper" }] } }),
    )
    await syncAndWaitForBranches()
    const code = await openNavigationReview(page, "call the scaled helper")
    const pair = diffRows(code, "inline")
    const struck = deletedParts(pair)
    await expect(struck).toHaveText("helper")
    const inserted = changedParts(pair)
    await expect(inserted).toHaveText("helperScaled")

    /** The underline of the link under the pointer: where it starts and how wide it is, in whole pixels */
    const link = async () => {
      const boxes = await Promise.all((await code.getByTestId("diff-link").all()).map(line => line.boundingBox()))
      return boxes.map(box => [Math.round(box!.x), Math.round(box!.width)])
    }
    const place = async (part: Locator) => {
      const box = (await part.boundingBox())!
      return { x: box.x + box.width / 2, y: box.y + box.height / 2, underline: [Math.round(box.x), Math.round(box.width)] }
    }
    const scroller = code.getByTestId("diff-view")
    const old = await place(struck)
    await page.mouse.move(old.x, old.y)
    await page.keyboard.down(MODIFIER)
    await page.mouse.move(old.x + 1, old.y)
    await expect(scroller).toHaveCSS("cursor", "pointer")
    // the old name alone: not the new name beside it
    await expect.poll(link).toEqual([old.underline])
    // the new name alone: not the struck-through part before it
    const added = await place(inserted)
    await page.mouse.move(added.x, added.y)
    await expect.poll(link).toEqual([added.underline])

    // a click on the old name asks for the declaration of `helper` on the old side, and shows it
    const asked = await recordCommand(page, "find_declaration")
    await page.mouse.move(old.x, old.y)
    await page.mouse.click(old.x, old.y)
    await page.keyboard.up(MODIFIER)
    const fileView = page.getByTestId("declaration-file-view")
    await expect(fileView).toContainText("src/Store.kt")
    await expect.poll(() => selectedInDiffs(page)).toEqual({ text: "helper", path: "src/Store.kt" })
    expect(asked.map(call => (call as unknown as FindDeclarationParams).site)).toMatchObject([{ path: "src/Uses.kt", side: "old", line: 3, column: 14 }])

    // Back (⌘[ where the page says it runs on a Mac, as WebKit's does): the old name the jump started from, selected
    // where its row strikes it through
    const mac = await page.evaluate(() => navigator.userAgent.toLowerCase().includes("mac"))
    await page.keyboard.press(mac ? "Meta+BracketLeft" : BACK)
    await expect(fileView).toBeHidden()
    await expect.poll(() => selectedInDiffs(page)).toEqual({ text: "helper", path: "src/Uses.kt" })
    // the menu of the code on the old name acts on it, whatever a click left selected elsewhere
    await diffRows(code, "context", { hasText: "store.add(1)" }).click()
    await expect.poll(async () => (await selectedInDiffs(page)).text).toBe("")
    // the reveal of Back scrolled the row
    const again = await place(struck)
    await page.mouse.click(again.x, again.y, { button: "right" })
    await page.getByRole("menuitem", { name: "Find Usages" }).click()
    // the usage the review knows: the call of the old line itself
    await expect(page.getByRole("listbox", { name: "Usages of helper" }).getByRole("option")).toHaveText([
      "callval sum = helper(2) + outsider(3) + outsider(4) + outsider(5)src/Uses.kt:4",
    ])
  },
)

test.describe("Find Usages", () => {
  /** The test repository of the test, for its fixture of the index */
  let repositoryPath = ""
  test.beforeEach(async ({ setupRepo, syncAndWaitForBranches }) => {
    repositoryPath = (await setupRepo("symbol_navigation")).path
    await syncAndWaitForBranches()
  })

  test("should list the usages of a name in the review and outside it, from ⌥F7 and from the menu of the code, and show the one chosen", async ({ page }) => {
    // the index of the repository, as accio answers `references --at src/Store.kt:36:5`: the call of Base.kt and a prose namesake
    const snippet = "fun outsider(value: Int) = helper(value)"
    writeFileSync(
      path.join(repositoryPath, ".git", "branch-deck-accio.json"),
      JSON.stringify({
        references: {
          "src/Store.kt:36:5": [
            { file: "src/Base.kt", line: 1, column: 28, snippet, kind: "call", resolved: "typed" },
            { file: "README.md", line: 1, column: 1, snippet: "helper", kind: "read", resolved: "name" },
          ],
        },
      }),
    )
    const code = await openNavigationReview(page)
    const chooser = page.getByTestId("symbol-chooser")
    const usages = page.getByRole("listbox", { name: "Usages of helper" })
    // ⌥F7 on the selection: the call, the declaration in Store.kt, then the call outside the review
    await findUsagesOf(code, "src/Uses.kt", "helper")
    await expect(usages.getByRole("option")).toHaveText(["callhelper(2)src/Uses.kt:4", "declarationfun helper(value: Int) = valuesrc/Store.kt:36", `call${snippet}src/Base.kt:1`])
    await expect(chooser).toContainText("2 usages of helper")
    await expect(chooser).toContainText("1 name-only match was omitted because it was not linked to this declaration")
    await expect(usages.getByRole("group", { name: "Outside the review" })).toContainText("src/Base.kt:1")
    await usages.getByRole("option", { name: /src\/Store\.kt/ }).click()
    await expectClosed(chooser)
    // Back: the name the list was asked for
    await expectHelperShownAndBack(code)
    // Escape closes the list and gives the focus back to the view: ⌥F7 opens it again
    await page.keyboard.press("Alt+F7")
    await expect(usages).toBeVisible()
    await page.keyboard.press("Escape")
    await expectClosed(chooser)
    await page.keyboard.press("Alt+F7")
    await expect(usages).toBeVisible()
    await page.keyboard.press("Escape")
    // closed before the menu opens the next one: the close gives the focus back to the view, also after the next list
    // took it when its exit ends late (under load)
    await expectClosed(chooser)

    // the item of the menu of the code; ↓ to the usage outside the review, Enter opens its file
    const again = await pointOf(code, "src/Uses.kt", "helper")
    await page.mouse.click(again.x, again.y, { button: "right" })
    await page.getByRole("menuitem", { name: "Find Usages" }).click()
    await expect(usages.getByRole("option").first()).toBeFocused()
    await page.keyboard.press("ArrowDown")
    await page.keyboard.press("ArrowDown")
    await expect(usages.getByRole("option").last()).toBeFocused()
    await page.keyboard.press("Enter")
    const fileView = page.getByTestId("declaration-file-view")
    await expect(fileView).toContainText("outside the review")
    await expect.poll(() => selectedInDiffs(page)).toEqual({ text: "helper", path: "src/Base.kt" })
  })

  test("should keep what the review knows and say why when the index of the repository fails", async ({ page }) => {
    writeFileSync(path.join(repositoryPath, ".git", "branch-deck-accio.json"), JSON.stringify({ error: "accio exited with code 2" }))
    const code = await openNavigationReview(page)
    await findUsagesOf(code, "src/Uses.kt", "helper")
    await expect(page.getByRole("listbox", { name: "Usages of helper" }).getByRole("option")).toHaveText([
      "callhelper(2)src/Uses.kt:4",
      "declarationfun helper(value: Int) = valuesrc/Store.kt:36",
    ])
    await expect(page.getByTestId("symbol-chooser")).toContainText("accio did not answer: accio exited with code 2: the usages outside the review are missing")
    await page.keyboard.press("Escape")
    // a name the review does not declare: no declaration, and why
    const outsider = await pointOf(code, "src/Uses.kt", "outsider")
    await page.keyboard.down(MODIFIER)
    await page.mouse.click(outsider.x, outsider.y)
    await page.keyboard.up(MODIFIER)
    await expect(page.getByText("No declaration of outsider found")).toBeVisible()
    await expect(page.getByText("accio did not answer: accio exited with code 2")).toBeVisible()
  })

  test("should say when a name has no usage but its declaration", async ({ page }) => {
    const code = await openNavigationReview(page)
    await findUsagesOf(code, "src/Store.kt", "version")
    await expect(page.getByText("No usages of version found")).toBeVisible()
    await expectClosed(page.getByTestId("symbol-chooser"))
  })
})
