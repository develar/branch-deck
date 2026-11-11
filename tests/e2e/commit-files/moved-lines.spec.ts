// @spec openspec/specs/commit/quiet-changes/spec.md
import type { Locator, Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import {
  changedParts,
  copiedByKey,
  deletedParts,
  diffLineNumbers,
  diffRows,
  diffSides,
  expectBarReachesEndOf,
  findMarks,
  horizontalBars,
  joinedText,
  movedSides,
  moveNotes,
  moveTargets,
  remarkHits,
  scrollDiffsTo,
} from "../helpers/diff-helpers"
import { codeOf, findInReview, MARKS_SUBJECT, openLedgerPreview, openMarksReview } from "../helpers/marks-helpers"
import { openPlanWindow, turnOnReviewPlan } from "../helpers/review-plan-helpers"
import { selectedInDiffs } from "../helpers/selection-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"
import { expectClosed } from "../helpers/wait-helpers"

// the moves of src/Ledger.kt in the `commit_marks` template. `accounts()` moves up in the class as it is: its four
// lines and the blank line before them leave the old lines 22 to 26 for the new lines 6 to 10. `describe` moves into
// the class: its six lines and the blank line after them leave the old lines 46 to 52 for the new lines 19 to 25,
// indented by two columns, a space added in its fourth line
const ACCOUNTS = ["  fun accounts(): List<String> {", "    val names = entries.map { it.account }", "    return names.distinct().sorted()", "  }"]
const DESCRIBE_WAS = [
  "fun describe(entry: Entry): String {",
  '  val sign = if (entry.amount < 0) "-" else "+"',
  "  val amount = kotlin.math.abs(entry.amount)",
  '  val text = "$sign$amount ${entry.account}"',
  "  return text.trim()",
  "}",
  "",
]
const DESCRIBE = [
  "  fun describe(entry: Entry): String {",
  '    val sign = if (entry.amount < 0) "-" else "+"',
  "    val amount = kotlin.math.abs(entry.amount)",
  '    val text = "$sign $amount ${entry.account}"',
  "    return text.trim()",
  "  }",
  "",
]

/** The rows of the lines of a move in `scope`, of one end: where they left or where they stand now */
function movedRows(scope: Locator, at: "source" | "destination"): Locator {
  return diffRows(scope).filter({ has: scope.page().locator(`[data-cell="moved"][data-move=${at}]`) })
}

/** The note where the lines of `accounts()` left, and the one where they stand now */
function accountsNotes(scope: Locator): { source: Locator; destination: Locator } {
  return { source: moveNotes(scope, "source").filter({ hasText: "5 lines moved" }), destination: moveNotes(scope, "destination").filter({ hasText: "from line 22" }) }
}

/** The view went to `note`: it is marked as the row the jump landed on, and in view */
async function expectLandedOn(note: Locator): Promise<void> {
  await expect(note).toHaveAttribute("data-landed", "")
  await expect(note).toBeInViewport()
}

/** Back and Forward of the diffs: Chromium of the tests says it runs on Windows (Ctrl+Alt+← and →), WebKit on macOS (⌘[ and ⌘]) */
function historyKeys(browserName: string): { back: string; forward: string } {
  return browserName === "webkit" ? { back: "Meta+BracketLeft", forward: "Meta+BracketRight" } : { back: "Control+Alt+ArrowLeft", forward: "Control+Alt+ArrowRight" }
}

/** The one button of a place that names the key of the moves: the note the key takes now */
function keyedTargets(scope: Locator): Locator {
  return scope.locator("[data-testid=diff-move-target][aria-keyshortcuts]")
}

/** The key the item of a menu shows, a `Kbd` after its label */
function itemKey(item: Locator): Locator {
  return item.locator("[data-slot=kbd]")
}

/**
 * A commit of the branch `store` that moves `compute` of src/Store.kt from the end of its class to its start, its nine
 * lines and the blank line before them from the old lines 121 to 130 to the new lines 2 to 11, thirty functions
 * between the two ends; with `edited` its fourth line has `newBonus` for `oldBonus`, an edited line of the move. One
 * function between the ends changes too, a change of the file that is no move; with `apart` every one of them, so no
 * fold hides the way and the ends are screens apart
 */
async function commitMovedCompute(repo: TestRepositoryBuilder, { edited, apart = false }: { edited: boolean; apart?: boolean }): Promise<void> {
  const functions = (value: (at: number) => number) => Array.from({ length: 30 }, (_, at) => `  fun filler${at}(): Int {\n    return ${value(at)}\n  }\n`).join("\n")
  const fillers = functions(at => at)
  const compute = (bonus: string) =>
    `  fun compute(items: List<Int>): Int {\n    val start = items.first()\n    val middle = items.size / 2\n    val total = items.sum() + ${bonus}\n    val result = total - start\n    val scaled = result * middle\n    val bounded = scaled.coerceAtLeast(0)\n    return bounded * 2\n  }\n`
  await repo.commit("Add the store", { "src/Store.kt": `class Store {\n${fillers}\n${compute("oldBonus")}}\n` }, 1704120600)
  await repo.pushInTerminal("master")
  await repo.commit(
    "(store) compute moves up\n\nThe store computes first.\n",
    { "src/Store.kt": `class Store {\n${compute(edited ? "newBonus" : "oldBonus")}\n${functions(at => (apart || at === 7 ? at + 100 : at))}}\n` },
    1704121200,
  )
}

/**
 * src/Sorted.kt as `before` on master and as `after` in the one commit of the branch `sorted`, in a review with its
 * plan: the code of the review with the section of the file (`own`) and the section of its imports, which stands
 * folded at the end of the story and is opened here (`imports`)
 */
async function openSortedPlan(
  page: Page,
  repo: TestRepositoryBuilder,
  syncAndWaitForBranches: () => Promise<unknown>,
  before: string,
  after: string,
): Promise<{ code: Locator; own: Locator; imports: Locator }> {
  await repo.commit("Add the sorted", { "src/Sorted.kt": before }, 1704120600)
  await repo.pushInTerminal("master")
  await repo.commit("(sorted) the imports\n\nThe total counts two.\n", { "src/Sorted.kt": after }, 1704121200)
  turnOnReviewPlan(repo)
  const window = await openPlanWindow(page, syncAndWaitForBranches, "sorted")
  const code = window.getByTestId("commit-review-code")
  await code.getByRole("button", { name: "Expand Sorted.kt" }).click()
  const section = (key: string) => code.locator(`[data-testid="diff-view-file"][data-section="${key}:src/Sorted.kt"]`)
  return { code, own: section("file"), imports: section("group:mechanical:importsOnly") }
}

/** The cell and the tint of each line of `rows` ("deleted whitespace"; "null" for the full colour) */
function cellsOf(rows: Locator): Promise<string[]> {
  return diffSides(rows).evaluateAll(sides => sides.map(side => `${side.getAttribute("data-cell")} ${side.getAttribute("data-tint")}`))
}

test.describe("The moves of a diff in the preview of a commit", () => {
  test.beforeEach(async ({ setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("commit_marks")
    await syncAndWaitForBranches()
  })

  test("Inline shows the lines of a move once where they stand now, an edited one as the row of its two lines, and one note that opens where they left", async ({ page }) => {
    const { ledger } = await openLedgerPreview(page)
    // where the lines stand now: a note that says where they were and what happened to them on the way, then the lines
    await expect(moveNotes(ledger, "destination")).toHaveText(["Moved from line 22", "Moved from line 46 · re-indented · 1 line edited"])
    const stands = movedRows(ledger, "destination")
    await expect.poll(() => codeOf(stands)).toEqual([...ACCOUNTS, "", ...DESCRIBE])
    // each a new line of the move, in its full colour, but the edited one: the row of its old line and its new one,
    // pale, with the space the new line has more in the full colour and nothing struck through
    await expect
      .poll(() => stands.evaluateAll(rows => rows.map(row => row.getAttribute("data-kind"))))
      .toEqual([...Array.from({ length: 8 }, () => "added"), "inline", "added", "added", "added"])
    const edited = diffRows(ledger, "inline", { hasText: "val text =" })
    await expect(diffSides(edited)).toHaveAttribute("data-tint", "pale")
    await expect(diffLineNumbers(edited)).toHaveText(["49", "22"])
    await expect.poll(() => joinedText(changedParts(edited))).toBe(" ")
    await expect(deletedParts(edited)).toHaveCount(0)
    await expect(diffSides(stands.first())).not.toHaveAttribute("data-tint")
    await expect(diffLineNumbers(stands.first())).toHaveText(["", "6"])

    // where they left: one note for the five lines, no row of them, and the button that shows them
    const { source } = accountsNotes(ledger)
    await scrollDiffsTo(ledger, source)
    await expect(source).toHaveText("5 lines moved to line 6")
    await expect(moveNotes(ledger, "source")).toHaveText(["5 lines moved to line 6", "7 lines moved to line 19"])
    await expect(movedRows(ledger, "source")).toHaveCount(0)
    const toggle = ledger.getByRole("button", { name: /^5 lines moved to line 6: / })
    await expect(toggle).toHaveAccessibleName("5 lines moved to line 6: Show the 5 old lines")
    await expect(toggle).toHaveAttribute("aria-expanded", "false")
    // the preview has no key of the notes and none of the moves: neither button names one
    await expect(toggle).not.toHaveAttribute("aria-keyshortcuts")
    await expect(moveTargets(source)).not.toHaveAttribute("aria-keyshortcuts")

    // a click: the blank line and the four lines show after the note, deleted rows in the colour of a move
    await toggle.click()
    await expect(toggle).toHaveAttribute("aria-expanded", "true")
    await expect(toggle).toHaveAccessibleName("5 lines moved to line 6: Hide the 5 old lines")
    const leftRows = movedRows(ledger, "source")
    await expect.poll(() => codeOf(leftRows)).toEqual(["", ...ACCOUNTS])
    await expect.poll(() => leftRows.evaluateAll(rows => rows.map(row => row.getAttribute("data-kind")))).toEqual(Array.from({ length: 5 }, () => "deleted"))
    await expect(diffLineNumbers(leftRows.nth(1))).toHaveText(["23", ""])
    // a second click hides them again
    await toggle.click()
    await expect(toggle).toHaveAttribute("aria-expanded", "false")
    await expect(leftRows).toHaveCount(0)
  })

  test("Unified and Split keep every line of a move at both ends, in the colour of a move, after notes that open nothing", async ({ page }) => {
    const { preview, ledger } = await openLedgerPreview(page)
    await expect(moveNotes(ledger, "destination")).toHaveCount(2)

    await preview.getByRole("button", { name: "Unified", exact: true }).click()
    // where the lines stand now: inserted rows of the move, the edited one pale with what it has more
    await expect.poll(() => codeOf(movedRows(ledger, "destination"))).toEqual([...ACCOUNTS, "", ...DESCRIBE])
    const edited = diffRows(ledger, "added", { hasText: 'val text = "$sign $amount' })
    await expect(diffSides(edited)).toHaveAttribute("data-tint", "pale")
    await expect.poll(() => joinedText(changedParts(edited))).toBe(" ")
    // where they left: every line after its note, deleted rows of the move; the notes are plain words
    const { source } = accountsNotes(ledger)
    await scrollDiffsTo(ledger, moveNotes(ledger, "source").last())
    // into the view: the rows around it are painted then, the lines after both notes
    await moveNotes(ledger, "source").last().scrollIntoViewIfNeeded()
    await expect(moveNotes(ledger, "source")).toHaveText(["5 lines moved to line 6", "7 lines moved to line 19"])
    await expect(ledger.getByRole("button", { name: /lines moved to line \d+: / })).toHaveCount(0)
    await expect.poll(() => codeOf(movedRows(ledger, "source"))).toEqual(["", ...ACCOUNTS, ...DESCRIBE_WAS])
    await expect.poll(() => movedRows(ledger, "source").evaluateAll(rows => new Set(rows.map(row => row.getAttribute("data-kind"))).size)).toBe(1)
    await expect(diffSides(diffRows(ledger, "deleted", { hasText: 'val text = "$sign$amount' }))).toHaveAttribute("data-tint", "pale")
    await expect(diffSides(diffRows(ledger, "deleted", { hasText: "fun describe(entry: Entry): String {" }))).not.toHaveAttribute("data-tint")

    await preview.getByRole("button", { name: "Split", exact: true }).click()
    // where the lines stand now: on the new side, the old side filled beside them; their note over the new column
    await ledger.getByText("Ledger.kt").first().scrollIntoViewIfNeeded()
    const { destination } = accountsNotes(ledger)
    await expect(destination).toHaveAttribute("data-over", "new")
    await expect.poll(() => codeOf(movedSides(ledger, "destination", 1))).toEqual([...ACCOUNTS, "", ...DESCRIBE])
    await expect(movedSides(ledger, "destination", 0)).toHaveCount(0)
    const standsRow = diffRows(ledger, "changed")
      .filter({ has: page.locator('[data-cell="moved"][data-move=destination]') })
      .first()
    await expect(diffSides(standsRow, 0)).toHaveAttribute("data-cell", "filler")
    // where they left: on the old side, the new side filled; their note over the old column
    await scrollDiffsTo(ledger, moveNotes(ledger, "source").last())
    await moveNotes(ledger, "source").last().scrollIntoViewIfNeeded()
    await expect(source).toHaveAttribute("data-over", "old")
    await expect.poll(() => codeOf(movedSides(ledger, "source", 0))).toEqual(["", ...ACCOUNTS, ...DESCRIBE_WAS])
    await expect(movedSides(ledger, "source", 1)).toHaveCount(0)
    const leftRow = diffRows(ledger, "changed")
      .filter({ has: page.locator('[data-cell="moved"][data-move=source]') })
      .first()
    await expect(diffSides(leftRow, 1)).toHaveAttribute("data-cell", "filler")
  })

  test("a click on the place a note of a move names shows its other end and marks the note there, without a key in the preview", async ({ page }) => {
    const { preview, ledger } = await openLedgerPreview(page)
    const { source, destination } = accountsNotes(ledger)
    // where the lines stand now, the note names where they were: a button to it
    const were = moveTargets(destination)
    await expect(were).toHaveText("from line 22")
    await expect(were).toHaveAccessibleName("Go to line 22, where the lines were")
    await expect(were).not.toHaveAttribute("aria-keyshortcuts")
    await expect(source).not.toBeInViewport()

    await were.click()
    await expectLandedOn(source)
    // and from there back to where they stand now
    const stand = moveTargets(source)
    await expect(stand).toHaveText("to line 6")
    await expect(stand).toHaveAccessibleName("Go to line 6, where the lines stand now")
    await stand.click()
    await expectLandedOn(destination)
    await expect(source).not.toHaveAttribute("data-landed")

    // Unified and Split keep the lines at both ends, each after its note: the same click goes there and back
    for (const mode of ["Unified", "Split"]) {
      await preview.getByRole("button", { name: mode, exact: true }).click()
      await expect(ledger.getByRole("button", { name: /^5 lines moved to line 6: / })).toHaveCount(0)
      await moveTargets(destination).click()
      await expectLandedOn(source)
      await expect(movedSides(ledger, "source").first()).toBeInViewport()
      await moveTargets(source).click()
      await expectLandedOn(destination)
      await expect(source).not.toHaveAttribute("data-landed")
    }
  })
})

/**
 * A commit of the branch `config` that moves two lines of src/Config.kt up, each to a place of its own with an
 * unchanged line between: the old line 8 to the new line 4 and the old line 9 to the new line 6. Three blocks, so four
 * notes a few rows apart: where each line stands now, and one after the other where both left
 */
async function commitTwoMovedLines(repo: TestRepositoryBuilder): Promise<void> {
  const config = (lines: string[]) => `package demo\n\nclass Config {\n${lines.map(line => (line === "" ? "\n" : `  ${line}\n`)).join("")}}\n`
  const alpha = 'val alphaTimeoutMilliseconds = computeTimeout("alpha", 1000)'
  const beta = 'val betaRetryCountMaximum = computeRetries("beta", 5)'
  await repo.commit("Add the settings", { "src/Config.kt": config(["val first = 1", "val second = 2", "val third = 3", "", alpha, beta]) }, 1704120600)
  await repo.pushInTerminal("master")
  await repo.commit("(config) the timeouts first", { "src/Config.kt": config([alpha, "val first = 1", beta, "val second = 2", "val third = 3", ""]) }, 1704121200)
}

test.describe("The moves of a diff in the review window", () => {
  test.beforeEach(async ({ setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("commit_marks")
    await syncAndWaitForBranches()
  })

  /** The review of the commit of the marks at the top of Ledger.kt, the focus on its row of the tree */
  async function openLedgerReview(page: Page): Promise<{ window: Locator; code: Locator; diffs: Locator; position: Locator }> {
    const { window, code, position } = await openMarksReview(page, "ledger", MARKS_SUBJECT)
    await window.getByRole("treeitem", { name: "Ledger.kt" }).click()
    await expect(position).toHaveText("File 1 / 3")
    await expect(accountsNotes(code).destination).toBeInViewport()
    return { window, code, diffs: code.locator("[aria-label='Changes of the commit']"), position }
  }

  // the focus goes to the diffs as the button pressed scrolls out of the painted rows, and the keys of the view work on: WebKit differs there
  test("a click on the place a note names jumps to the other end of the move, Back returns and Forward goes there again", { tag: "@webkit" }, async ({ page, browserName }) => {
    const { code, diffs } = await openLedgerReview(page)
    const { source, destination } = accountsNotes(code)
    const { back, forward } = historyKeys(browserName)
    // in a review the button of the note the key takes names the key: here the first note of a move in view
    await expect(moveTargets(destination)).toHaveAttribute("aria-keyshortcuts", "G")
    await expect(keyedTargets(code)).toHaveText(["from line 22"])

    await moveTargets(destination).click()
    await expectLandedOn(source)
    await expect(destination).not.toBeInViewport()
    // the note the view landed on names the key now
    await expect(keyedTargets(code)).toHaveText(["to line 6"])
    // the diffs have the focus: their keys work at the place shown
    await expect(diffs).toBeFocused()
    await page.keyboard.press(back)
    await expectLandedOn(destination)
    await expect(source).not.toBeInViewport()
    await page.keyboard.press(forward)
    await expectLandedOn(source)
    // the jump back by the note there is a jump of its own: Back returns to where the lines left
    await moveTargets(source).click()
    await expectLandedOn(destination)
    await expect(diffs).toBeFocused()
    await page.keyboard.press(back)
    await expectLandedOn(source)
  })

  // the key from a row of the tree, the focus it moves to the diffs, and the second press from there: WebKit differs there
  test("G jumps from the note of a move in view to its other end and back, from the tree of the files and from the diffs", { tag: "@webkit" }, async ({ page, browserName }) => {
    const { window, code, diffs, position } = await openLedgerReview(page)
    const { source, destination } = accountsNotes(code)
    await expect(window.getByRole("treeitem", { name: "Ledger.kt" })).toBeFocused()

    // from the tree: the first note of a move in view is the one where `accounts()` stands now
    await page.keyboard.press("g")
    await expectLandedOn(source)
    await expect(destination).not.toBeInViewport()
    await expect(diffs).toBeFocused()
    // from the diffs, the note the jump landed on: back to where the lines stand
    await page.keyboard.press("g")
    await expectLandedOn(destination)
    await expect(source).not.toBeInViewport()
    // each press is a jump: Back returns along them
    await page.keyboard.press(historyKeys(browserName).back)
    await expectLandedOn(source)

    // with no note of a move in view the key moves nothing: Report.kt stays, and N goes on from its top
    await window.getByRole("treeitem", { name: "Report.kt" }).click()
    await expect(position).toHaveText("File 2 / 3")
    await diffs.focus()
    await page.keyboard.press("g")
    await page.keyboard.press("n")
    await expect(diffRows(code, "inline", { hasText: "connectStore(path)" }).first()).toBeInViewport()
    await expect(position).toHaveText("File 2 / 3")
    await expect(code.locator("[data-landed]")).toHaveCount(0)
  })

  // a button the paint replaces as its lines show, and the key of the notes after it: the focus of the diffs differs in WebKit
  test("H and a click show the lines a move took after the note that stands for them, and hide them again", { tag: "@webkit" }, async ({ page }) => {
    const { code, diffs } = await openLedgerReview(page)
    // to where `describe` left: its note is the one note in view that opens, and names the key of the notes
    const stands = moveNotes(code, "destination").filter({ hasText: "from line 46" })
    await scrollDiffsTo(code, stands)
    await moveTargets(stands).click()
    await expectLandedOn(moveNotes(code, "source").filter({ hasText: "7 lines moved" }))
    const toggle = code.getByRole("button", { name: /^7 lines moved to line 19: / })
    await expect(toggle).toHaveAccessibleName("7 lines moved to line 19: Show the 7 old lines")
    await expect(toggle).toHaveAttribute("aria-keyshortcuts", "H")
    await expect(code.getByRole("button", { name: /^5 lines moved to line 6: / })).not.toBeInViewport()

    // H: the seven lines show after the note, in the colour of a move, the edited one pale; the diffs keep the focus
    await page.keyboard.press("h")
    await expect(toggle).toHaveAttribute("aria-expanded", "true")
    const leftRows = movedRows(code, "source")
    await expect.poll(() => codeOf(leftRows)).toEqual(DESCRIBE_WAS)
    await expect(diffSides(diffRows(code, "deleted", { hasText: 'val text = "$sign$amount' }))).toHaveAttribute("data-tint", "pale")
    await expect(diffSides(leftRows.first())).not.toHaveAttribute("data-tint")
    await expect(diffs).toBeFocused()
    // a click on the note hides them, the diffs keeping the focus; H shows them again and hides them
    await toggle.click()
    await expect(toggle).toHaveAttribute("aria-expanded", "false")
    await expect(leftRows).toHaveCount(0)
    await expect(diffs).toBeFocused()
    await page.keyboard.press("h")
    await expect.poll(() => codeOf(leftRows)).toEqual(DESCRIBE_WAS)
    await page.keyboard.press("h")
    await expect(toggle).toHaveAttribute("aria-expanded", "false")
    await expect(leftRows).toHaveCount(0)

    // ⌥-click: every note of the file that opens follows it, the note where `accounts()` left too, above the view
    await toggle.click({ modifiers: ["Alt"] })
    await expect(toggle).toHaveAttribute("aria-expanded", "true")
    const accounts = code.getByRole("button", { name: /^5 lines moved to line 6: / })
    await expect(accounts).toHaveAttribute("aria-expanded", "true")
    await expect.poll(() => codeOf(leftRows)).toEqual(["", ...ACCOUNTS, ...DESCRIBE_WAS])
    // the notes where the lines stand now hide nothing: no button of them
    await expect(code.getByRole("button", { name: /^Moved from line/ })).toHaveCount(0)
    // and ⌥-click on an open one closes them all
    await accounts.click({ modifiers: ["Alt"] })
    await expect(accounts).toHaveAttribute("aria-expanded", "false")
    await expect(toggle).toHaveAttribute("aria-expanded", "false")
    await expect(leftRows).toHaveCount(0)
  })

  test("the menu of the code on a line of a move and on the note at its end shows the other end", async ({ page }) => {
    const { code } = await openLedgerReview(page)
    const { source, destination } = accountsNotes(code)
    const item = page.getByRole("menuitem", { name: "Go to Other End of Move" })

    // on a line where the lines stand now: the item, with the key of the review, which takes the note of this move
    await diffRows(code, "added", { hasText: "return names.distinct().sorted()" }).click({ button: "right" })
    await expect(item).toBeVisible()
    await expect(itemKey(item)).toHaveText("G")
    await item.click()
    await expectLandedOn(source)
    // on the note where they left
    await source.click({ button: "right", position: { x: 40, y: 8 } })
    await item.click()
    await expectLandedOn(destination)
    // on a line of the other move in view the item names no key: the key takes the note the view landed on
    await expect(keyedTargets(code)).toHaveText(["from line 22"])
    await diffRows(code, "added", { hasText: "return text.trim()" }).click({ button: "right" })
    await expect(item).toBeVisible()
    await expect(itemKey(item)).toHaveCount(0)
    await page.keyboard.press("Escape")
    await expectClosed(page.getByRole("menu"))
    // a line of no move has no such item
    await diffRows(code, "context", { hasText: "class Ledger(private val store: Store) {" }).click({ button: "right" })
    await expect(page.getByRole("menuitem", { name: "Go to Declaration" })).toBeVisible()
    await expect(item).toHaveCount(0)
  })

  // the menu a key opens reads the selection of the view, and gives the focus back to the diffs: WebKit differs there
  test(
    "the menu of the code a key opens on a line of a move shows the other end, and has no such item once the jump left nothing selected",
    { tag: "@webkit" },
    async ({ page }) => {
      const { code, diffs } = await openLedgerReview(page)
      const { source } = accountsNotes(code)
      const menu = page.getByRole("menu")
      const item = menu.getByRole("menuitem", { name: "Go to Other End of Move" })

      // a word of a line of `accounts()` where it stands now is selected: Shift+F10 opens the menu on that line
      await diffRows(code, "added", { hasText: "return names.distinct().sorted()" })
        .getByTestId("diff-code")
        .dblclick({ position: { x: 60, y: 8 } })
      await expect.poll(async () => (await selectedInDiffs(page)).text).toBe("return")
      await expect(diffs).toBeFocused()
      await page.keyboard.press("Shift+F10")
      await expect(item).toBeVisible()
      await expect(itemKey(item)).toHaveText("G")
      // the menu takes the focus a frame after it shows: its keys before that are the view's
      await expect(menu).toBeFocused()
      await item.click()
      await expectLandedOn(source)
      await expect(diffs).toBeFocused()

      // the jump selects nothing: the menu of a key is on no line, and the key of the moves goes on from the note
      expect((await selectedInDiffs(page)).text).toBe("")
      await page.keyboard.press("Shift+F10")
      await expect(menu.getByRole("menuitem", { name: "Go to Declaration" })).toBeVisible()
      await expect(item).toHaveCount(0)
      await page.keyboard.press("Escape")
      await expectClosed(menu)
      await expect(keyedTargets(code)).toHaveText(["to line 6"])
    },
  )

  test("the find searches the lines a move took at the note that stands for them and lands on it; opened, the hit is in the line", async ({ page }) => {
    const { window, code } = await openLedgerReview(page)
    const { source } = accountsNotes(code)
    // the line stands once where it is now, and once behind the note of where it left
    const { field, count } = await findInReview(window, code, "fun accounts")
    await expect(count).toHaveText("2 results")
    await page.keyboard.press("Enter")
    await expect(count).toHaveText("1 of 2")
    await expect.poll(() => joinedText(findMarks(diffRows(code, "added", { hasText: "fun accounts" }), "current"))).toBe("fun accounts")

    // the second hit is of an old line the note stands for: the view goes to the note, which names it
    await page.keyboard.press("Enter")
    await expect(count).toHaveText("2 of 2")
    const hits = remarkHits(source)
    await expect(hits).toHaveText("1 match in the old lines")
    await expect(hits).toHaveAttribute("data-hit", "current")
    await expect(source).toBeInViewport()
    // the note opened: the hit is in the line it stood for, still the current one
    await code.getByRole("button", { name: /^5 lines moved to line 6: / }).click()
    await expect.poll(() => joinedText(findMarks(diffRows(code, "deleted", { hasText: "fun accounts" }), "current"))).toBe("fun accounts")
    await expect(count).toHaveText("2 of 2")
    await expect(hits).toHaveCount(0)

    // text both lines of the edited line of `describe` have: the hit of its new line is in the row of the two lines,
    // the hit of its old line behind the note where the lines left, and the row marks only its own as the current one
    await field.fill("val text")
    await expect(count).toHaveText("2 results")
    const pair = diffRows(code, "inline", { hasText: "val text" })
    await page.keyboard.press("Enter")
    await expect(count).toHaveText("1 of 2")
    await expect.poll(() => joinedText(findMarks(pair, "current"))).toBe("val text")
    await page.keyboard.press("Enter")
    await expect(count).toHaveText("2 of 2")
    const described = remarkHits(moveNotes(code, "source").filter({ hasText: "7 lines moved" }))
    await expect(described).toHaveText("1 match in the old lines")
    await expect(described).toHaveAttribute("data-hit", "current")
    await expect.poll(() => joinedText(findMarks(pair))).toBe("val text")
    await expect(findMarks(pair, "current")).toHaveCount(0)
  })

  test("a copy takes the lines of a move once, where they stand now, and the lines that left too once their note is opened", async ({ page }) => {
    const { code, diffs } = await openLedgerReview(page)
    /** The side of Ledger.kt as the view shows it: ⌘A in the diffs, then the copy of the system */
    const copyFile = async (): Promise<string[]> => {
      await diffs.focus()
      await page.keyboard.press("ControlOrMeta+a")
      return ((await copiedByKey(page)) ?? "").split("\n")
    }
    const closed = await copyFile()
    expect(closed.filter(line => line === ACCOUNTS[0])).toHaveLength(1)
    expect(closed).not.toContain(DESCRIBE_WAS[0])
    expect(closed).toContain(DESCRIBE[0])
    // the notes give no text
    expect(closed.filter(line => line.includes("moved to line") || line.includes("Moved from"))).toEqual([])

    // the note of the lines `accounts()` left opened: they are lines of the view too; `describe` stays behind its note
    await moveTargets(accountsNotes(code).destination).click()
    await expectLandedOn(accountsNotes(code).source)
    await code.getByRole("button", { name: /^5 lines moved to line 6: / }).click()
    await expect.poll(() => codeOf(movedRows(code, "source"))).toEqual(["", ...ACCOUNTS])
    const open = await copyFile()
    expect(open.filter(line => line === ACCOUNTS[0])).toHaveLength(2)
    expect(open).not.toContain(DESCRIBE_WAS[0])
  })
})

test.describe("The moves of a commit a test makes, in the review window", () => {
  test("one note of a move names the key G at a time: the first one in view, the one N stopped under, the one a jump or Back landed on; the menu names the key on that end alone", async ({
    page,
    browserName,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("simple")
    await commitTwoMovedLines(repo)
    await syncAndWaitForBranches()
    // the file is a few rows: both ends of both its moves, every note of it, are in view at once
    const { window, code } = await openMarksReview(page, "config", "the timeouts first")
    await window.getByRole("treeitem", { name: "Config.kt" }).click()
    await expect(moveNotes(code)).toHaveCount(4)
    const diffs = code.locator("[aria-label='Changes of the commit']")
    const alpha = { stands: moveNotes(code, "destination").filter({ hasText: "from line 8" }), left: moveNotes(code, "source").filter({ hasText: "to line 4" }) }
    const beta = { stands: moveNotes(code, "destination").filter({ hasText: "from line 9" }), left: moveNotes(code, "source").filter({ hasText: "to line 6" }) }
    for (const note of [alpha.stands, alpha.left, beta.stands, beta.left]) {
      await expect(note).toBeInViewport()
    }
    const item = page.getByRole("menuitem", { name: "Go to Other End of Move" })
    /** The key the item shows in the menu of the code on the line `text`; none where the key of the moves takes another note */
    const keyInMenuOf = async (text: string, key: string | null) => {
      await diffRows(code, "added", { hasText: text }).click({ button: "right" })
      await expect(item).toBeVisible()
      await expect(itemKey(item)).toHaveText(key === null ? [] : [key])
      await page.keyboard.press("Escape")
      await expectClosed(page.getByRole("menu"))
    }

    // four notes in view, one hint: the first note of a move in view is the one the key takes
    await expect(keyedTargets(code)).toHaveText(["from line 8"])
    await keyInMenuOf("alphaTimeoutMilliseconds", "G")
    await keyInMenuOf("betaRetryCountMaximum", null)

    // N stops at the line of the first move, then at the line of the second: the note it stopped under takes the key,
    // though the first note is still in view above it
    await window.getByRole("treeitem", { name: "Config.kt" }).click()
    await page.keyboard.press("n")
    await expect(keyedTargets(code)).toHaveText(["from line 8"])
    await page.keyboard.press("n")
    await expect(keyedTargets(code)).toHaveText(["from line 9"])
    await expect(alpha.stands).toBeInViewport()
    await keyInMenuOf("betaRetryCountMaximum", "G")
    await keyInMenuOf("alphaTimeoutMilliseconds", null)

    // G goes from that note to where its line left, and back from the note it landed on
    await diffs.focus()
    await page.keyboard.press("g")
    await expectLandedOn(beta.left)
    await expect(keyedTargets(code)).toHaveText(["to line 6"])
    await page.keyboard.press("g")
    await expectLandedOn(beta.stands)
    await expect(keyedTargets(code)).toHaveText(["from line 9"])

    // Back and Forward land too: after a click on the place of the first move and Back, the key goes on from the
    // note Back returned to, never from the one the click landed on
    await moveTargets(alpha.stands).click()
    await expectLandedOn(alpha.left)
    await expect(keyedTargets(code)).toHaveText(["to line 4"])
    await page.keyboard.press(historyKeys(browserName).back)
    await expectLandedOn(alpha.stands)
    await expect(keyedTargets(code)).toHaveText(["from line 8"])
    await page.keyboard.press("g")
    await expectLandedOn(alpha.left)
    await expect(alpha.stands).not.toHaveAttribute("data-landed")
  })

  test("the find counts the word an edited line of a move lost once, marks it in the row of its two lines and lands on the note where the line left", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("simple")
    await commitMovedCompute(repo, { edited: true, apart: true })
    await syncAndWaitForBranches()
    const { window, code } = await openMarksReview(page, "store", "compute moves up")
    await window.getByRole("treeitem", { name: "Store.kt" }).click()
    // where the lines stand now, at the top of the file: the edited line is the row of its two lines, `oldBonus` struck through
    const pair = diffRows(code, "inline", { hasText: "val total = items.sum()" })
    await expect.poll(() => joinedText(deletedParts(pair))).toBe("oldBonus")
    const left = moveNotes(code, "source")
    await expect(left).not.toBeInViewport()

    // the old text holds the word once: one result, though the row of the pair shows it too, marked there
    const { count } = await findInReview(window, code, "oldBonus")
    await expect(count).toHaveText("1 result")
    await expect.poll(() => joinedText(findMarks(pair))).toBe("oldBonus")

    // the step goes to where the line left: the note that stands for it names the one hit, the current one
    await page.keyboard.press("Enter")
    await expect(count).toHaveText("1 of 1")
    await expect(left).toBeInViewport()
    // (eight lines here: with every function changed the last line of `compute`, a brace, pairs with another brace)
    await expect(left).toHaveText(/^8 lines moved to line 2/)
    const hits = remarkHits(left)
    await expect(hits).toHaveText("1 match in the old lines")
    await expect(hits).toHaveAttribute("data-hit", "current")
    await expect(pair).not.toBeInViewport()
    // a second step stays at the one hit
    await page.keyboard.press("Enter")
    await expect(count).toHaveText("1 of 1")
    await expect(left).toBeInViewport()

    // the note opened: the hit is in the line it stood for, still the one result
    await code.getByRole("button", { name: /^8 lines moved to line 2: / }).click()
    await expect.poll(() => joinedText(findMarks(diffRows(code, "deleted", { hasText: "val total = items.sum()" }), "current"))).toBe("oldBonus")
    await expect(count).toHaveText("1 of 1")
    await expect(hits).toHaveCount(0)
  })
})

test.describe("The moves of a diff in a review with its plan", () => {
  test("a move with an edited line says its words in the heading of the change: a click on its place and G go to where the lines left and back to the heading", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("simple")
    await commitMovedCompute(repo, { edited: true })
    turnOnReviewPlan(repo)
    const window = await openPlanWindow(page, syncAndWaitForBranches, "store")
    const code = window.getByTestId("commit-review-code")
    // where the lines stand now: the heading of the change and the words of the move in one row, which names the key
    const heading = moveNotes(code, "destination")
    await expect(heading).toHaveText("compute — body changed · Moved from line 121 · 1 line edited")
    await expect(heading).toHaveAttribute("data-tone", "unit")
    await expect(heading.getByTestId("diff-unit-heading")).toHaveText("compute — body changed")
    await expect(keyedTargets(code)).toHaveText(["from line 121"])

    // a click on the place: the note where the lines left, marked, the diffs holding the focus
    await moveTargets(heading).click()
    const left = moveNotes(code, "source")
    await expectLandedOn(left)
    await expect(left).toHaveText("10 lines moved to line 2")
    await expect(code.locator("[aria-label='Changes of the commit']")).toBeFocused()
    // G from there: back to the row of the heading, which is the note of that end
    await window.keyboard.press("g")
    await expectLandedOn(heading)
    await expect(left).not.toHaveAttribute("data-landed")
    await window.keyboard.press("g")
    await expectLandedOn(left)
  })

  test("a move as it was is in the folded section of the moves: a jump in it goes there and back, and Forward opens the section folded again", async ({
    page,
    browserName,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("simple")
    await commitMovedCompute(repo, { edited: false })
    turnOnReviewPlan(repo)
    const window = await openPlanWindow(page, syncAndWaitForBranches, "store")
    const code = window.getByTestId("commit-review-code")
    const { back, forward } = historyKeys(browserName)
    // the lines moved as they were: nothing of them to read, so their section stands folded at the end, no note shown
    await expect(moveNotes(code)).toHaveCount(0)
    const expand = code.getByRole("button", { name: "Expand Store.kt" })
    await expect(expand).toHaveCount(1)
    await expand.click()
    const stands = moveNotes(code, "destination")
    const left = moveNotes(code, "source")
    await expect(stands).toHaveText("Moved from line 121")
    await expect(left).toHaveText("10 lines moved to line 2")

    await moveTargets(stands).click()
    await expectLandedOn(left)
    await window.keyboard.press("g")
    await expectLandedOn(stands)
    await window.keyboard.press(back)
    await expectLandedOn(left)
    // the section folded again: Forward to the place in it opens it
    await code.getByRole("button", { name: "Collapse Store.kt" }).last().click()
    await expect(moveNotes(code)).toHaveCount(0)
    await window.keyboard.press(forward)
    await expectLandedOn(stands)
  })

  test("imports that were sorted stay the pale lines of a block the review plays down, with no note of a move", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("simple")
    // four imports put in order, two of them moves to the engine, and a line of the body changed
    const sorted = (imports: string[], value: number) =>
      `package demo\n\n${imports.map(name => `import ${name}\n`).join("")}\nclass Sorted {\n  fun total(items: List<Int>): Int {\n    return items.sum() + ${value}\n  }\n}\n`
    const imports = [
      "org.example.zeta.ZetaServiceProvider",
      "org.example.alpha.AlphaServiceProvider",
      "org.example.middle.MiddleServiceProvider",
      "org.example.beta.BetaServiceProvider",
    ]
    // the section of the imports, folded at the end: opened, the two imports that left and the two where they stand
    const { code } = await openSortedPlan(page, repo, syncAndWaitForBranches, sorted(imports, 1), sorted(imports.toSorted(), 2))
    const moved = diffRows(code).filter({ hasText: /import org\.example\.(middle|zeta)\./ })
    await expect
      .poll(() => codeOf(moved))
      .toEqual([
        "import org.example.zeta.ZetaServiceProvider",
        "import org.example.middle.MiddleServiceProvider",
        "import org.example.middle.MiddleServiceProvider",
        "import org.example.zeta.ZetaServiceProvider",
      ])
    // each a deleted or an inserted line, pale as a change of whitespace alone: none in the colour of a move, no note
    await expect.poll(() => cellsOf(moved)).toEqual(["deleted whitespace", "deleted whitespace", "inserted whitespace", "inserted whitespace"])
    await expect(moveNotes(code)).toHaveCount(0)
    await expect(movedSides(code)).toHaveCount(0)
  })

  test("imports that left a block with code for the imports, one edited on the way, stay the deleted lines of their block, none behind a note of a move", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("simple")
    // five imports, then a comment and four more; the commit puts the four first, one from another package now, and
    // deletes the comment: to the engine a move of four lines with one edited, out of the block of the comment into a
    // block of the imports alone
    const core = ["Registry", "Settings", "StartupHooks", "Shutdown", "Lifecycle"].map(name => `import org.example.core.Core${name}\n`).join("")
    const providers = (alpha: string) =>
      ["org.example.zeta.Zeta", `org.example.${alpha}.Alpha`, "org.example.middle.Middle", "org.example.omega.Omega"].map(name => `import ${name}ServiceProvider`)
    const body = (value: number) => `\nclass Sorted {\n  fun total(items: List<Int>): Int {\n    return items.sum() + ${value}\n  }\n}\n`
    const lines = (imports: string[]) => imports.map(line => `${line}\n`).join("")
    const before = `package demo\n\n${core}// the providers of the services\n${lines(providers("alpha"))}${body(1)}`
    const { code, own, imports } = await openSortedPlan(page, repo, syncAndWaitForBranches, before, `package demo\n\n${lines(providers("beta"))}${core}${body(2)}`)
    // the section of the file: the comment and the four imports that left are deleted lines of their block, every
    // one shown, the import as it was before its edit among them
    await expect.poll(() => codeOf(diffRows(own, "deleted"))).toEqual(["// the providers of the services", ...providers("alpha")])
    await expect.poll(() => cellsOf(diffRows(own, "deleted"))).toEqual(Array.from({ length: 5 }, () => "deleted null"))
    // the section of the imports: the four imports where they stand now are inserted lines, pale as the block the
    // review plays down, the edited one as it is now
    await expect.poll(() => codeOf(diffRows(imports, "added"))).toEqual(providers("beta"))
    await expect.poll(() => cellsOf(diffRows(imports, "added"))).toEqual(Array.from({ length: 4 }, () => "inserted whitespace"))
    // the move is drawn at neither end: no note says that lines moved, none hides a line, no line has its colour
    await expect(moveNotes(code)).toHaveCount(0)
    await expect(movedSides(code)).toHaveCount(0)
    await expect(diffRows(code, "inline", { hasText: "AlphaServiceProvider" })).toHaveCount(0)
  })

  test("the row of two lines of an import that a review plays down is reached by the scroll of its column, though its old line is a move to the engine", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("simple")
    // a long import gets a longer name in place, and its old form is added again below two unchanged imports in
    // place of a short one: the engine pairs the two lines of the first block, and the marks call the old line a move
    // to the second block. Both blocks are of the imports alone
    const long = "organization.example.platform.services.providers.implementation.registry.internal.registration.factory.implementation.platform.defaults"
    const alpha = `import ${long}.AlphaService`
    const text = (first: string, second: string, value: number) =>
      `package demo\n\n${first}\nimport org.example.Unchanged\nimport org.example.UnchangedToo\n${second}\n\nclass Sorted {\n  fun total(items: List<Int>): Int {\n    return items.sum() + ${value}\n  }\n}\n`
    const after = text(`import ${long}.AlphaLegacyServiceProvider`, alpha, 2)
    const { code, imports } = await openSortedPlan(page, repo, syncAndWaitForBranches, text(alpha, "import org.example.short.Gamma", 1), after)
    // the review draws no move in these blocks: the first is the row of its two lines, the old name struck through
    // before the new one, wider than either line
    const pair = diffRows(imports, "inline")
    await expect(deletedParts(pair)).toHaveText("AlphaService")
    await expect(changedParts(pair)).toHaveText("AlphaLegacyServiceProvider")
    await expect(moveNotes(code)).toHaveCount(0)
    // the row is wider than its column, and the scroll bar of the column reaches its end
    await expectBarReachesEndOf(pair, horizontalBars(imports))
  })
})

/**
 * The moves of the preview of the template in the theme of the page: the lines of a move and the notes at its ends
 * have the colour the token `--diff-moved` names, its edited line the one `--diff-moved-pale` names, and both are the
 * colour of no other line of a diff; the tokens are light or dark as the theme. The lines and the notes of a move have
 * a rail down their start, which no other line has: a move is told without its colour
 */
async function expectMovedColour(page: Page, theme: "light" | "dark"): Promise<void> {
  const { ledger } = await openLedgerPreview(page)
  await expect(moveNotes(ledger, "destination")).toHaveCount(2)
  const { tokens, line, edited, note, rails } = await ledger.evaluate(element => {
    const background = (selector: string) => getComputedStyle(element.querySelector(selector)!).backgroundColor
    const railed = (selector: string) => getComputedStyle(element.querySelector(selector)!).boxShadow !== "none"
    const named = (token: string) => {
      const probe = document.createElement("div")
      probe.style.background = `var(${token})`
      element.append(probe)
      const colour = getComputedStyle(probe).backgroundColor
      probe.remove()
      return colour
    }
    return {
      tokens: Object.fromEntries(
        ["moved", "moved-pale", "inserted", "inserted-pale", "deleted", "deleted-pale", "modified", "modified-pale", "quiet", "separator"].map(name => [
          name,
          named(`--diff-${name}`),
        ]),
      ),
      line: background('[data-testid=diff-side][data-cell="moved"]:not([data-tint])'),
      edited: background('[data-testid=diff-side][data-cell="moved"][data-tint="pale"]'),
      note: background("[data-testid=diff-row][data-tone=remark][data-move]"),
      rails: [
        '[data-testid=diff-side][data-cell="moved"]:not([data-tint])',
        '[data-testid=diff-side][data-cell="moved"][data-tint="pale"]',
        "[data-testid=diff-row][data-tone=remark][data-move]",
        '[data-testid=diff-side][data-cell="inserted"]',
        '[data-testid=diff-side][data-cell="context"]',
        "[data-testid=diff-row][data-tone=remark][data-quiet]",
      ].map(railed),
    }
  })
  expect(rails).toEqual([true, true, true, false, false, false])
  expect([line, note, edited]).toEqual([tokens.moved, tokens.moved, tokens["moved-pale"]])
  // one colour each: no two tokens of a line of a diff are the same
  expect(new Set(Object.values(tokens)).size).toBe(Object.keys(tokens).length)
  // the first channel of the colour: light on the light card, dark on the dark one
  expect(Number(/\d+/.exec(tokens.moved!)![0]) > 128 ? "light" : "dark").toBe(theme)
}

test.describe("The colour of a move in the light theme", () => {
  test.use({ colorScheme: "light" })

  test("the lines of a move and the notes at its ends have the moved colour of the light theme, which is the colour of no other line", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    await setupRepo("commit_marks")
    await syncAndWaitForBranches()
    await expectMovedColour(page, "light")
  })
})

test.describe("The colour of a move in the dark theme", () => {
  test.use({ colorScheme: "dark" })

  test("the lines of a move and the notes at its ends have the moved colour of the dark theme, which is the colour of no other line", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    await setupRepo("commit_marks")
    await syncAndWaitForBranches()
    await expectMovedColour(page, "dark")
  })
})
