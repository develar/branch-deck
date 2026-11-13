// @spec openspec/specs/commit/quiet-changes/spec.md
import { readFileSync, writeFileSync } from "node:fs"
import path from "node:path"
import type { Locator, Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { expandBranch, findBranchRow, getBranchCommitRow } from "../helpers/branch-helpers"
import {
  acrossSides,
  copiedByKey,
  diffHistoryKeys,
  diffRows,
  diffSides,
  expectLandedAloneOn,
  keyedMoveTargets,
  moveNotes,
  movePlaces,
  moveTargets,
  remarks,
  renamedNames,
  scrollDiffsTo,
} from "../helpers/diff-helpers"
import { answerStoreMoves, answerStoreRename, codeOf, lateSubjectMarks, MARKS_SUBJECT, MOVES_SUBJECT, openLedgerPreview, openMarksReview } from "../helpers/marks-helpers"
import { chooseWhitespace, openCommitFilesSection, openCommitPreview } from "../helpers/preview-helpers"
import { clickContextMenuItem, openContextMenu, selectedInDiffs } from "../helpers/selection-helpers"
import { nextAnswer, runAndWaitForAnswer } from "../helpers/sync-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"
import { expectClosed, holdCommand, resizeReportsDelivered } from "../helpers/wait-helpers"

// what the files of the commit of the `commit_marks` template say of each other. `reportTitle` moves from the end of
// src/Ledger.kt to the end of src/Report.kt as it is: its four lines leave the old lines 55 to 58 of the ledger for the
// new lines 18 to 21 of the report (the blank line before them is no line of the move). src/Store.kt declares
// `openStore` and renames it to `connectStore`; the report calls it at its lines 5 and 11 and declares none
const TITLE = ["fun reportTitle(ledger: Ledger, account: String): String {", "  val balance = ledger.balance(account)", '  return "Report for $account: $balance"', "}"]
const CALL = "    val store = openStoreconnectStore(path)"
const DECLARATION = "fun openStoreconnectStore(path: String): Store {"

/** The rows of the lines of a move between two files in `scope`, of one end: where they left or where they stand now */
function acrossRows(scope: Locator, at: "source" | "destination"): Locator {
  return diffRows(scope).filter({ has: acrossSides(scope.page(), at) })
}

/** The note where the lines of `reportTitle` left the ledger (`left`), and the one where they stand in the report (`stands`) */
function titleNotes(scope: Locator): { left: Locator; stands: Locator } {
  return { left: moveNotes(scope, "source").filter({ hasText: "to Report.kt:18" }), stands: moveNotes(scope, "destination").filter({ hasText: "from Ledger.kt:55" }) }
}

/** The file a note of renames names as the one that declares a name ("declared in Store.kt"): a button where the view shows that file */
function declaredIn(scope: Locator | Page): Locator {
  return scope.getByTestId("diff-rename-declared")
}

/** What the index of the repository proved of a rename, after the words of its note ("all uses in this change"); `data-proof` is its answer */
function proofs(scope: Locator | Page): Locator {
  return scope.getByTestId("diff-rename-proof")
}

/** Selects in `scope` from the start of the line whose code is `from` to the end of the line whose code is `to`, as a drag over them leaves it */
async function selectLines(scope: Locator, from: string, to: string): Promise<void> {
  await scope.evaluate(
    (element, ends) => {
      const cells = [...element.querySelectorAll("[data-testid=diff-row] [data-testid=diff-code]")]
      const [first, last] = ends.map(text => cells.find(cell => cell.textContent === text)!)
      const range = document.createRange()
      range.setStart(first!, 0)
      range.setEnd(last!, last!.childNodes.length)
      document.getSelection()!.removeAllRanges()
      document.getSelection()!.addRange(range)
    },
    [from, to],
  )
}

/** The selection of the page: whether it is a caret, and the code of the rows its two ends are in */
async function selectedEnds(page: Page): Promise<{ collapsed: boolean; anchor: string | null; focus: string | null }> {
  return page.evaluate(() => {
    const selection = document.getSelection()
    const codeAt = (node: Node | null | undefined) => {
      const element = node instanceof Element ? node : (node?.parentElement ?? null)
      return element?.closest("[data-testid=diff-row]")?.querySelector("[data-testid=diff-code]")?.textContent ?? null
    }
    return { collapsed: selection?.isCollapsed ?? true, anchor: codeAt(selection?.anchorNode), focus: codeAt(selection?.focusNode) }
  })
}

/** The row of a file in the tree of the files of a preview, which holds its diff once it is open */
function fileRow(preview: Locator, name: string): Locator {
  return preview.getByRole("tree", { name: "Changed files" }).getByRole("treeitem", { name })
}

/** Opens the diff of a file of the preview under its row, by a click on its name */
async function openFile(preview: Locator, name: string): Promise<Locator> {
  const row = fileRow(preview, name)
  await row.getByText(name).click()
  await expect(row).toHaveAttribute("aria-expanded", "true")
  return row
}

/**
 * How far the top of `row` is below the top of `view`, in whole px. The painter replaces the element of a row as its
 * tokens arrive: a row that is no element of the page at the moment it is measured is measured again
 */
async function topIn(view: Locator, row: Locator): Promise<number> {
  let top = Number.NaN
  await expect
    .poll(async () => {
      const [outer, inner] = [await view.boundingBox(), await row.boundingBox()]
      top = outer === null || inner === null ? Number.NaN : Math.round(inner.y - outer.y)
      return Number.isNaN(top)
    })
    .toBe(false)
  return top
}

/** The height of `row` in px, measured again while the painter replaces its element */
async function heightOf(row: Locator): Promise<number> {
  let height = Number.NaN
  await expect
    .poll(async () => {
      height = (await row.boundingBox())?.height ?? Number.NaN
      return Number.isNaN(height)
    })
    .toBe(false)
  return height
}

test.describe("The marks of a subject in the preview of a commit", () => {
  test.beforeEach(async ({ setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("commit_marks")
    await syncAndWaitForBranches()
  })

  test("the lines that moved to another file show at both ends in every layout, in the colour of a move, each end under a note that names the file and the line of the other", async ({
    page,
  }) => {
    const { preview, ledger } = await openLedgerPreview(page)
    const report = await openFile(preview, "Report.kt")
    const { left } = titleNotes(ledger)
    const { stands } = titleNotes(report)
    for (const [mode, kinds] of [
      ["Inline", ["deleted", "added"]],
      ["Unified", ["deleted", "added"]],
      ["Split", ["changed", "changed"]],
    ] as const) {
      await preview.getByRole("button", { name: mode, exact: true }).click()
      // where the lines left: a note of plain words that opens nothing, then every line, in Inline too
      await scrollDiffsTo(ledger, left)
      await left.scrollIntoViewIfNeeded()
      await expect(left).toHaveText("Moved to Report.kt:18")
      await expect(remarks(left, "moved")).not.toHaveAttribute("aria-expanded")
      await expect.poll(() => codeOf(acrossSides(ledger, "source"))).toEqual(TITLE)
      // rows of deleted lines of the move, each as it was: none pale, none edited on the way
      await expect(acrossRows(ledger, "source").and(diffRows(page, kinds[0]))).toHaveCount(4)
      await expect(acrossSides(ledger, "source").and(page.locator("[data-tint]"))).toHaveCount(0)
      // where they stand now: the same four lines, after their note
      await report.getByText("Report.kt").first().scrollIntoViewIfNeeded()
      await scrollDiffsTo(report, stands)
      await stands.scrollIntoViewIfNeeded()
      await expect(stands).toHaveText("Moved from Ledger.kt:55")
      await expect(remarks(stands, "moved")).not.toHaveAttribute("aria-expanded")
      await expect.poll(() => codeOf(acrossSides(report, "destination"))).toEqual(TITLE)
      await expect(acrossRows(report, "destination").and(diffRows(page, kinds[1]))).toHaveCount(4)
      if (mode === "Split") {
        // a column per side: each note over the column of its lines, the other one filled beside them
        await expect(left).toHaveAttribute("data-over", "old")
        await expect(stands).toHaveAttribute("data-over", "new")
        await expect(acrossSides(ledger, "source", 0)).toHaveCount(4)
        await expect(acrossSides(ledger, "source", 1)).toHaveCount(0)
        await expect(acrossSides(report, "destination", 1)).toHaveCount(4)
        await expect(acrossSides(report, "destination", 0)).toHaveCount(0)
        await expect(diffSides(acrossRows(ledger, "source").first(), 1)).toHaveAttribute("data-cell", "filler")
      }
    }
  })

  test("a click on the file and the line a note of a move names opens the diff of that file under its row and shows the other end there, and the note there shows the first again", async ({
    page,
  }) => {
    const { preview, ledger } = await openLedgerPreview(page)
    const report = fileRow(preview, "Report.kt")
    await expect(report).toHaveAttribute("aria-expanded", "false")
    const { left } = titleNotes(ledger)
    await scrollDiffsTo(ledger, left)
    // the place is a button, named by the file and the line it shows; no key in a preview
    const to = moveTargets(left)
    await expect(to).toHaveText("to Report.kt:18")
    await expect(to).toHaveAccessibleName("Go to line 18 of src/Report.kt, where the lines stand now")
    await expect(to).not.toHaveAttribute("aria-keyshortcuts")

    await to.click()
    // the row of the report opened its diff, which shows the note where the lines stand, marked
    await expect(report).toHaveAttribute("aria-expanded", "true")
    const { stands } = titleNotes(report)
    await expectLandedAloneOn(preview, stands)
    await expect.poll(() => codeOf(acrossSides(report, "destination"))).toEqual(TITLE)
    // and from there back to where they left, in the diff that is open already
    const from = moveTargets(stands)
    await expect(from).toHaveText("from Ledger.kt:55")
    await expect(from).toHaveAccessibleName("Go to line 55 of src/Ledger.kt, where the lines were")
    await from.click()
    await expectLandedAloneOn(preview, left)
    // once more, into the diff that stayed open: the same place asked again is shown again
    await moveTargets(left).click()
    await expectLandedAloneOn(preview, stands)

    // a place is shown once. Back at the top of the preview, the directory of the files folded and unfolded: its diffs
    // are there again, each at its top, and none goes to the place a note asked of it before
    const body = preview.getByTestId("commit-preview-body")
    /** The preview stands at its top, no row is marked as landed on, and the first rows of the ledger are on screen */
    const expectAtTop = async () => {
      // the frames after the diffs mounted: a place asked of one of them would have scrolled to it by now
      await resizeReportsDelivered(page)
      expect(await body.evaluate(element => element.scrollTop)).toBe(0)
      await expect(preview.locator("[data-landed]")).toHaveCount(0)
      await expect(diffRows(ledger).first()).toBeVisible()
    }
    await expect
      .poll(() =>
        body.evaluate(element => {
          element.scrollTop = 0
          return element.scrollTop
        }),
      )
      .toBe(0)
    const directory = fileRow(preview, "src")
    await directory.getByText("src", { exact: true }).click()
    await expect(directory).toHaveAttribute("aria-expanded", "false")
    await expect(report).toHaveCount(0)
    await directory.getByText("src", { exact: true }).click()
    await expect(report).toHaveAttribute("aria-expanded", "true")
    await expectAtTop()
    // the diffs read again under another whitespace policy: the same
    await runAndWaitForAnswer(page, "get_file_diffs", () => chooseWhitespace(page, preview, "Ignore Whitespace"))
    await expectAtTop()
    // and a place asked now is shown, as the first time
    await scrollDiffsTo(ledger, left)
    await moveTargets(left).click()
    await expectLandedAloneOn(preview, stands)

    // closed and opened again by its row, the diff of the report is a diff as any: no place is asked of it
    await report.getByText("Report.kt").first().click()
    await expect(report).toHaveAttribute("aria-expanded", "false")
    await report.getByText("Report.kt").first().click()
    await expect(report).toHaveAttribute("aria-expanded", "true")
    await expect(diffRows(report, "inline").first()).toBeVisible()
    await expect(preview.locator("[data-landed]")).toHaveCount(0)
  })

  test("the note of a name another file declares names that file, and a click on it opens the diff of that file at the declaration, the new name selected", async ({ page }) => {
    const { preview } = await openLedgerPreview(page)
    const report = await openFile(preview, "Report.kt")
    // one note for the two calls, at the first of them: the name is declared in no line of this file
    const note = diffRows(report, "note").filter({ has: remarks(page, "renamed") })
    await expect(note).toHaveText("Renamed openStore → connectStore · 2 sites · declared in Store.kt")
    await expect.poll(() => codeOf(diffRows(report, "inline"))).toEqual([CALL, CALL])
    // each call draws the name in the mark of a renamed name, never as a change
    await expect(renamedNames(report).filter({ hasText: "connectStore" })).toHaveCount(2)
    const store = fileRow(preview, "Store.kt")
    await expect(store).toHaveAttribute("aria-expanded", "false")

    const declared = declaredIn(note)
    await expect(declared).toHaveAccessibleName("Go to the declaration of connectStore in src/Store.kt")
    await declared.click()
    await expect(store).toHaveAttribute("aria-expanded", "true")
    const declaration = diffRows(store, "inline", { hasText: "connectStore(path: String): Store {" })
    await expect(declaration).toBeInViewport()
    await expect.poll(async () => (await selectedInDiffs(page)).text).toBe("connectStore")
    // the file that declares the name says so in its own note, which names no other file and counts the places of
    // both: its declaration and the two calls of the report
    await expect(diffRows(store, "note").filter({ has: remarks(page, "renamed") })).toHaveText("Renamed openStore → connectStore · 3 sites in 2 files")
    await expect.poll(() => codeOf(declaration)).toEqual([DECLARATION])
  })
})

test.describe("What the index of the repository proved of a rename, in the preview of a commit", () => {
  /** The notes of the rename in the report, which uses the name, and in the store, which declares it, once the index answered */
  async function openRenameNotes(page: Page): Promise<{ used: Locator; declared: Locator }> {
    const answered = nextAnswer(page, "get_index_marks")
    const { preview } = await openLedgerPreview(page)
    const report = await openFile(preview, "Report.kt")
    const store = await openFile(preview, "Store.kt")
    await answered
    const note = (file: Locator) => diffRows(file, "note").filter({ has: remarks(page, "renamed") })
    return { used: note(report), declared: note(store) }
  }

  test("a note says after its words that every use of the name is in the change, where the name is declared and where it is used", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("commit_marks")
    answerStoreRename(repo.path)
    await syncAndWaitForBranches()
    const { used, declared } = await openRenameNotes(page)
    await expect(used).toHaveText("Renamed openStore → connectStore · 2 sites · declared in Store.kt · all uses in this change")
    await expect(declared).toHaveText("Renamed openStore → connectStore · 3 sites in 2 files · all uses in this change")
    await expect(proofs(used)).toHaveAttribute("data-proof", "confirmed")
    await expect(proofs(declared)).toHaveAttribute("data-proof", "confirmed")
  })

  test("a note says that the name is used outside the change where the index lists a use the change left alone", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("commit_marks")
    answerStoreRename(repo.path, { outside: true })
    await syncAndWaitForBranches()
    const { used, declared } = await openRenameNotes(page)
    await expect(used).toHaveText("Renamed openStore → connectStore · 2 sites · declared in Store.kt · uses outside this change")
    await expect(declared).toHaveText("Renamed openStore → connectStore · 3 sites in 2 files · uses outside this change")
    await expect(proofs(declared)).toHaveAttribute("data-proof", "usesOutside")
  })

  test("a note says nothing of the uses of a name where no index answers: the rename is named as before it was asked", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("commit_marks")
    await syncAndWaitForBranches()
    const { used, declared } = await openRenameNotes(page)
    // the index answered that it knows nothing: the notes are what they were before it was asked
    await expect(used).toHaveText("Renamed openStore → connectStore · 2 sites · declared in Store.kt")
    await expect(declared).toHaveText("Renamed openStore → connectStore · 3 sites in 2 files")
    await expect(proofs(page)).toHaveCount(0)
  })
})

test.describe("A declaration the index of the repository says moved whole, in the preview of a commit", () => {
  // the `declaration_moves` template: `summary` leaves the lines 7 to 10 of the ledger for the lines 12 to 18 of the
  // report, its parameters one a line; `reportTitle` leaves 12 to 15 for 7 to 10 as it is
  const SUMMARY = ["fun summary(ledger: Ledger, account: String): String {", "  val sum = ledger.balance(account)", '  return "$sum"', "}"]
  const SUMMARY_WRAPPED = ["fun summary(", "  ledger: Ledger,", "  account: String,", "): String {", "  val sum = ledger.balance(account)", '  return "$sum"', "}"]

  test("a declaration of another layout is a move once the index says it moved: a click on the place its note names shows its seven lines, whose note shows the four old ones again; the row of a declaration moved as it is stays the note it was", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("declaration_moves")
    answerStoreMoves(repo.path)
    const release = await holdCommand(page, "get_index_marks")
    await syncAndWaitForBranches()
    await expandBranch(page, findBranchRow(page, "ledger"))
    const preview = await openCommitPreview(page, getBranchCommitRow(page, "ledger", MOVES_SUBJECT))
    await openCommitFilesSection(preview)
    const ledger = await openFile(preview, "Ledger.kt")
    const report = await openFile(preview, "Report.kt")
    // before the index answers: the title, the same lines at both ends, is a move, and nothing says the summary moved
    const title = moveNotes(ledger, "source").filter({ hasText: "to Report.kt:7" })
    await expect(title).toHaveText("Moved to Report.kt:7")
    const summary = moveNotes(ledger, "source").filter({ hasText: "to Report.kt:12" })
    await expect(moveNotes(ledger, "source")).toHaveCount(1)
    await expect(summary).toHaveCount(0)

    const answered = nextAnswer(page, "get_index_marks")
    release()
    await answered
    // the summary is a move by its name; the title is the one move it was
    await scrollDiffsTo(ledger, summary)
    await expect(summary).toHaveText("Moved summary to Report.kt:12")
    await expect(moveNotes(ledger, "source")).toHaveCount(2)
    await expect(title).toHaveText("Moved to Report.kt:7")
    await expect(moveTargets(summary)).toHaveAccessibleName("Go to line 12 of src/Report.kt, where summary stands now")
    await moveTargets(summary).click()
    // where it stands now: its seven lines, every one a line of the move, under the note that names the four it left
    const stands = moveNotes(report, "destination").filter({ hasText: "from Ledger.kt:7" })
    await expectLandedAloneOn(preview, stands)
    await expect(stands).toHaveText("Moved summary from Ledger.kt:7")
    await expect.poll(() => codeOf(acrossSides(report, "destination"))).toEqual([...TITLE, ...SUMMARY_WRAPPED])
    await expect.poll(() => codeOf(acrossSides(ledger, "source"))).toEqual([...SUMMARY, ...TITLE])
    await expect(moveTargets(stands)).toHaveAccessibleName("Go to line 7 of src/Ledger.kt, where summary was")
    await moveTargets(stands).click()
    await expectLandedAloneOn(preview, summary)
  })
})

test.describe("The marks of a subject in the review window", () => {
  test.beforeEach(async ({ setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("commit_marks")
    await syncAndWaitForBranches()
  })

  /** The review of the commit of the marks, the note where `reportTitle` left the ledger in view at the end of its card */
  async function openTitleReview(page: Page): Promise<{ window: Locator; code: Locator; diffs: Locator; left: Locator; stands: Locator }> {
    const { window, code } = await openMarksReview(page, "ledger", MARKS_SUBJECT)
    const { left, stands } = titleNotes(code)
    await scrollDiffsTo(code, left)
    await left.scrollIntoViewIfNeeded()
    return { window, code, diffs: code.locator("[aria-label='Changes of the commit']"), left, stands }
  }

  /** The review of the commit of the marks at Report.kt, and the note of the function it calls, which Store.kt declares */
  async function openReportNote(page: Page): Promise<{ window: Locator; code: Locator; note: Locator }> {
    const { window, code } = await openMarksReview(page, "ledger", MARKS_SUBJECT)
    await window.getByRole("treeitem", { name: "Report.kt" }).click()
    const note = diffRows(code, "note").filter({ has: declaredIn(page) })
    await expect(note).toHaveText("Renamed openStore → connectStore · 2 sites · declared in Store.kt")
    return { window, code, note }
  }

  // the focus goes to the diffs as the button pressed scrolls out of the painted rows, and the keys of the view work on: WebKit differs there
  test(
    "a click on the place a note names jumps to the other end in the other file, into a card that is collapsed too; Back returns and Forward goes there again",
    { tag: "@webkit" },
    async ({ page, browserName }) => {
      const { code, diffs, left, stands } = await openTitleReview(page)
      const { back, forward } = diffHistoryKeys(browserName)
      // the card of the report is collapsed: none of its rows is there
      await code.getByRole("button", { name: "Collapse Report.kt" }).click()
      await expect(stands).toHaveCount(0)
      await left.scrollIntoViewIfNeeded()

      await moveTargets(left).click()
      // the card opened, and the note where the lines stand is marked; it names the key of the moves now
      await expectLandedAloneOn(code, stands)
      await expect(code.getByRole("button", { name: "Collapse Report.kt" })).toBeVisible()
      await expect(keyedMoveTargets(code)).toHaveText(["from Ledger.kt:55"])
      await expect.poll(() => codeOf(acrossSides(code, "destination"))).toEqual(TITLE)
      // the diffs have the focus: their keys work at the place shown
      await expect(diffs).toBeFocused()
      await page.keyboard.press(back)
      await expectLandedAloneOn(code, left)
      await page.keyboard.press(forward)
      await expectLandedAloneOn(code, stands)
      // the jump back by the note there is a jump of its own: Back returns to where the lines stand
      await moveTargets(stands).click()
      await expectLandedAloneOn(code, left)
      await expect(diffs).toBeFocused()
      await page.keyboard.press(back)
      await expectLandedAloneOn(code, stands)
    },
  )

  // the key from the diffs after a jump, the focus it keeps there: WebKit differs there
  test("G jumps from the note of a move between two files to its other end in the other file and back", { tag: "@webkit" }, async ({ page, browserName }) => {
    const { window, code, diffs, left, stands } = await openTitleReview(page)
    // the report at the top: the note where the lines stand is the one note of a move in view, and names the key
    await window.getByRole("treeitem", { name: "Report.kt" }).click()
    await expect(stands).toBeInViewport()
    await expect(keyedMoveTargets(code)).toHaveText(["from Ledger.kt:55"])
    await diffs.focus()

    await page.keyboard.press("g")
    await expectLandedAloneOn(code, left)
    await expect(keyedMoveTargets(code)).toHaveText(["to Report.kt:18"])
    await expect(diffs).toBeFocused()
    // from the note the jump landed on: back to where the lines stand
    await page.keyboard.press("g")
    await expectLandedAloneOn(code, stands)
    // each press is a jump: Back returns along them
    await page.keyboard.press(diffHistoryKeys(browserName).back)
    await expectLandedAloneOn(code, left)
  })

  test("the menu of the code on a line that moved between two files shows its other end in the other file", async ({ page }) => {
    const { window, code, left, stands } = await openTitleReview(page)
    const item = page.getByRole("menuitem", { name: "Go to Other End of Move" })
    await window.getByRole("treeitem", { name: "Report.kt" }).click()
    await expect(stands).toBeInViewport()

    // on a line where the lines stand now: the item, with the key of the review, which takes the note of this move
    await diffRows(code, "added", { hasText: "val balance = ledger.balance(account)" }).click({ button: "right" })
    await expect(item).toBeVisible()
    await expect(item.locator("[data-slot=kbd]")).toHaveText("G")
    await item.click()
    await expectLandedAloneOn(code, left)
    // and on the note where they left
    await left.click({ button: "right", position: { x: 40, y: 8 } })
    await item.click()
    await expectLandedAloneOn(code, stands)
  })

  // the name a jump selects in another card, and Back to the name it left: the selection of the view differs in WebKit
  test(
    "a click on the file that declares a name shows the declaration there with the new name selected, and Back returns to the use",
    { tag: "@webkit" },
    async ({ page, browserName }) => {
      const { code, note } = await openReportNote(page)
      const call = diffRows(code, "inline", { hasText: "connectStore(path)" }).first()
      const declaration = diffRows(code, "inline", { hasText: "connectStore(path: String): Store {" })

      await declaredIn(note).click()
      await expect(declaration).toBeInViewport()
      await expect.poll(() => selectedInDiffs(page)).toEqual({ text: "connectStore", path: "src/Store.kt" })
      await expect(code.locator("[aria-label='Changes of the commit']")).toBeFocused()
      // Back: the first call of the name in the report, where the note stands, its name selected
      await page.keyboard.press(diffHistoryKeys(browserName).back)
      await expect(call).toBeInViewport()
      await expect.poll(() => selectedInDiffs(page)).toEqual({ text: "connectStore", path: "src/Report.kt" })
      await page.keyboard.press(diffHistoryKeys(browserName).forward)
      await expect.poll(() => selectedInDiffs(page)).toEqual({ text: "connectStore", path: "src/Store.kt" })
    },
  )

  test("Go to Declaration of the menu of the code on the note of a name another file declares shows the declaration the note names, and Back returns to the use", async ({
    page,
    browserName,
  }) => {
    const { window, code, note } = await openReportNote(page)
    const item = page.getByRole("menuitem", { name: "Go to Declaration" })

    // on the words of the note, a row of no name: the item goes where the note says, and there are no usages to find
    await note.click({ button: "right", position: { x: 40, y: 8 } })
    await expect(item).toBeEnabled()
    await expect(page.getByRole("menuitem", { name: "Find Usages" })).toBeDisabled()
    await item.click()
    await expect(diffRows(code, "inline", { hasText: "connectStore(path: String): Store {" })).toBeInViewport()
    await expect.poll(() => selectedInDiffs(page)).toEqual({ text: "connectStore", path: "src/Store.kt" })
    await page.keyboard.press(diffHistoryKeys(browserName).back)
    await expect.poll(() => selectedInDiffs(page)).toEqual({ text: "connectStore", path: "src/Report.kt" })

    // the note of a name the file declares itself names no file: the item has no name to go to there
    await window.getByRole("treeitem", { name: "Store.kt" }).click()
    const own = diffRows(code, "note").filter({ hasText: "Renamed openStore → connectStore · 3 sites in 2 files" })
    await own.click({ button: "right", position: { x: 40, y: 8 } })
    await expect(item).toBeDisabled()
    await page.keyboard.press("Escape")
    await expectClosed(page.getByRole("menu"))
  })
})

/** Six files more of a commit, each a line that changes: with two files of its own the commit has eight, so its review has the search of its files */
function fillerFiles(value: (at: number) => number): Record<string, string> {
  return Object.fromEntries(Array.from({ length: 6 }, (_, at) => [`src/Filler${at}.kt`, `package demo\n\nval filler${at} = ${value(at)}\n`]))
}

/**
 * A commit of the branch `accounts` that moves `describeAccount` from the end of src/Accounts.kt (its four lines leave
 * the old lines 7 to 10) to the end of src/Names.kt (the new lines 5 to 8), and changes six files more: eight files,
 * so the review has the search of its files
 */
async function commitMovedDescription(repo: TestRepositoryBuilder): Promise<void> {
  const describe = 'fun describeAccount(account: Account): String {\n  val name = account.name.trim()\n  return "Account $name of ${account.owner}"\n}\n'
  const accounts = "package demo\n\nclass Accounts {\n  val all = mutableListOf<Account>()\n}\n"
  const names = 'package demo\n\nval separator = ", "\n'
  await repo.commit("Add the accounts", { "src/Accounts.kt": `${accounts}\n${describe}`, "src/Names.kt": names, ...fillerFiles(at => at) }, 1704120600)
  await repo.pushInTerminal("master")
  await repo.commit(
    "(accounts) the description moves to the names",
    { "src/Accounts.kt": accounts, "src/Names.kt": `${names}\n${describe}`, ...fillerFiles(at => at + 100) },
    1704121200,
  )
}

/**
 * A commit of the branch `loader` in which src/Accounts.kt renames its function `openAccounts` to `loadAccounts`, and
 * src/Names.kt, which calls it at its line 4, renames a local on the same line and on the next one; six files more
 * change: eight files, so the review has the search of its files
 */
async function commitRenamedLoader(repo: TestRepositoryBuilder): Promise<void> {
  const accounts = (name: string) => `package demo\n\nclass Accounts(val path: String)\n\nfun ${name}(path: String): Accounts {\n  return Accounts(path)\n}\n`
  const names = (local: string, name: string) => `package demo\n\nfun describeAll(path: String): String {\n  val ${local} = ${name}(path)\n  return ${local}.path\n}\n`
  await repo.commit("Add the loader", { "src/Accounts.kt": accounts("openAccounts"), "src/Names.kt": names("list", "openAccounts"), ...fillerFiles(at => at) }, 1704120600)
  await repo.pushInTerminal("master")
  await repo.commit(
    "(loader) the loader says what it does",
    { "src/Accounts.kt": accounts("loadAccounts"), "src/Names.kt": names("found", "loadAccounts"), ...fillerFiles(at => at + 100) },
    1704121200,
  )
}

test.describe("The marks of a subject in a review that does not show every file", () => {
  test("a note names the other end of a move in plain words while the review does not show its file: no button, no key, no item of the menu; shown again, it is a button", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("simple")
    await commitMovedDescription(repo)
    await syncAndWaitForBranches()
    const { window, code } = await openMarksReview(page, "accounts", "the description moves to the names")
    const search = window.getByRole("textbox", { name: "Filter files" })
    const files = window.getByRole("tree", { name: "Changed files" }).locator("[role=treeitem]:not([aria-expanded])")
    await expect(files).toHaveCount(8)
    const left = moveNotes(code, "source").filter({ hasText: "to Names.kt:5" })
    const stands = moveNotes(code, "destination").filter({ hasText: "from Accounts.kt:7" })
    await expect(left).toHaveText("Moved to Names.kt:5")
    await expect(moveTargets(left)).toHaveText("to Names.kt:5")
    await expect(moveTargets(left)).toHaveAttribute("aria-keyshortcuts", "G")

    // the search hides the file the lines stand in: the same words, which show nothing
    await search.fill("Accounts")
    await expect(files).toHaveCount(1)
    await expect(left).toHaveText("Moved to Names.kt:5")
    await expect(movePlaces(left)).toHaveText("to Names.kt:5")
    await expect(movePlaces(left)).toHaveAttribute("title", "Line 5 of src/Names.kt, where the lines stand now")
    await expect(moveTargets(code)).toHaveCount(0)
    await expect(acrossSides(code, "source")).toHaveCount(4)
    // the key of the moves takes no note, and the menu of the code has no item for the move
    const diffs = code.locator("[aria-label='Changes of the commit']")
    await diffs.focus()
    await page.keyboard.press("g")
    await diffRows(code, "deleted", { hasText: "val name = account.name.trim()" }).click({ button: "right" })
    await expect(page.getByRole("menuitem", { name: "Go to Declaration" })).toBeVisible()
    await expect(page.getByRole("menuitem", { name: "Go to Other End of Move" })).toHaveCount(0)
    await page.keyboard.press("Escape")
    await expectClosed(page.getByRole("menu"))
    await expect(code.locator("[data-landed]")).toHaveCount(0)
    await expect(left).toBeInViewport()

    // every file again: the place is the button it was, and shows the other end in its file
    await search.fill("")
    await expect(files).toHaveCount(8)
    await expect(movePlaces(code)).toHaveCount(0)
    await moveTargets(left).click()
    await expectLandedAloneOn(code, stands)
    await expect
      .poll(() => codeOf(acrossSides(code, "destination")))
      .toEqual(["fun describeAccount(account: Account): String {", "  val name = account.name.trim()", '  return "Account $name of ${account.owner}"', "}"])
  })
})

test.describe("The file that declares a name in a review that does not show every file", () => {
  test("a note names the file that declares a name in plain words while the review does not show that file: no button, nothing to go to; shown again, it is a button, in a note of two renames beside the name it speaks of", async ({
    page,
    browserName,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("simple")
    await commitRenamedLoader(repo)
    await syncAndWaitForBranches()
    const { window, code } = await openMarksReview(page, "loader", "the loader says what it does")
    const search = window.getByRole("textbox", { name: "Filter files" })
    const files = window.getByRole("tree", { name: "Changed files" }).locator("[role=treeitem]:not([aria-expanded])")
    await expect(files).toHaveCount(8)
    // one note for the two renames of the line: the local and the function the other file declares, which the words
    // after them name, so a reader knows which of the two is declared there
    const note = diffRows(code, "note").filter({ has: declaredIn(page) })
    await expect(note).toHaveText("Renamed list → found · 2 sites · openAccounts → loadAccounts · 1 site · loadAccounts declared in Accounts.kt")
    await expect(declaredIn(note)).toHaveRole("button")
    // the file that declares it counts the places of both files
    await expect(diffRows(code, "note").filter({ hasText: "2 sites in 2 files" })).toHaveText("Renamed openAccounts → loadAccounts · 2 sites in 2 files")

    // the search hides the file that declares the name: the same words, which show nothing
    await search.fill("Names")
    await expect(files).toHaveCount(1)
    await expect(note).toHaveText("Renamed list → found · 2 sites · openAccounts → loadAccounts · 1 site · loadAccounts declared in Accounts.kt")
    await expect(declaredIn(note)).toHaveText("declared in Accounts.kt")
    await expect(declaredIn(note)).toHaveAttribute("title", "loadAccounts is declared in src/Accounts.kt")
    await expect(declaredIn(note)).not.toHaveRole("button")
    await expect(code.getByRole("button", { name: /declaration of loadAccounts/ })).toHaveCount(0)
    // a click on the words goes nowhere, and the menu of the code on the note has no declaration to go to
    await declaredIn(note).click()
    await note.click({ button: "right", position: { x: 40, y: 8 } })
    await expect(page.getByRole("menuitem", { name: "Go to Declaration" })).toBeDisabled()
    await page.keyboard.press("Escape")
    await expectClosed(page.getByRole("menu"))
    await expect(note).toBeInViewport()
    expect((await selectedInDiffs(page)).text).toBe("")

    // every file again: the words are the button they were, and show the declaration, the new name selected
    await search.fill("")
    await expect(files).toHaveCount(8)
    await expect(declaredIn(note)).toHaveAccessibleName("Go to the declaration of loadAccounts in src/Accounts.kt")
    await declaredIn(note).click()
    await expect.poll(() => selectedInDiffs(page)).toEqual({ text: "loadAccounts", path: "src/Accounts.kt" })
    await page.keyboard.press(diffHistoryKeys(browserName).back)
    await expect.poll(() => selectedInDiffs(page)).toEqual({ text: "loadAccounts", path: "src/Names.kt" })
  })
})

test.describe("The diff of a file of a preview that is still read", () => {
  /** A commit of the branch `tall` with a file of 120 lines before forty small ones: below the diff of the first the preview is taller than its view */
  async function commitTallFile(repo: TestRepositoryBuilder): Promise<void> {
    const small = Object.fromEntries(Array.from({ length: 40 }, (_, at) => [`src/Small${String(at).padStart(2, "0")}.kt`, `package demo\n\nval small${at} = ${at}\n`]))
    const tall = `package demo\n\n${Array.from({ length: 120 }, (_, at) => `val line${at} = ${at}`).join("\n")}\n`
    await repo.commit("(tall) a tall file and forty small ones", { "src/Ahead.kt": tall, ...small }, 1704121200)
  }

  // the scroller of the preview as the rows of a diff above the view come: WebKit keeps nothing by itself
  test(
    "a file opened while the diffs of the commit are read says so in its own view, and its rows come above the row a reader has at the top of the preview, which stays where it is",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const repo = await setupRepo("simple")
      await commitTallFile(repo)
      await syncAndWaitForBranches()
      const release = await holdCommand(page, "get_file_diffs")
      await expandBranch(page, findBranchRow(page, "tall"))
      const preview = await openCommitPreview(page, getBranchCommitRow(page, "tall", "a tall file and forty small ones"))
      await openCommitFilesSection(preview)
      const ahead = await openFile(preview, "Ahead.kt")
      // the view of the file is there, with the one row that says its diff is read
      const notice = ahead.getByTestId("diff-notice")
      await expect(notice).toHaveText("Loading…")
      await expect(ahead.getByRole("region", { name: "Changes of src/Ahead.kt" })).toHaveAttribute("aria-busy", "true")
      // the reader goes on down the files: the file that is read lies above the view
      const body = preview.getByTestId("commit-preview-body")
      const row = fileRow(preview, "Small20.kt")
      await expect(async () => {
        await row.evaluate(element => element.scrollIntoView({ block: "start" }))
        expect(Math.abs(await topIn(body, row))).toBeLessThan(2)
      }).toPass()
      await resizeReportsDelivered(page)
      const [noticeBox, bodyBox] = [await notice.boundingBox(), await body.boundingBox()]
      expect(noticeBox!.y + noticeBox!.height).toBeLessThanOrEqual(bodyBox!.y)
      const before = await topIn(body, row)
      const scrolled = await body.evaluate(element => element.scrollTop)
      const rowHeight = noticeBox!.height

      const answered = nextAnswer(page, "get_file_diffs")
      release()
      await answered
      // the 122 rows of the file came in place of the one that said it is read: the preview moved by as much, the row
      // not at all (WebKit keeps a whole scroll offset: the row of the tree may stand at half a pixel)
      await expect.poll(async () => Math.abs((await body.evaluate(element => element.scrollTop)) - (scrolled + 121 * rowHeight))).toBeLessThan(1)
      expect(Math.abs((await topIn(body, row)) - before)).toBeLessThanOrEqual(1)
      await ahead.getByText("Ahead.kt").first().scrollIntoViewIfNeeded()
      await expect(diffRows(ahead, "added", { hasText: "val line0 = 0" })).toBeVisible()
      await expect(notice).toHaveCount(0)
    },
  )

  test("the scroller around the diffs of a preview is kept by its views while it holds one, and is the engine's to keep again once its last diff is closed", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    await setupRepo("commit_marks")
    await syncAndWaitForBranches()
    const { preview, ledger } = await openLedgerPreview(page)
    const body = preview.getByTestId("commit-preview-body")
    /** Whether the engine keeps what is on screen in the preview in place by itself (its scroll anchoring) */
    const anchors = () => body.evaluate(element => getComputedStyle(element).overflowAnchor)
    // each view moves the preview by the rows that come above the reader: the engine must not move it a second time
    await expect.poll(anchors).toBe("none")
    const report = await openFile(preview, "Report.kt")
    await ledger.getByText("Ledger.kt").first().click()
    await expect(ledger).toHaveAttribute("aria-expanded", "false")
    await expect.poll(anchors).toBe("none")
    // no diff left: what changes above the reader in the preview is the engine's to hold again
    await report.getByText("Report.kt").first().click()
    await expect(report).toHaveAttribute("aria-expanded", "false")
    await expect.poll(anchors).toBe("auto")
    await report.getByText("Report.kt").first().click()
    await expect(report).toHaveAttribute("aria-expanded", "true")
    await expect.poll(anchors).toBe("none")
  })
})

test.describe("The marks of a subject in the uncommitted changes", () => {
  test("a click on the place a note of a move names in another file of the list opens that file and shows the other end in its diff, and the note there shows the first again", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("commit_marks")
    // in the working tree, the function the store declares moves to the end of the report: its three lines leave the
    // lines 19 to 21 of the store for the lines 23 to 25 of the report
    const moved = "fun connectStore(path: String): Store {\n  return Store(path)\n}\n"
    const file = (name: string) => path.join(repo.path, "src", name)
    writeFileSync(file("Store.kt"), readFileSync(file("Store.kt"), "utf8").replace(`\n${moved}`, ""))
    writeFileSync(file("Report.kt"), `${readFileSync(file("Report.kt"), "utf8")}\n${moved}`)
    await syncAndWaitForBranches()
    await openContextMenu(page, findBranchRow(page, "ledger"))
    await clickContextMenuItem(page, "Amend Changes…")
    const card = page.getByTestId("uncommitted-changes-card")
    const item = (name: string) => card.getByTestId("file-diff").filter({ has: page.getByRole("button", { name: `src/${name}`, exact: true }) })
    const header = (name: string) => card.getByRole("button", { name: `src/${name}`, exact: true })
    const [store, report] = [item("Store.kt"), item("Report.kt")]
    await expect(header("Report.kt")).toHaveAttribute("aria-expanded", "false")

    await header("Store.kt").click()
    const left = moveNotes(store, "source")
    await expect(left).toHaveText("Moved to Report.kt:23")
    await expect.poll(() => codeOf(acrossSides(store, "source"))).toEqual(moved.trimEnd().split("\n"))
    await moveTargets(left).click()
    // the item of the report opened, and its diff shows the note where the lines stand, marked
    await expect(header("Report.kt")).toHaveAttribute("aria-expanded", "true")
    const stands = moveNotes(report, "destination")
    await expect(stands).toHaveText("Moved from Store.kt:19")
    await expectLandedAloneOn(card, stands)
    await expect.poll(() => codeOf(acrossSides(report, "destination"))).toEqual(moved.trimEnd().split("\n"))
    // and back, into the diff that is open
    await moveTargets(stands).click()
    await expectLandedAloneOn(card, left)
  })
})

test.describe("The marks of a subject that come after the rows of a file were shown", () => {
  /** The row of the second line of `reportTitle` where it stands in the report: below the two notes the marks of the subject add to that file */
  const balanceRow = (scope: Locator) => diffRows(scope, "added", { hasText: "val balance = ledger.balance(account)" })

  /**
   * Puts `row` at the top of `view`, the scroller it is in (a pixel below it at most), and waits until the view heard
   * of the scroll. The painter replaces the element of a row as its tokens arrive, so a
   * scroll asked of an element that just left the page moves nothing: asked again until the row stands there
   */
  async function standAt(view: Locator, row: Locator): Promise<void> {
    await expect(async () => {
      await row.evaluate(element => element.scrollIntoView({ block: "start" }))
      expect(Math.abs(await topIn(view, row))).toBeLessThan(2)
    }).toPass()
    // the scroll event of the frame is delivered: the views read the row at their top by it
    await resizeReportsDelivered(view.page())
  }

  /**
   * `row` stays where it is in `view`, the scroller it is in, while `change` adds two rows of words above the row at
   * the top of the view: the view scrolled by the height of the two rows, the row not at all
   */
  async function expectStaysThrough(view: Locator, row: Locator, change: () => Promise<void>): Promise<void> {
    const before = await topIn(view, row)
    const scrolled = await view.evaluate(element => element.scrollTop)
    const rowHeight = await heightOf(row)
    await change()
    await expect.poll(() => view.evaluate(element => element.scrollTop)).toBe(scrolled + 2 * rowHeight)
    expect(await topIn(view, row)).toBe(before)
  }

  /**
   * The review of the commit of the marks, at its Report.kt, while the marks of the subject of that file are late: the
   * report says nothing of its subject until the index answered, as where its pair had to be compared, and the answer
   * is held (with the kind `renamed` switched on, the answer makes the two calls quiet). `answer` lets the answer of
   * the index arrive, with the marks of the report
   */
  async function openLateReview(
    page: Page,
    repositoryPath: string,
    syncAndWaitForBranches: () => Promise<unknown>,
  ): Promise<{ window: Locator; code: Locator; report: Locator; answer: () => Promise<void> }> {
    answerStoreRename(repositoryPath)
    await lateSubjectMarks(page, "src/Report.kt")
    const release = await holdCommand(page, "get_index_marks")
    await syncAndWaitForBranches()
    const { window, code } = await openMarksReview(page, "ledger", MARKS_SUBJECT)
    const report = code.getByRole("region", { name: "src/Report.kt" })
    await window.getByRole("treeitem", { name: "Report.kt" }).click()
    await expect(diffRows(report, "note")).toHaveCount(0)
    const answer = async () => {
      const answered = nextAnswer(page, "get_index_marks")
      release()
      await answered
    }
    return { window, code, report, answer }
  }

  // the scroll of the view at the moment its rows are laid out again: the engines differ in what they keep by themselves
  test(
    "the notes they add come above the row a reader has at the top of a review, which stays where it is, and the uses the index proved keep their rows, played down",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const repo = await setupRepo("commit_marks")
      const { window, code, report, answer } = await openLateReview(page, repo.path, syncAndWaitForBranches)
      const view = code.getByTestId("diff-view")
      // as the content of the file gave it: the calls and the added lines are changes as any, under no note
      await expect.poll(() => codeOf(diffRows(report, "inline"))).toEqual([CALL, CALL])
      await expect(acrossSides(report)).toHaveCount(0)
      await expect(report.getByTestId("review-file-to-read")).toHaveCount(0)
      const row = balanceRow(report)
      await standAt(view, row)
      // the two notes come, both above the row at the top: the view moves by their two rows, the row not at all
      await expectStaysThrough(view, row, answer)
      await expect(acrossSides(row, "destination")).toHaveCount(1)
      // the calls the index proved are passed now: one change of three is left to read
      await expect(report.getByTestId("review-file-to-read")).toHaveText("1 to read")
      // back at the top of the file: both notes, and every row of a line that was there, the calls with both names
      await window.getByRole("treeitem", { name: "Report.kt" }).click()
      await expect(diffRows(report, "note")).toHaveText(["Renamed openStore → connectStore · 2 sites · declared in Store.kt · all uses in this change", "Moved from Ledger.kt:55"])
      const calls = diffRows(report, "inline")
      await expect.poll(() => codeOf(calls)).toEqual([CALL, CALL])
      await expect.poll(() => diffSides(calls).evaluateAll(sides => sides.map(side => side.getAttribute("data-tint")))).toEqual(["quiet", "quiet"])
      await expect(diffRows(report, "once")).toHaveCount(0)
      await expect.poll(() => codeOf(acrossSides(report, "destination"))).toEqual(TITLE)
    },
  )

  // the selection of the page over rows that are painted again: the engines differ in what is left of it
  test(
    "a selection a reader made in a file stays on its lines through the notes that come above them, and a copy takes those lines before and after",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const repo = await setupRepo("commit_marks")
      const { report, answer } = await openLateReview(page, repo.path, syncAndWaitForBranches)
      // from the start of the second line of `reportTitle` to the end of its third, as a drag over the two lines leaves it
      const first = balanceRow(report)
      const last = diffRows(report, "added", { hasText: TITLE[2] })
      await expect.poll(() => codeOf(first)).toEqual([TITLE[1]])
      await expect.poll(() => codeOf(last)).toEqual([TITLE[2]])
      await selectLines(report, TITLE[1]!, TITLE[2]!)
      const selected = TITLE.slice(1, 3).join("\n")
      expect(await copiedByKey(page)).toBe(selected)
      // the answer of the index comes with the marks of the report: two notes above the selected lines, every row
      // of the file painted again
      await answer()
      await expect(diffRows(report, "note")).toHaveCount(2)
      // the selection is on the same two lines, in the rows painted for them now: its ends are in the file, it is no
      // caret, and the copy is what it was
      await expect.poll(() => selectedEnds(page)).toEqual({ collapsed: false, anchor: TITLE[1], focus: TITLE[2] })
      expect(await copiedByKey(page)).toBe(selected)
    },
  )

  // a selection from one file to another: what an engine leaves of it as the rows go is a range between two hosts
  test(
    "a selection from a line of a file to a line of the next file stays on both lines through the notes that come between them",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const repo = await setupRepo("commit_marks")
      const { code, report, answer } = await openLateReview(page, repo.path, syncAndWaitForBranches)
      const store = code.getByRole("region", { name: "src/Store.kt" })
      await expect.poll(() => codeOf(balanceRow(report))).toEqual([TITLE[1]])
      await expect.poll(() => codeOf(diffRows(store, "inline"))).toEqual([DECLARATION])
      // from the start of the second line of `reportTitle` in the report to the end of the declaration in the store
      await selectLines(code, TITLE[1]!, DECLARATION)
      const ends = { collapsed: false, anchor: TITLE[1], focus: DECLARATION }
      expect(await selectedEnds(page)).toEqual(ends)
      const copied = await copiedByKey(page)
      expect(copied?.split("\n").slice(0, 3)).toEqual(TITLE.slice(1))
      expect(copied?.split("\n").at(-1)).toBe("fun connectStore(path: String): Store {")
      await answer()
      await expect(diffRows(report, "note")).toHaveCount(2)
      await expect.poll(() => selectedEnds(page)).toEqual(ends)
      expect(await copiedByKey(page)).toBe(copied)
    },
  )

  /**
   * The preview of the commit of the marks while the marks of the subject of the report are late: the diffs of the
   * report and of the store open under the one of the ledger, every line of the store shown, so the preview is tall
   * enough below the report for the report to lie above the view, and for a row of it to stand at the top. `answer`
   * lets the answer of the index arrive, with the marks of the report
   */
  async function openLatePreview(
    page: Page,
    repositoryPath: string,
    syncAndWaitForBranches: () => Promise<unknown>,
  ): Promise<{ body: Locator; report: Locator; store: Locator; answer: () => Promise<void> }> {
    answerStoreRename(repositoryPath)
    await lateSubjectMarks(page, "src/Report.kt")
    const release = await holdCommand(page, "get_index_marks")
    await syncAndWaitForBranches()
    const { preview } = await openLedgerPreview(page)
    const report = await openFile(preview, "Report.kt")
    await expect(diffRows(report, "note")).toHaveCount(0)
    const store = await openFile(preview, "Store.kt")
    await store.getByTestId("diff-fold").click({ modifiers: ["Alt"] })
    const answer = async () => {
      const answered = nextAnswer(page, "get_index_marks")
      release()
      await answered
    }
    return { body: preview.getByTestId("commit-preview-body"), report, store, answer }
  }

  // the scroll of the scroller around the diffs of a preview, which each diff moves by its own part: WebKit keeps nothing by itself
  test(
    "the notes they add to a diff of a preview that lies above the one a reader has on screen leave the row at the top of the view where it is",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const repo = await setupRepo("commit_marks")
      const { body, report, store, answer } = await openLatePreview(page, repo.path, syncAndWaitForBranches)
      // at the end of the preview the report lies above the view, whole
      const row = diffRows(store, "context", { hasText: "private val lines = mutableListOf<String>()" })
      await expect(row).toBeVisible()
      await expect
        .poll(() =>
          body.evaluate(element => {
            element.scrollTop = element.scrollHeight
            return element.scrollTop + element.clientHeight >= element.scrollHeight - 1
          }),
        )
        .toBe(true)
      await resizeReportsDelivered(page)
      const [reportBox, bodyBox] = [await report.boundingBox(), await body.boundingBox()]
      expect(reportBox!.y + reportBox!.height).toBeLessThanOrEqual(bodyBox!.y)
      // the report grows by its two notes above the view: the preview scrolls by as much, and the row stays
      await expectStaysThrough(body, row, answer)
      await expect(proofs(store)).toHaveText("all uses in this change")
      await report.getByText("Report.kt").first().scrollIntoViewIfNeeded()
      await expect(diffRows(report, "note")).toHaveText(["Renamed openStore → connectStore · 2 sites · declared in Store.kt · all uses in this change", "Moved from Ledger.kt:55"])
    },
  )

  test(
    "the notes they add to the diff a reader has on screen in a preview come above its top row, which stays where it is",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const repo = await setupRepo("commit_marks")
      const { body, report, answer } = await openLatePreview(page, repo.path, syncAndWaitForBranches)
      const row = balanceRow(report)
      await standAt(body, row)
      await expectStaysThrough(body, row, answer)
      await expect(acrossSides(row, "destination")).toHaveCount(1)
    },
  )
})
