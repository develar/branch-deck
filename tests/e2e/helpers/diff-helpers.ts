import type { Locator, Page } from "@playwright/test"

/*
 * The rows of a diff (docs/contributing/e2e-tests.md, "The rows of a diff"): the painter draws them outside React,
 * without a role or a name, so a test finds them by the test ids and the data attributes the painter writes, never by a
 * `dv-` class.
 */

/** The kind of a row (`data-kind`): a line row by its kind, a fold, a note */
export type DiffRowKind = "context" | "added" | "deleted" | "changed" | "inline" | "merge" | "fold" | "note"

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

/** The parts of the lines in `scope` that a changed range of the engine holds: inserted or modified text, not the deleted parts of an Inline row */
export function changedParts(scope: Locator | Page): Locator {
  return scope.locator("[data-change]")
}

/** The deleted parts of the old line that an Inline row strikes through */
export function deletedParts(scope: Locator | Page): Locator {
  return scope.locator("[data-inline-deleted]")
}

/** The parts of the lines in `scope` that a hit of the find marks: every hit, or the current one alone. A hit is a part per token of the syntax */
export function findMarks(scope: Locator | Page, which: "every" | "current" = "every"): Locator {
  return scope.locator(which === "current" ? '[data-hit="current"]' : "[data-hit]")
}

/** The visual lines of the rows that Soft Wraps breaks */
export function visualLines(scope: Locator | Page): Locator {
  return scope.locator("[data-testid=diff-visual-line]")
}

/** The horizontal scroll bars of a file: one in Unified and Inline, one per side in Split */
export function horizontalBars(scope: Locator | Page, side?: number): Locator {
  return scope.locator(side === undefined ? "[data-testid=diff-hscroll]" : `[data-testid=diff-hscroll][data-side="${side}"]`)
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
