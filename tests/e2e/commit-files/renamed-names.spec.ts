// @spec openspec/specs/commit/quiet-changes/spec.md
import type { Locator, Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { expandBranch, findBranchRow, getBranchCommitRow } from "../helpers/branch-helpers"
import {
  deletedParts,
  diffRows,
  diffSides,
  editedParts,
  findMarks,
  joinedText,
  quietSides,
  remarkHits,
  remarks,
  RENAMED_NAME,
  RENAMED_NAME_START,
  renamedNames,
  scrollDiffsTo,
} from "../helpers/diff-helpers"
import { codeOf, findInReview, MARKS_SUBJECT, openLedgerPreview, openMarksReview, switchRenamedOn } from "../helpers/marks-helpers"
import { openCommitFilesSection, openCommitPreview } from "../helpers/preview-helpers"
import { openPlanWindow, turnOnReviewPlan } from "../helpers/review-plan-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"

// the renames of the `commit_marks` template (`MARKS_SUBJECT`). In src/Ledger.kt the local `total` of `balance` is
// `sum` at its three places, each on a line that changed in nothing else: its declaration at the new line 28, a use at
// 31 and one at 34. In src/Store.kt the function `openStore` is `connectStore`: a name that is not local, declared
// there and used in src/Report.kt alone, which declares none of it. The runs of a renamed name are labels until a
// measurement switches their kind on, so every such line is read and no note opens. What a view shows of a quiet run
// of a rename is tested on the answers of the engine with that kind switched on (`switchRenamedOn`): the three lines
// of `sum` are quiet then, and their blocks nothing to read
const NOTE = "Renamed total → sum · 3 sites"
const NEW_LINES = ["    var sum = 0L", "        sum += entry.amount", "    return sum"]
const OLD_LINES = NEW_LINES.map(line => line.replace("sum", "total"))
/** The text of the rows of Inline: the old name struck through before the new one */
const PAIR_LINES = NEW_LINES.map(line => line.replace("sum", "totalsum"))

/** The old names of renames that the rows of a pair of Inline strike through (`data-renamed` on a deleted part) */
const STRUCK_NAME = "[data-inline-deleted][data-renamed]"

/** What the note of the rename is named once it opens: the three old lines of the sites, which Inline hides */
const NOTE_NAME = "Renamed total to sum, 3 sites"

/** The rows of the lines that hold a site of a rename, of one kind when given: a name in the mark of a renamed name */
function siteRows(scope: Locator, kind?: "inline" | "deleted" | "added" | "changed" | "once"): Locator {
  return diffRows(scope, kind).filter({ has: scope.page().locator(RENAMED_NAME) })
}

/** The card of Ledger.kt in the diffs of a review: the sites of `sum` are its, those of the function the store renamed another file's */
function ledgerOf(code: Locator): Locator {
  return code.getByRole("region", { name: "src/Ledger.kt" })
}

/** What only the line of each site of `sum` holds, in the order of the lines: its declaration, the use in the loop, the result */
const SITE_TEXTS = ["var ", "+= entry.amount", "return "] as const

/**
 * The row of site `at` of `sum` among `rows`, the rows of its sites: by the text of its line, never by its place in
 * the page, which is the order of the paints
 */
function siteRow(rows: Locator, at: 0 | 1 | 2): Locator {
  return rows.filter({ hasText: SITE_TEXTS[at] })
}

/** The kind of the row right above each site of `sum` among `rows`, in the order of the lines */
async function kindsAboveSites(rows: Locator): Promise<(string | undefined)[]> {
  return [await kindAbove(siteRow(rows, 0)), await kindAbove(siteRow(rows, 1)), await kindAbove(siteRow(rows, 2))]
}

/** The kind (`data-kind`) of each of `rows`, in the order of the page */
async function kindsOf(rows: Locator): Promise<(string | undefined)[]> {
  return rows.evaluateAll(elements => elements.map(element => (element as HTMLElement).dataset.kind))
}

/**
 * The kind (`data-kind`) and the code of the row right above `row`, one row, in its file. Read in one step of the page
 * (`evaluateAll`): the painter replaces the element of a row as its tokens arrive, and a row found in one step and read
 * in the next may have left the page by then
 */
async function rowAbove(row: Locator): Promise<{ kind: string | undefined; code: string }> {
  const above = await row.evaluateAll(elements =>
    elements.map(element => {
      const found = element.parentElement?.querySelector<HTMLElement>(`[data-testid=diff-row][data-row="${Number((element as HTMLElement).dataset.row) - 1}"]`)
      return { kind: found?.dataset.kind, code: found?.querySelector("[data-testid=diff-code]")?.textContent ?? "" }
    }),
  )
  expect(above).toHaveLength(1)
  return above[0]!
}

/** The kind (`data-kind`) of the row right above `row` in its file */
async function kindAbove(row: Locator): Promise<string | undefined> {
  return (await rowAbove(row)).kind
}

/** The one row of `scope` whose note of renames names `name`, once it is there: what a row of words is found by */
async function rowOfNote(scope: Locator, name: string): Promise<Locator> {
  const row = diffRows(scope, "note").filter({ has: remarks(scope.page(), "renamed").filter({ hasText: name }) })
  await expect(row).toHaveCount(1)
  return row
}

/** The diff of the file `name` of the commit of the marks, under its row of the tree of the files of the preview, once it shows `shows` */
async function openFileOf(tree: Locator, name: string, shows: string): Promise<Locator> {
  const file = tree.getByRole("treeitem", { name })
  await file.getByText(name).click()
  await expect(file).toContainText(shows)
  return file
}

/** The note of the rename of `total` in `scope`, once the diffs show it, with its words */
async function shownNote(scope: Locator): Promise<Locator> {
  const note = remarks(scope, "renamed").filter({ hasText: "total" })
  await scrollDiffsTo(scope, note)
  await expect(note).toHaveText([NOTE])
  return note
}

/** `note` is plain words that open nothing: no line of its rename is hidden */
async function expectPlainWords(note: Locator): Promise<void> {
  await expect(note).not.toHaveAttribute("aria-expanded")
  await expect(note).not.toHaveRole("button")
}

/**
 * One column shows both lines of each site in `scope`: the old line, then the new one, the name alone marked on
 * both, as renamed, and the note before the old line of the declaration. Gives the rows of the old lines and of the
 * new ones
 */
async function expectBothLinesStacked(scope: Locator): Promise<{ old: Locator; added: Locator }> {
  const [old, added] = [siteRows(scope, "deleted"), siteRows(scope, "added")]
  await expect.poll(() => codeOf(old)).toEqual(OLD_LINES)
  await expect.poll(() => codeOf(added)).toEqual(NEW_LINES)
  await expect(siteRows(scope)).toHaveCount(6)
  await expect(renamedNames(old)).toHaveText(["total", "total", "total"])
  await expect(renamedNames(added)).toHaveText(["sum", "sum", "sum"])
  await expect(renamedNames(old).first()).toHaveAttribute("title", "Renamed to sum")
  await expect(editedParts(siteRows(scope))).toHaveCount(0)
  expect(await kindAbove(siteRow(old, 0))).toBe("note")
  return { old, added }
}

/** Split shows a row a site in `scope`: the old name in the old column, the new one in the new column, each alone marked. Gives the rows */
async function expectBothLinesBeside(scope: Locator): Promise<Locator> {
  const rows = siteRows(scope, "changed")
  await expect.poll(() => codeOf(diffSides(rows, 0))).toEqual(OLD_LINES)
  await expect.poll(() => codeOf(diffSides(rows, 1))).toEqual(NEW_LINES)
  await expect(renamedNames(diffSides(rows, 0))).toHaveText(["total", "total", "total"])
  await expect(renamedNames(diffSides(rows, 1))).toHaveText(["sum", "sum", "sum"])
  await expect(editedParts(rows)).toHaveCount(0)
  return rows
}

/** The layout `name` chosen in the preview, and the note of the rename of `ledger` in it */
async function inLayout(preview: Locator, ledger: Locator, name: "Unified" | "Split"): Promise<Locator> {
  await preview.getByRole("button", { name, exact: true }).click()
  return shownNote(ledger)
}

/** ⌘F for `total` in the review and a step: three results, the first the current one. The field and the count */
async function findOldName(page: Page, window: Locator, code: Locator): Promise<{ field: Locator; count: Locator }> {
  const found = await findInReview(window, code, "total")
  await expect(found.count).toHaveText("3 results")
  await page.keyboard.press("Enter")
  await expect(found.count).toHaveText("1 of 3")
  return found
}

test.describe("The renames of a diff in the preview of a commit", () => {
  test.beforeEach(async ({ setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("commit_marks")
    await syncAndWaitForBranches()
  })

  test("Inline names a rename once, before the line of its declaration, and shows each site as one row, the old name struck through before the new one, both in the mark of a renamed name", async ({
    page,
  }) => {
    const { ledger } = await openLedgerPreview(page)
    // one note for the three sites, in plain words: every line of a label is read, so it opens nothing
    await expectPlainWords(await shownNote(ledger))
    // each line of the name is the row of its two lines: nothing of a line that is read is left out
    const rows = siteRows(ledger)
    await expect.poll(() => codeOf(rows)).toEqual(PAIR_LINES)
    expect(await kindsOf(rows)).toEqual(["inline", "inline", "inline"])
    // the new name in the mark of a renamed name, the old one struck through in it, and no other changed part
    await expect(renamedNames(rows)).toHaveText(["sum", "sum", "sum"])
    await expect(deletedParts(rows)).toHaveText(["total", "total", "total"])
    await expect(rows.locator(STRUCK_NAME)).toHaveCount(3)
    // struck through, though the syntax underlines the name of a reassigned variable
    await expect(rows.locator(STRUCK_NAME).first()).toHaveCSS("text-decoration-line", "line-through")
    await expect(editedParts(rows)).toHaveCount(0)
    // what the name was, and what it became, is the hover of each
    await expect(renamedNames(rows).first()).toHaveAttribute("title", "Renamed from total")
    await expect(deletedParts(rows).first()).toHaveAttribute("title", "Renamed to sum")
    // the lines are changes to read, never played down
    await expect(quietSides(rows)).toHaveCount(0)
    await expect(rows.locator('[data-testid=diff-side][data-cell="inserted"]')).toHaveCount(3)
    // the note is the row before the declaration; the other sites stand after an unchanged line, with no note
    expect(await kindsAboveSites(rows)).toEqual(["note", "context", "context"])
    // each name is one part, which says to a screen reader that the name starts there
    await expect(rows.locator(RENAMED_NAME_START)).toHaveText(["sum", "sum", "sum"])
    // a real change of the file is drawn as it was: the old words struck through before the new ones, as a change
    const real = diffRows(ledger, "inline", { hasText: "fun isEmpty(): Boolean" })
    await scrollDiffsTo(ledger, real)
    expect(await joinedText(deletedParts(real))).toBe("isEmpty")
    expect(await joinedText(editedParts(real))).toBe("none")
    await expect(renamedNames(real)).toHaveCount(0)
    await expect(real.locator(STRUCK_NAME)).toHaveCount(0)
  })

  test("Unified and Split keep both lines of each site of a rename, the old name and the new one in the mark of a renamed name", async ({ page }) => {
    const { preview, ledger } = await openLedgerPreview(page)
    await inLayout(preview, ledger, "Unified")
    // each site is its old line, then its new one, lines to read: the name alone is marked, as renamed, on both
    await expectBothLinesStacked(ledger)
    await expect(quietSides(siteRows(ledger))).toHaveCount(0)

    await inLayout(preview, ledger, "Split")
    // a row a site: the old name in the old column, the new one in the new column
    await expect(quietSides(await expectBothLinesBeside(ledger))).toHaveCount(0)
  })

  test("a name that is not local is named at its declaration, which is read, and once in a file that only uses it, each use in the mark of a renamed name", async ({ page }) => {
    await expandBranch(page, findBranchRow(page, "ledger"))
    const preview = await openCommitPreview(page, getBranchCommitRow(page, "ledger", MARKS_SUBJECT))
    const tree = await openCommitFilesSection(preview)
    const store = await openFileOf(tree, "Store.kt", "return Store(path)")
    // the declaration of the function: the one site of the file, under the note of the rename, a change to read; the
    // note counts the places of the commit, the two calls of the report among them
    await expect(remarks(store, "renamed")).toHaveText(["Renamed openStore → connectStore · 3 sites in 2 files"])
    const declaration = siteRows(store, "inline")
    await expect.poll(() => codeOf(declaration)).toEqual(["fun openStoreconnectStore(path: String): Store {"])
    await expect(renamedNames(declaration)).toHaveText(["connectStore"])
    await expect(renamedNames(declaration)).toHaveAttribute("title", "Renamed from openStore")
    await expect(declaration.locator(STRUCK_NAME)).toHaveText(["openStore"])
    await expect(editedParts(declaration)).toHaveCount(0)
    expect(await kindAbove(declaration)).toBe("note")
    // Report.kt calls the function and declares none of it: the files of the commit say that the store declares the
    // name, so the report names the rename once, at its first call, and draws the name at both calls in the mark of a
    // renamed name, never as a change (subject-marks.spec.ts: the file the note names, what the index proved)
    const report = await openFileOf(tree, "Report.kt", "class Report(private val ledger: Ledger) {")
    await expect(remarks(report, "renamed")).toHaveText(["Renamed openStore → connectStore · 2 sites"])
    const calls = siteRows(report, "inline")
    await expect(calls).toHaveCount(2)
    await expect(renamedNames(calls)).toHaveText(["connectStore", "connectStore"])
    await expect(calls.locator(STRUCK_NAME)).toHaveText(["openStore", "openStore"])
    await expect(editedParts(calls)).toHaveCount(0)
    expect(await joinedText(deletedParts(calls.first()))).toBe("openStore")
  })
})

test.describe("The quiet runs of a renamed local in the preview of a commit", () => {
  test.beforeEach(async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("commit_marks")
    await syncAndWaitForBranches()
    await switchRenamedOn(page)
  })

  test("Inline shows each line of a quiet rename once, the new name in its mark, and the one note of the rename shows and hides the old line of every site", async ({ page }) => {
    const { ledger } = await openLedgerPreview(page)
    // the three blocks of the name are nothing to read: five of the eleven are left
    await expect(ledger.getByTestId("file-to-read")).toHaveText("5 to read")
    await expect(ledger.getByTestId("file-to-read")).toHaveAttribute("title", "5 of 11 changes to read")
    // one note for the three sites, the button that shows the lines they stand for; the preview has no key of the notes
    const note = await shownNote(ledger)
    await expect(note).toHaveRole("button")
    await expect(note).toHaveAccessibleName(`${NOTE_NAME}: Show the 3 old lines`)
    await expect(note).toHaveAttribute("aria-expanded", "false")
    await expect(note).not.toHaveAttribute("aria-keyshortcuts")
    // each line of the name once, by its new line: played down, the new name in the mark of a renamed name, which
    // says what it was, and nothing else of the line marked or struck through
    const once = siteRows(ledger, "once")
    await expect.poll(() => codeOf(once)).toEqual(NEW_LINES)
    await expect(siteRows(ledger)).toHaveCount(3)
    await expect(quietSides(once)).toHaveCount(3)
    await expect(diffSides(once).first()).toHaveAttribute("data-cell", "once")
    await expect(diffSides(once).first()).toHaveAttribute("data-run", "renamed")
    await expect(renamedNames(once)).toHaveText(["sum", "sum", "sum"])
    await expect(renamedNames(once).first()).toHaveAttribute("title", "Renamed from total")
    await expect(editedParts(once)).toHaveCount(0)
    await expect(deletedParts(once)).toHaveCount(0)
    // the note is the row before the declaration; the other sites stand after an unchanged line, with no note
    expect(await kindsAboveSites(once)).toEqual(["note", "context", "context"])

    // a click: the old line of every site shows before its new one, in its block and in the two others, all played
    // down, the old name in the mark on the old lines
    await note.click()
    await expect(note).toHaveAttribute("aria-expanded", "true")
    await expect(note).toHaveAccessibleName(`${NOTE_NAME}: Hide the 3 old lines`)
    const { old, added } = await expectBothLinesStacked(ledger)
    await expect(quietSides(old)).toHaveCount(3)
    await expect(quietSides(added)).toHaveCount(3)
    await expect(once).toHaveCount(0)
    expect(await kindsAboveSites(old)).toEqual(["note", "context", "context"])

    // a second click hides them again
    await note.click()
    await expect(note).toHaveAttribute("aria-expanded", "false")
    await expect.poll(() => codeOf(once)).toEqual(NEW_LINES)
    await expect(old).toHaveCount(0)
    // the real change of the file is read as before
    const real = diffRows(ledger, "inline", { hasText: "fun isEmpty(): Boolean" })
    await scrollDiffsTo(ledger, real)
    expect(await joinedText(editedParts(real))).toBe("none")
    await expect(quietSides(real)).toHaveCount(0)
  })

  test("Unified and Split keep both lines of each site of a quiet rename, played down, the name in its mark on both, under a note that opens nothing", async ({ page }) => {
    const { preview, ledger } = await openLedgerPreview(page)
    // plain words: no line is hidden
    await expectPlainWords(await inLayout(preview, ledger, "Unified"))
    // each site is its old line, then its new one, each of its type and played down, the name alone marked
    await expectBothLinesStacked(ledger)
    await expect(quietSides(siteRows(ledger))).toHaveCount(6)
    await expect.poll(() => quietSides(siteRows(ledger)).evaluateAll(sides => sides.map(side => side.getAttribute("data-run")))).toEqual(Array.from({ length: 6 }, () => "renamed"))
    await expect(diffRows(ledger, "once")).toHaveCount(0)

    await expectPlainWords(await inLayout(preview, ledger, "Split"))
    // a row a site: the old name in the old column, the new one in the new column, both played down
    const rows = await expectBothLinesBeside(ledger)
    await expect(quietSides(rows, 0)).toHaveCount(3)
    await expect(quietSides(rows, 1)).toHaveCount(3)
  })
})

test.describe("The renames of a diff in the review window", () => {
  test(
    "the find for the old name of a rename marks it where a row shows it: struck through in the rows of Inline, in the old lines of Unified, and never in the new name",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await setupRepo("commit_marks")
      await syncAndWaitForBranches()
      const { window, code } = await openMarksReview(page, "ledger", MARKS_SUBJECT)
      // Inline strikes the old name through at each site: three results, each in its row
      const { field, count } = await findOldName(page, window, code)
      const rows = siteRows(ledgerOf(code), "inline")
      await expect.poll(() => codeOf(rows)).toEqual(PAIR_LINES)
      await expect(findMarks(rows)).toHaveText(["total", "total", "total"])
      await expect(rows.locator(`${STRUCK_NAME}[data-hit]`)).toHaveCount(3)
      await expect(findMarks(siteRow(rows, 0), "current")).toHaveText("total")
      await expect(findMarks(code, "current")).toHaveCount(1)
      await expect(siteRow(rows, 0)).toBeInViewport()
      // a hit keeps its colour on the struck-through name: not the colour of a renamed name
      const struck = siteRow(rows, 2).locator(STRUCK_NAME)
      const colour = () => struck.evaluate(element => getComputedStyle(element).backgroundColor)
      const marked = await colour()
      // a step goes to the next site, in the order of the lines
      await page.keyboard.press("Enter")
      await expect(count).toHaveText("2 of 3")
      await expect(findMarks(siteRow(rows, 1), "current")).toHaveText("total")
      await expect(findMarks(siteRow(rows, 0), "current")).toHaveCount(0)
      // the new name is found as any text of the lines, and is no hit of the old one
      await field.fill("sum +=")
      await expect(count).toHaveText("1 result")
      await expect.poll(() => joinedText(findMarks(siteRow(rows, 1)))).toBe("sum +=")
      await expect(struck).not.toHaveAttribute("data-hit")
      expect(await colour()).not.toBe(marked)
      // a hit that holds a part of the new name cuts it in two parts: the first alone says where the name starts, so
      // a screen reader hears "renamed" once a name
      await field.fill("um +=")
      await expect(count).toHaveText("1 result")
      await expect(renamedNames(siteRow(rows, 1))).toHaveText(["s", "um"])
      await expect(siteRow(rows, 1).locator(RENAMED_NAME_START)).toHaveText(["s"])
      await expect(rows.locator(RENAMED_NAME_START)).toHaveCount(3)

      // Unified shows the old lines: the old name is found in them
      await field.fill("total")
      await window.getByRole("button", { name: "Unified", exact: true }).click()
      await expect(count).toHaveText("3 results")
      await expect(findMarks(siteRows(ledgerOf(code), "deleted"))).toHaveText(["total", "total", "total"])
      await expect(findMarks(siteRows(ledgerOf(code), "added"))).toHaveCount(0)
    },
  )
})

test.describe("The quiet runs of a renamed local in the review window", () => {
  test.beforeEach(async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("commit_marks")
    await syncAndWaitForBranches()
    await switchRenamedOn(page)
  })

  // the focus stays with the diffs through a click on a button the paint replaces, and the key of the notes from the
  // diffs: WebKit differs there
  test("the note of a quiet rename shows the old line of every site by a click, by ⌥-click with the other notes of its file, and by H", { tag: "@webkit" }, async ({ page }) => {
    const { window, code } = await openMarksReview(page, "ledger", MARKS_SUBJECT)
    const diffs = code.locator("[aria-label='Changes of the commit']")
    await window.getByRole("treeitem", { name: "Ledger.kt" }).click()
    const note = await shownNote(code)
    await expect(note).toHaveAccessibleName(`${NOTE_NAME}: Show the 3 old lines`)
    await expect(note).toHaveAttribute("aria-expanded", "false")
    // in a review the note names the key that opens it
    await expect(note).toHaveAttribute("aria-keyshortcuts", "H")
    const [once, old] = [siteRows(ledgerOf(code), "once"), siteRows(ledgerOf(code), "deleted")]
    await expect.poll(() => codeOf(once)).toEqual(NEW_LINES)

    // a click: the three old lines show, the declaration's under the note and the two others in their own blocks; the
    // diffs keep the focus, so their keys go on
    await note.click()
    await expect(note).toHaveAttribute("aria-expanded", "true")
    await expect.poll(() => codeOf(old)).toEqual(OLD_LINES)
    await expect(once).toHaveCount(0)
    await expect(diffs).toBeFocused()
    await note.click()
    await expect(note).toHaveAttribute("aria-expanded", "false")
    await expect(old).toHaveCount(0)

    // ⌥-click on it opens every note of the file that opens, the one of the wrapped call among them, and closes them all
    const wrapped = remarks(code, "formatting")
    await note.click({ modifiers: ["Alt"] })
    await expect(note).toHaveAttribute("aria-expanded", "true")
    await expect.poll(() => codeOf(old)).toEqual(OLD_LINES)
    // the note stays under the pointer, though the notes above it took rows: the view keeps its place by the row at
    // its top as it stood at the press, also right after the scroll that brought the note into view
    await expect(note).toBeInViewport()
    await window.getByRole("treeitem", { name: "Ledger.kt" }).click()
    await expect(wrapped).toHaveAttribute("aria-expanded", "true")
    await scrollDiffsTo(code, note)
    await note.click({ modifiers: ["Alt"] })
    await expect(note).toHaveAttribute("aria-expanded", "false")
    await expect(old).toHaveCount(0)

    // H, the note in view: its old lines show, and go again
    await expect(note).toBeInViewport()
    await diffs.focus()
    await page.keyboard.press("h")
    await expect(note).toHaveAttribute("aria-expanded", "true")
    await expect.poll(() => codeOf(old)).toEqual(OLD_LINES)
    await expect(diffs).toBeFocused()
    await page.keyboard.press("h")
    await expect(note).toHaveAttribute("aria-expanded", "false")
    await expect.poll(() => codeOf(once)).toEqual(NEW_LINES)
  })

  // the field of the find takes the focus from the diffs and the hits are painted into rows painted anew: WebKit differs there
  test(
    "the find for the old name of a quiet rename lands on the row that shows its new line and marks the new name; under the opened note the hit is in the old line",
    { tag: "@webkit" },
    async ({ page }) => {
      const { window, code } = await openMarksReview(page, "ledger", MARKS_SUBJECT)
      // no row shows `total`: each site is found at the row of its new line, one result a site
      const { field, count } = await findOldName(page, window, code)
      const once = siteRows(ledgerOf(code), "once")
      await expect.poll(() => codeOf(once)).toEqual(NEW_LINES)
      // the hit is marked on the new name, which says what it stands for; the first one is the current hit, in view
      await expect(findMarks(once)).toHaveText(["sum", "sum", "sum"])
      await expect(findMarks(siteRow(once, 0), "current")).toHaveText("sum")
      await expect(findMarks(siteRow(once, 0), "current")).toHaveAttribute("title", "Renamed from total")
      await expect(findMarks(code, "current")).toHaveCount(1)
      await expect(siteRow(once, 0)).toBeInViewport()
      // the note counts no hit: its lines are found where they stand
      const note = remarks(code, "renamed").filter({ hasText: "total" })
      await expect(note).toHaveAttribute("aria-expanded", "false")
      await expect(remarkHits(await rowOfNote(code, "total"))).toHaveCount(0)
      // a step goes to the next site, in the order of the lines
      await page.keyboard.press("Enter")
      await expect(count).toHaveText("2 of 3")
      await expect(findMarks(siteRow(once, 1), "current")).toHaveText("sum")
      await expect(findMarks(siteRow(once, 0), "current")).toHaveCount(0)

      // the note opened: the old lines show, and each hit is in the old name of its line, never in the new one
      await note.click()
      await expect(note).toHaveAttribute("aria-expanded", "true")
      const [old, added] = [siteRows(ledgerOf(code), "deleted"), siteRows(ledgerOf(code), "added")]
      await expect.poll(() => codeOf(old)).toEqual(OLD_LINES)
      await expect(count).toHaveText(/^\d of 3$/)
      await expect(findMarks(old)).toHaveText(["total", "total", "total"])
      await expect(findMarks(added)).toHaveCount(0)
      await expect(findMarks(code, "current")).toHaveCount(1)

      // the new name is found in the lines shown once, as any text of them
      await note.click()
      await expect(note).toHaveAttribute("aria-expanded", "false")
      await field.fill("sum +=")
      await expect(count).toHaveText("1 result")
      await expect.poll(() => joinedText(findMarks(siteRow(once, 1)))).toBe("sum +=")
    },
  )
})

test.describe("The renames of a diff in a review with its plan", () => {
  test("a rename whose declaration starts the block of a change says its words in the heading of that change", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("simple")
    // the local `sum` of `total` is `all` at its two places, two lines of one block
    const total = (name: string) =>
      `class Total {\n  fun total(items: List<Int>): Int {\n    val ${name} = items.fold(0) { a, b -> a + b }\n    return ${name}\n  }\n\n  fun other() = 1\n}\n`
    await repo.commit("Add the total", { "src/Total.kt": total("sum") }, 1704120600)
    await repo.pushInTerminal("master")
    await repo.commit("(total) the total is all of the items\n\nThe sum of the items is named for what it is.\n", { "src/Total.kt": total("all") }, 1704121200)
    turnOnReviewPlan(repo)
    const window = await openPlanWindow(page, syncAndWaitForBranches, "total")
    const code = window.getByTestId("commit-review-code")
    // one row of words before the block: the heading of the change, then the words of the rename
    const notes = diffRows(code, "note")
    await expect(notes).toHaveText(["total — body changed · Renamed sum → all · 2 sites"])
    await expect(notes.getByTestId("diff-unit-heading")).toHaveText("total — body changed")
    await expect(remarks(notes, "renamed")).toHaveText("Renamed sum → all · 2 sites")
    await expect(renamedNames(code)).toHaveText(["all", "all"])
    await expect(code.locator(STRUCK_NAME)).toHaveText(["sum", "sum"])
  })
})

// src/Pairs.kt of a change built in the test. In `sums` three locals are renamed, each declared on a line of one block
// (`low`, `high`, `both`), so one note names the three; in `pair` two are (`left`, `right`), `left` on a line with
// another change, after a line that really changed: the declaration is the second line of its lines
const pairs = (names: { low: string; high: string; both: string; left: string; right: string }, scale: number) => `class Pairs(private val items: List<Int>) {
  fun sums(limit: Int): Int {
    val ${names.low} = items.filter { it < limit }
    val ${names.high} = items.filter { it >= limit }
    val ${names.both} = ${names.low} + ${names.high}
    return ${names.both}.sum()
  }

  fun pair(first: Int, second: Int): Int {
    val scale = ${scale}
    val ${names.left} = first * scale + ${scale}
    val ${names.right} = second * scale
    return ${names.left} + ${names.right}
  }
}
`

test.describe("The note of a block that declares several renamed names", () => {
  test("a note words both renames of a block that declares two, the first of three with the count of the others, and stands right above the line of its declaration", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("simple")
    await repo.commit("Add the pairs", { "src/Pairs.kt": pairs({ low: "low", high: "high", both: "both", left: "left", right: "right" }, 2) }, 1704120600)
    await repo.pushInTerminal("master")
    await repo.commit("(pairs) the pairs say what they hold", { "src/Pairs.kt": pairs({ low: "small", high: "large", both: "all", left: "a", right: "b" }, 3) }, 1704121200)
    await syncAndWaitForBranches()
    await expandBranch(page, findBranchRow(page, "pairs"))
    const preview = await openCommitPreview(page, getBranchCommitRow(page, "pairs", "the pairs say what they hold"))
    const file = await openFileOf(await openCommitFilesSection(preview), "Pairs.kt", "fun pair(first: Int, second: Int): Int {")
    const notes = remarks(file, "renamed")
    await expect(notes).toHaveCount(2)

    // three names declared in the block of `sums`: the first is worded, the two others counted; a hover lists them,
    // and a screen reader hears them after the count, in words no eye sees
    const three = notes.filter({ hasText: "low" })
    await expect(three).toHaveText("Renamed low → small · 2 sites · and 2 more: high to large, 2 sites; both to all, 2 sites")
    await expectPlainWords(three)
    const more = three.getByTestId("diff-remark-more")
    await expect(more).toHaveAttribute("title", "high → large · 2 sites\nboth → all · 2 sites")
    const said = more.getByText(": high to large, 2 sites; both to all, 2 sites")
    expect((await said.boundingBox())!.width).toBeLessThanOrEqual(1)
    // each of the three is drawn in its mark where it stands, with its own hover
    await expect(renamedNames(file).filter({ hasText: "large" }).first()).toHaveAttribute("title", "Renamed from high")

    // two names declared in the block of `pair`: both are worded, never "and 1 more"
    const two = notes.filter({ hasText: "left" })
    await expect(two).toHaveText("Renamed left → a · 2 sites · right → b · 2 sites")
    await expect(two.getByTestId("diff-remark-more")).toHaveCount(0)
    // the note is the row right above the old line of the declaration of `left`, the second line of its lines: the
    // line that really changed, both its old and its new one, stays above the note
    const declaration = diffRows(file, "deleted", { hasText: "val left = first * scale + 2" })
    expect(await rowAbove(declaration)).toEqual({ kind: "note", code: "" })
    expect(await rowAbove(await rowOfNote(file, "left"))).toEqual({ kind: "added", code: "    val scale = 3" })
    // on the lines of a run the names alone are marked: nothing between two of them, though the word diff joined them
    const joined = diffRows(file, "added", { hasText: "val all = small + large" })
    await expect(renamedNames(joined)).toHaveText(["all", "small", "large"])
    await expect(editedParts(joined)).toHaveCount(0)
    // Split: the two lines of the declaration in one row, under the note
    await preview.getByRole("button", { name: "Split", exact: true }).click()
    const beside = diffRows(file, "changed", { hasText: "val a = first * scale + 3" })
    await expect(beside).toHaveCount(1)
    await expect(two).toHaveText("Renamed left → a · 2 sites · right → b · 2 sites")
    await expect.poll(() => rowAbove(beside)).toEqual({ kind: "note", code: "" })
    expect((await rowAbove(await rowOfNote(file, "left"))).kind).toBe("changed")
  })
})

// src/Count.kt of a change built in the test: the local `total` of `count` is `sum` at three places. Its declaration
// and the call after it are one block, the call wrapped over two lines (one run of two old lines and three new ones);
// forty lines are added below, more than a view shows; the last place is the result, in a block of its own
const count = (renamed: boolean) => {
  const name = renamed ? "sum" : "total"
  const call = renamed ? "    log(extra,\n      sum)" : "    log(extra, total)"
  const checks = renamed ? Array.from({ length: 40 }, (_, at) => `    check(extra > ${at})\n`).join("") : ""
  return `class Count(private val items: List<Int>) {
  fun count(extra: Int): Int {
    val ${name} = items.size
${call}
    val first = items.first()
${checks}    val last = items.last()
    return ${name} + first + last
  }

  private fun log(first: Int, second: Int) {}
}
`
}

test.describe("A quiet rename whose places are far apart, one in lines wrapped otherwise", () => {
  /** The review of the change of src/Count.kt with the kind `renamed` switched on: its window and its diffs */
  async function openCountReview(page: Page, repo: TestRepositoryBuilder, sync: () => Promise<unknown>) {
    await repo.commit("Add the count", { "src/Count.kt": count(false) }, 1704120600)
    await repo.pushInTerminal("master")
    await repo.commit("(count) the count is a sum", { "src/Count.kt": count(true) }, 1704121200)
    await sync()
    await switchRenamedOn(page)
    return openMarksReview(page, "count", "the count is a sum")
  }

  // the key of the notes from the diffs, the focus kept across a paint: WebKit differs there
  test("H shows and hides the old line of a quiet site whose note is out of view", { tag: "@webkit" }, async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const { code } = await openCountReview(page, await setupRepo("simple"), syncAndWaitForBranches)
    const diffs = code.locator("[aria-label='Changes of the commit']")
    const note = remarks(code, "renamed")
    await expect(note).toHaveAccessibleName("Renamed total to sum, 3 sites: Show the 3 old lines")
    // the result, forty added lines below the note of its rename: shown once, and the note is out of view
    const result = diffRows(code, "once", { hasText: "return sum + first + last" })
    await scrollDiffsTo(code, result)
    await result.scrollIntoViewIfNeeded()
    await expect(result).toBeInViewport()
    await expect(note).not.toBeInViewport()
    // H: the old line of the site shows above its new one, though the note that opens it is screens above
    await diffs.focus()
    await page.keyboard.press("h")
    const old = diffRows(code, "deleted", { hasText: "return total + first + last" })
    await expect(old).toBeInViewport()
    await expect(diffRows(code, "once")).toHaveCount(0)
    expect((await rowAbove(diffRows(code, "added", { hasText: "return sum + first + last" }))).code).toBe("    return total + first + last")
    await expect(diffs).toBeFocused()
    // H again, from the lines the note showed: they go
    await page.keyboard.press("h")
    await expect(old).toHaveCount(0)
    await expect(result).toBeInViewport()
    // the note itself says so once it is in view again
    await page.getByRole("treeitem", { name: "Count.kt" }).click()
    await expect(note).toHaveAttribute("aria-expanded", "false")
  })

  // the field of the find takes the focus from the diffs and the hits are painted into rows painted anew: WebKit differs there
  test(
    "the find reads the hidden old lines of a run wrapped otherwise: the old name at the row of its new name, text only the old line holds at the code that stands for it",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const { window, code } = await openCountReview(page, await setupRepo("simple"), syncAndWaitForBranches)
      // the run of the declaration and the wrapped call: two old lines shown once by three new ones
      const once = diffRows(code, "once")
      await expect.poll(() => codeOf(once)).toEqual(["    val sum = items.size", "    log(extra,", "      sum)", "    return sum + first + last"])
      const [declared, wrapped, argument] = [once.filter({ hasText: "val sum" }), once.filter({ hasText: "log(extra," }), once.filter({ hasText: "sum)" })]
      // the old name: one result a place; the second is the argument of the call, found at the row of the new line
      // that holds its new name, the third row of the run, never at the first
      const { field, count: results } = await findInReview(window, code, "total")
      await expect(results).toHaveText("3 results")
      await page.keyboard.press("Enter")
      await expect(findMarks(declared, "current")).toHaveText("sum")
      await page.keyboard.press("Enter")
      await expect(results).toHaveText("2 of 3")
      await expect(findMarks(argument, "current")).toHaveText("sum")
      await expect(findMarks(code, "current")).toHaveCount(1)
      // text that only the old line holds in one piece (the space after the comma went with the wrap): found, at the
      // code of the new line that stands for it
      await field.fill("(extra, ")
      await expect(results).toHaveText("1 result")
      await page.keyboard.press("Enter")
      await expect(results).toHaveText("1 of 1")
      await expect.poll(() => joinedText(findMarks(wrapped, "current"))).toBe("(extra,")
      await expect(wrapped).toBeInViewport()
      // text both sides hold at the same place is found once, in the new line
      await field.fill("log(extra")
      await expect(results).toHaveText("1 result")
      await expect.poll(() => joinedText(findMarks(wrapped))).toBe("log(extra")
      // the note counts none of them: its lines are found where they stand
      const note = remarks(code, "renamed")
      await expect(remarkHits(await rowOfNote(code, "total"))).toHaveCount(0)
      // under the opened note the old line shows, and the text is in it
      await field.fill("(extra, ")
      await note.click()
      await expect(note).toHaveAttribute("aria-expanded", "true")
      await expect(results).toHaveText(/1 result|1 of 1/)
      await expect.poll(() => joinedText(findMarks(diffRows(code, "deleted", { hasText: "log(extra, total)" })))).toBe("(extra, ")
      await expect(findMarks(diffRows(code, "added"))).toHaveCount(0)
    },
  )
})

/**
 * The mark of a renamed name in the preview of the template, in the theme of the page: on the pale colour of its
 * inserted line it has a colour of its own, which is neither the colour of that line nor the full colour of an
 * inserted range, and a dotted line along its lower edge, so it does not rest on its colour; the old name that the
 * row strikes through before it has the same colour
 */
async function expectRenamedMark(page: Page, theme: "light" | "dark"): Promise<void> {
  const { ledger } = await openLedgerPreview(page)
  const name = renamedNames(siteRows(ledger)).first()
  await scrollDiffsTo(ledger, name)
  const real = diffRows(ledger, "inline", { hasText: "fun isEmpty(): Boolean" })
  await scrollDiffsTo(ledger, real)
  const colours = await ledger.evaluate(
    (element, [selector, struck]) => {
      const style = (target: Element) => getComputedStyle(target)
      const renamed = element.querySelector(selector!)!
      const edited = [...element.querySelectorAll(`[data-change]:not(${selector})`)].find(part => part.textContent === "none")!
      return {
        name: style(renamed).backgroundColor,
        struck: style(element.querySelector(struck!)!).backgroundColor,
        line: style(renamed.closest("[data-testid=diff-side]")!).backgroundColor,
        edited: style(edited).backgroundColor,
        dots: [style(renamed).backgroundImage.startsWith("repeating-linear-gradient("), style(edited).backgroundImage],
        card: style(element.querySelector('[data-testid=diff-side][data-cell="context"]')!).backgroundColor,
      }
    },
    [RENAMED_NAME, STRUCK_NAME],
  )
  // the dotted line is a layer of the background of the name, which a changed range has none of
  expect(colours.dots).toEqual([true, "none"])
  expect(new Set([colours.name, colours.line, colours.edited, colours.card]).size).toBe(4)
  expect(colours.struck).toBe(colours.name)
  // the first channel of the colour of its line: light in the light theme, dark in the dark one
  expect(Number(/\d+/.exec(colours.line)![0]) > 128 ? "light" : "dark").toBe(theme)
}

test.describe("The mark of a renamed name in the light theme", () => {
  test.use({ colorScheme: "light" })

  test("a renamed name has a colour of its own and a dotted line under it in the light theme, never the colour of a change", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    await setupRepo("commit_marks")
    await syncAndWaitForBranches()
    await expectRenamedMark(page, "light")
  })
})

test.describe("The mark of a renamed name in the dark theme", () => {
  test.use({ colorScheme: "dark" })

  test("a renamed name has a colour of its own and a dotted line under it in the dark theme, never the colour of a change", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("commit_marks")
    await syncAndWaitForBranches()
    await expectRenamedMark(page, "dark")
  })
})
