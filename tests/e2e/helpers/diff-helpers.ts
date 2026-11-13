import type { Locator, Page } from "@playwright/test"
import { expect } from "../fixtures/base-test"

/*
 * The rows of a diff (docs/contributing/e2e-tests.md, "The rows of a diff"): the painter draws them outside React,
 * without a role or a name, so a test finds them by the test ids and the data attributes the painter writes, never by a
 * `dv-` class.
 */

/** The kind of a row (`data-kind`): a line row by its kind (`once`: a line of a quiet run that Inline shows once), a fold, a note */
export type DiffRowKind = "context" | "added" | "deleted" | "changed" | "inline" | "once" | "merge" | "fold" | "note"

/** The selector of the rows of a diff, of one kind when given; for a locator and for the code a test runs in the page */
export function diffRowSelector(kind?: DiffRowKind): string {
  return kind === undefined ? "[data-testid=diff-row]" : `[data-testid=diff-row][data-kind=${kind}]`
}

/** The selector of the cell of a side of a row (0 old or left, 1 new or middle, 2 right): its line number and its code */
export function diffSideSelector(side?: number): string {
  return side === undefined ? "[data-testid=diff-side]" : `[data-testid=diff-side][data-side="${side}"]`
}

/** The selector of the code of a line: the cell that shows the text of a line, without its line number (`data-side`: the side of the line) */
export const DIFF_CODE = "[data-testid=diff-code][data-side]"

/** The rows of a diff in `scope`, of one kind when given */
export function diffRows(scope: Locator | Page, kind?: DiffRowKind, options?: { hasText?: string | RegExp }): Locator {
  return scope.locator(diffRowSelector(kind), options)
}

/** The cells of a side in `scope` (a row, a file) */
export function diffSides(scope: Locator | Page, side?: number): Locator {
  return scope.locator(diffSideSelector(side))
}

/** The line numbers a row or a file shows */
export function diffLineNumbers(scope: Locator | Page, options?: { hasText?: string | RegExp }): Locator {
  return scope.locator("[data-testid=diff-line-number]", options)
}

/**
 * The parts of the lines in `scope` that a changed range holds: inserted or modified text, and the renamed names
 * (`renamedNames`), not the deleted parts of an Inline row
 */
export function changedParts(scope: Locator | Page): Locator {
  return scope.locator("[data-change]")
}

/** The parts of the lines in `scope` that a range of the engine holds, read as a change: no renamed name */
export function editedParts(scope: Locator | Page): Locator {
  return scope.locator(`[data-change]:not(${RENAMED_NAME})`)
}

/** The selector of a part of a renamed name in a line: the name at a site of a rename, in its own mark (`data-change="3"`) */
export const RENAMED_NAME = '[data-change="3"]'

/**
 * The selector of the part a renamed name starts with (`data-name-start`): the one part of the name that says
 * "renamed" to a screen reader, where a hit of the find or a syntax run cuts the name into several
 */
export const RENAMED_NAME_START = '[data-change="3"][data-name-start]'

/**
 * The names a change renamed in the lines of `scope`, each in its mark: the new name on the new side, the old one on
 * the old side, with what it was or became as its hover ("Renamed from total"). A name is a part per token of the
 * syntax, one for a plain name
 */
export function renamedNames(scope: Locator | Page): Locator {
  return scope.locator(RENAMED_NAME)
}

/** The deleted parts of the old line that an Inline row strikes through */
export function deletedParts(scope: Locator | Page): Locator {
  return scope.locator("[data-inline-deleted]")
}

/** The parts of the lines in `scope` that a hit of the find marks: every hit, or the current one alone. A hit is a part per token of the syntax */
export function findMarks(scope: Locator | Page, which: "every" | "current" = "every"): Locator {
  return scope.locator(which === "current" ? '[data-hit="current"]' : "[data-hit]")
}

/**
 * The words of the notes that say what the marks of the diff say of the lines after them ("Formatting only · 3
 * lines"), of one kind of run when given: a button where the note opens (Inline shows its run once: `aria-expanded`
 * says whether its old lines show), plain words elsewhere. Of the note at an end of a move (`moved`) the words before
 * the place it names ("5 lines moved", "Moved"); `moveNotes` gives its whole row. Of the note of a rename (`renamed`)
 * all its words ("Renamed total → sum · 3 sites")
 */
export function remarks(scope: Locator | Page, kind?: "formatting" | "comments" | "moved" | "renamed"): Locator {
  return scope.locator(kind === undefined ? "[data-testid=diff-remark]" : `[data-testid=diff-remark][data-kind=${kind}]`)
}

/** An end of a move: `source` where its lines left, `destination` where they stand now */
export type MoveEndName = "source" | "destination"

/**
 * The rows of the notes at the ends of the moves in `scope` ("5 lines moved to line 6", "Moved from line 22 ·
 * re-indented"; of a move between two files "Moved to Report.kt:18", "Moved from Ledger.kt:55"), of one end when
 * given (`data-move`); the row a jump to an end landed on has `data-landed`
 */
export function moveNotes(scope: Locator | Page, at?: MoveEndName): Locator {
  return scope.locator(`${diffRowSelector("note")}${at === undefined ? "[data-move]" : `[data-move=${at}]`}`)
}

/**
 * The buttons of the notes of the moves in `scope` that show the other end of their move ("to line 6", "from line
 * 22", "to Report.kt:18"); in a review the one the key of the moves takes names the key (`aria-keyshortcuts`). The
 * place of an end in a file the view does not show is plain words, no button (`movePlaces`)
 */
export function moveTargets(scope: Locator | Page): Locator {
  return scope.getByTestId("diff-move-target")
}

/** The view went to `row` (a note at an end of a move, the heading that says it): it is marked as the row the jump landed on, and in view */
export async function expectLandedOn(row: Locator): Promise<void> {
  await expect(row).toHaveAttribute("data-landed", "")
  await expect(row).toBeInViewport()
}

/**
 * The view went to `row` and the mark went with it: no other row of `scope` has it. The row the jump left may be
 * outside the painted rows by then, so the mark is counted, not asked of that row
 */
export async function expectLandedAloneOn(scope: Locator, row: Locator): Promise<void> {
  await expectLandedOn(row)
  await expect(scope.locator("[data-landed]")).toHaveCount(1)
}

/** The one button of a place that names the key of the moves (`aria-keyshortcuts`): the note the key takes now */
export function keyedMoveTargets(scope: Locator | Page): Locator {
  return scope.locator("[data-testid=diff-move-target][aria-keyshortcuts]")
}

/** Back and Forward of the diffs: Chromium of the tests says it runs on Windows (Ctrl+Alt+← and →), WebKit on macOS (⌘[ and ⌘]) */
export function diffHistoryKeys(browserName: string): { back: string; forward: string } {
  return browserName === "webkit" ? { back: "Meta+BracketLeft", forward: "Meta+BracketRight" } : { back: "Control+Alt+ArrowLeft", forward: "Control+Alt+ArrowRight" }
}

/** The places the notes of the moves in `scope` name in plain words: the other end is in a file the view does not show */
export function movePlaces(scope: Locator | Page): Locator {
  return scope.getByTestId("diff-move-place")
}

/** The cells of the lines of a move (`data-cell="moved"`), of one end and of a side when given */
export function movedSides(scope: Locator | Page, at?: MoveEndName, side?: number): Locator {
  return scope.locator(`${diffSideSelector(side)}[data-cell="moved"]${at === undefined ? "" : `[data-move=${at}]`}`)
}

/**
 * The cells of the lines of a move between two files of the subject (`data-across`: the other end is in the file the
 * note names), of one end and of a side when given
 */
export function acrossSides(scope: Locator | Page, at?: MoveEndName, side?: number): Locator {
  return scope.locator(`${diffSideSelector(side)}[data-cell="moved"][data-across]${at === undefined ? "" : `[data-move=${at}]`}`)
}

/** The hits of the find in the old lines a note stands for, named after its words ("1 match in the old line"); `data-hit` says whether the current one is among them */
export function remarkHits(scope: Locator | Page): Locator {
  return scope.getByTestId("diff-remark-hits")
}

/**
 * The cells of the lines of a quiet run of the marks, which are played down (`data-tint="quiet"`), of a side when
 * given, and of the runs of one kind when given (`data-run`: a file may hold quiet lines of formatting and of a
 * renamed name)
 */
export function quietSides(scope: Locator | Page, side?: number, run?: "formatting" | "renamed"): Locator {
  return scope.locator(`${diffSideSelector(side)}[data-tint="quiet"]${run === undefined ? "" : `[data-run=${run}]`}`)
}

/**
 * Scrolls the diffs of `scope` down until `target` is in view: the painter draws only the rows in view and one viewport
 * around them, so a row far below it is not in the page until a scroll brings it near. While the target is not painted,
 * the last row painted comes into view; once it is, the target itself, never a row past it: a scroll past a target
 * painted meanwhile (a paint late under load) would leave it, and the rows a test reads next to it, at the edge of the
 * rows painted, or out of them
 */
export async function scrollDiffsTo(scope: Locator, target: Locator): Promise<void> {
  await expect(async () => {
    if ((await target.count()) === 0) {
      // the last row painted is the lowest one: the rows below it follow as it comes into view
      await diffRows(scope).last().scrollIntoViewIfNeeded()
    } else {
      await target.scrollIntoViewIfNeeded({ timeout: 500 })
    }
    await expect(target).toBeInViewport({ timeout: 500 })
  }).toPass()
}

/** The visual lines of the rows that Soft Wraps breaks */
export function visualLines(scope: Locator | Page): Locator {
  return scope.locator("[data-testid=diff-visual-line]")
}

/** The horizontal scroll bars of a file: one in Unified and Inline, one per side in Split */
export function horizontalBars(scope: Locator | Page, side?: number): Locator {
  return scope.locator(side === undefined ? "[data-testid=diff-hscroll]" : `[data-testid=diff-hscroll][data-side="${side}"]`)
}

/**
 * The text of `row` is wider than its column, and the horizontal scroll bar of the column (`bar`), moved to its end,
 * shows the end of the text: the bar reaches as far as the row is wide
 */
export async function expectBarReachesEndOf(row: Locator, bar: Locator): Promise<void> {
  /** How far the end of the text of the row is past the right edge of its column, in px */
  const cutOff = () =>
    row.evaluate(element => {
      const text = document.createRange()
      text.selectNodeContents(element.querySelector("[data-testid=diff-code][data-side]")!)
      return Math.round(text.getBoundingClientRect().right - element.getBoundingClientRect().right)
    })
  await expect.poll(cutOff).toBeGreaterThan(0)
  await expect(bar).toHaveCount(1)
  await bar.evaluate(element => {
    element.scrollLeft = element.scrollWidth
  })
  await expect.poll(cutOff).toBeLessThanOrEqual(0)
}

/** The buttons of the folds in `scope`, each "⋯ N unchanged lines" */
export function foldButtons(scope: Locator | Page): Locator {
  return scope.getByTestId("diff-fold")
}

/** The text of the elements of `parts` in their order: a hit or a deleted part of a line is an element per token of the syntax */
export async function joinedText(parts: Locator): Promise<string> {
  return (await parts.allTextContents()).join("")
}

/**
 * The copy key of the system in the page (⌘C on macOS, Ctrl+C elsewhere: the native copy of a selection, in every
 * engine), and the text the page gave the clipboard for it; null when the page gave none
 */
export async function copiedByKey(page: Page): Promise<string | null> {
  await page.evaluate(() => {
    const holder = window as unknown as { __copied?: string | null }
    holder.__copied = null
    // after the listeners of the page, which set the text
    document.addEventListener("copy", event => (holder.__copied = event.clipboardData?.getData("text/plain") ?? null), { once: true })
  })
  await page.keyboard.press("ControlOrMeta+c")
  return page.evaluate(() => (window as unknown as { __copied?: string | null }).__copied ?? null)
}

/**
 * The files of a review stand where they stay: its overview (the heading of a commit shown alone) comes above them
 * after they show, by a read of its own, and moves them down once the view has measured it. A test that reads the
 * place of a row or of a word right after the window opens waits for this first, or it points at where the row was
 */
export async function expectOverviewLaidOut(code: Locator): Promise<void> {
  const overview = code.getByTestId("diff-view-prologue")
  await expect(overview).toBeVisible()
  const file = code.getByTestId("diff-view-file").first()
  await expect
    .poll(async () => {
      const [above, below] = [await overview.boundingBox(), await file.boundingBox()]
      return above !== null && below !== null && below.y >= above.y + above.height
    })
    .toBe(true)
}
