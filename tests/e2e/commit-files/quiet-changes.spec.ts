// @spec openspec/specs/commit/quiet-changes/spec.md
import type { Locator, Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { expandBranch, findBranchRow, getBranchCommitRow } from "../helpers/branch-helpers"
import {
  changedParts,
  copiedByKey,
  deletedParts,
  diffRows,
  diffSides,
  expectOverviewLaidOut,
  findMarks,
  joinedText,
  quietSides,
  remarkHits,
  remarks,
  scrollDiffsTo,
} from "../helpers/diff-helpers"
import { openCommitFilesSection, openCommitPreview } from "../helpers/preview-helpers"
import { availablePullRequests, getOpenedUrls, pullRequest } from "../helpers/pull-request-helpers"
import { openPlanWindow, turnOnReviewPlan } from "../helpers/review-plan-helpers"
import { availableReviewRequests, openRequestReview, reviewRequest } from "../helpers/review-request-helpers"
import { openedReview, openReviewWindow } from "../helpers/review-window-helpers"
import { selectedInDiffs } from "../helpers/selection-helpers"
import { openLinkedSubWindow, openSubWindowPage } from "../helpers/sub-window-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"

// the `commit_marks` template: one commit of the branch `ledger`. In src/Ledger.kt (11 blocks) the call of `Entry` is
// wrapped over three lines (a quiet run of formatting, the one block with nothing to read), the comment of `largest`
// is worded again (a run of comments, a label), a local is renamed, functions move and `isEmpty` really changes;
// src/Report.kt has three blocks, src/Store.kt one. The stops of Ledger.kt are read from the page, never pinned: a
// later stage of the marks (the moved lines) takes more of its eleven blocks out of the reading
const SUBJECT = "tidy the ledger and move the report title to the report"
const OLD_CALL = "    val entry = Entry(account, amount, note)"
const NEW_CALL = ["    val entry = Entry(", "      account, amount, note", "    )"]

/** The diff of Ledger.kt in the preview of the commit of the marks: its row of the tree of the files, which holds its diff */
async function openLedgerPreview(page: Page): Promise<{ preview: Locator; ledger: Locator }> {
  await expandBranch(page, findBranchRow(page, "ledger"))
  const preview = await openCommitPreview(page, getBranchCommitRow(page, "ledger", SUBJECT))
  const tree = await openCommitFilesSection(preview)
  const ledger = tree.getByRole("treeitem", { name: "Ledger.kt" })
  await ledger.getByText("Ledger.kt").click()
  await expect(ledger).toContainText("fun record(account: String, amount: Long, note: String) {")
  return { preview, ledger }
}

/** D on the preview of the commit of `branch`: its review window shown in the page, once its files stand where they stay */
async function openReview(page: Page, branch: string, subject: string): Promise<{ window: Locator; code: Locator; position: Locator }> {
  await expandBranch(page, findBranchRow(page, branch))
  await openCommitPreview(page, getBranchCommitRow(page, branch, subject))
  await page.keyboard.press("d")
  await openSubWindowPage(page, await openedReview(page, 1))
  const window = page.getByTestId("review-window")
  const code = window.getByTestId("commit-review-code")
  await expectOverviewLaidOut(code)
  return { window, code, position: window.getByTestId("review-position") }
}

/** Brings `target` back into view after a jump down the diffs took it out of the painted rows: the file of the tree shows its top */
async function scrollDiffsUpTo(code: Locator, target: Locator): Promise<void> {
  await code.page().getByRole("treeitem", { name: "Ledger.kt" }).click()
  await expect(target).toBeInViewport()
}

/** The blocks of changed lines of Ledger.kt in the template: the marks change no block */
const LEDGER_CHANGES = 11

/**
 * What `count` (the count of a file beside its lines: "10 to read") says is left to read of Ledger.kt: fewer than its
 * blocks, since the wrapped call is none of them, and its hover says it in full
 */
async function ledgerStops(count: Locator): Promise<number> {
  await expect(count).toHaveText(/^\d+ to read$/)
  const stops = Number.parseInt((await count.textContent())!, 10)
  expect(stops).toBeGreaterThan(0)
  expect(stops).toBeLessThan(LEDGER_CHANGES)
  await expect(count).toHaveAttribute("title", `${stops} of ${LEDGER_CHANGES} changes to read`)
  return stops
}

/** A row of the diffs as a test reads where N and P stop: its file, its index there, its kind, its code, and the kind of the row above it */
interface ShownRow {
  readonly path: string
  readonly row: number
  readonly kind: string
  readonly text: string
  readonly above: string
  /**
   * The diffs stand at their end: a stop of the last card shows no lower than with that card at the top, so its row is
   * not always the fifth under the header
   */
  readonly end: boolean
}

/**
 * The row N or P shows: a stop goes four rows below the header of its file, which sticks at the top of the diffs, so
 * the rows before it stay in view. The fifth row under that header, with the kind of the fourth
 */
async function shownStop(code: Locator): Promise<ShownRow> {
  return code.getByTestId("diff-view").evaluate(view => {
    const top = view.getBoundingClientRect().top
    // the header at the top: the last one that starts in the space above the first card
    const header = [...view.querySelectorAll<HTMLElement>("[data-testid=review-file-header]")].findLast(element => element.getBoundingClientRect().top - top < 24)
    const file = header?.closest<HTMLElement>("[data-testid=diff-view-file]")
    if (header === undefined || file == null) {
      throw new Error("no header at the top of the diffs")
    }
    const below = header.getBoundingClientRect().bottom - 1
    const rows = [...file.querySelectorAll<HTMLElement>("[data-testid=diff-row]")]
      .filter(row => row.getBoundingClientRect().top >= below)
      .sort((a, b) => a.getBoundingClientRect().top - b.getBoundingClientRect().top)
    const [above, stop] = [rows[3], rows[4]]
    if (above === undefined || stop === undefined) {
      throw new Error(`${rows.length} rows under the header of ${file.dataset.path}`)
    }
    const code = [...stop.querySelectorAll("[data-testid=diff-code]")].map(cell => cell.textContent ?? "").join("")
    return {
      path: file.dataset.path ?? "",
      row: Number(stop.dataset.row),
      kind: stop.dataset.kind ?? "",
      text: code,
      above: above.dataset.kind ?? "",
      end: view.scrollTop + view.clientHeight >= view.scrollHeight - 1,
    }
  })
}

/** Presses `key` (N or P) and gives the row it shows, once the diffs stand at another stop than `from` */
async function pressToStop(page: Page, code: Locator, key: "n" | "p", from: ShownRow | null): Promise<ShownRow> {
  await page.keyboard.press(key)
  let shown: ShownRow | undefined
  await expect
    .poll(async () => {
      shown = await shownStop(code).catch(() => undefined)
      return shown !== undefined && (from === null || shown.path !== from.path || shown.row !== from.row)
    })
    .toBe(true)
  return shown!
}

/** The kinds of a row of changed lines to read: no unchanged line, no row of words, no line of a quiet run that Inline shows once */
const READ_KINDS = ["added", "deleted", "inline", "changed"]

/** The text of each row of `rows`, without its line numbers */
async function codeOf(rows: Locator): Promise<string[]> {
  return rows.locator("[data-testid=diff-code]").allTextContents()
}

test.describe("The marks of a diff in the preview of a commit", () => {
  test.beforeEach(async ({ setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("commit_marks")
    await syncAndWaitForBranches()
  })

  test("Inline shows a wrapped call once, played down after its note, and a click on the note shows and hides its old line", async ({ page }) => {
    const { ledger } = await openLedgerPreview(page)
    // the three new lines of the call, once: neither inserted nor unchanged, in the quiet colour, without their old line
    const once = diffRows(ledger, "once")
    await expect.poll(() => codeOf(once)).toEqual(NEW_CALL)
    await expect(quietSides(once)).toHaveCount(3)
    await expect(diffSides(once).first()).toHaveAttribute("data-cell", "once")
    await expect(diffSides(once).first()).toHaveAttribute("data-run", "formatting")
    await expect(changedParts(once)).toHaveCount(0)
    await expect(diffRows(ledger, "deleted", { hasText: "Entry(account, amount, note)" })).toHaveCount(0)
    // the note before them says why, and is the button that shows the line they stand for
    const note = remarks(ledger, "formatting")
    await expect(note).toHaveText("Formatting only · 3 lines")
    await expect(note).toHaveAccessibleName("Formatting only, 3 lines: Show the old line")
    await expect(note).toHaveAttribute("aria-expanded", "false")
    // the preview has no key of the notes, so the note names none (the review window: H)
    await expect(note).not.toHaveAttribute("aria-keyshortcuts")

    // a click: the old line shows before the new ones, all four quiet, as Unified shows them
    await note.click()
    await expect(note).toHaveAttribute("aria-expanded", "true")
    await expect(note).toHaveAccessibleName("Formatting only, 3 lines: Hide the old line")
    const old = diffRows(ledger, "deleted", { hasText: "Entry(account, amount, note)" })
    await expect.poll(() => codeOf(old)).toEqual([OLD_CALL])
    await expect(quietSides(old)).toHaveAttribute("data-cell", "deleted")
    await expect.poll(() => codeOf(diffRows(ledger, "added").filter({ has: page.locator('[data-tint="quiet"]') }))).toEqual(NEW_CALL)
    await expect(once).toHaveCount(0)

    // a second click hides it again
    await note.click()
    await expect(note).toHaveAttribute("aria-expanded", "false")
    await expect.poll(() => codeOf(once)).toEqual(NEW_CALL)
    await expect(old).toHaveCount(0)
  })

  test("a run of comments is a label: its note opens nothing, its lines show as any change", async ({ page }) => {
    const { ledger } = await openLedgerPreview(page)
    const note = remarks(ledger, "comments")
    // far below the wrapped call
    await scrollDiffsTo(ledger, note)
    await expect(note).toHaveText("Comments only · 1 line")
    // plain words, no button: nothing is hidden
    await expect(note).not.toHaveAttribute("aria-expanded")
    await expect(ledger.getByRole("button", { name: /^Comments only/ })).toHaveCount(0)
    // the comment is a similar pair of Inline, pale with its words struck through and inserted, never quiet
    const comment = diffRows(ledger, "inline", { hasText: "entry with the largest amount." })
    await expect(deletedParts(comment)).toHaveText("The")
    await expect.poll(() => joinedText(changedParts(comment))).toBe("Finds the")
    await expect(diffSides(comment)).toHaveAttribute("data-tint", "pale")
    await expect(quietSides(comment)).toHaveCount(0)
  })

  test("Unified and Split keep every line of a wrapped call, played down, after a note that opens nothing", async ({ page }) => {
    const { preview, ledger } = await openLedgerPreview(page)
    const note = remarks(ledger, "formatting")
    await expect(note).toHaveAttribute("aria-expanded", "false")

    await preview.getByRole("button", { name: "Unified", exact: true }).click()
    await expect(note).toHaveText("Formatting only · 3 lines")
    await expect(note).not.toHaveAttribute("aria-expanded")
    // the old line, then the three new ones, each of its type and quiet; the lines of a block to read keep their colour
    const quiet = diffRows(ledger).filter({ has: page.locator('[data-tint="quiet"]') })
    await expect.poll(() => codeOf(quiet)).toEqual([OLD_CALL, ...NEW_CALL])
    await expect.poll(() => quietSides(ledger).evaluateAll(sides => sides.map(side => side.getAttribute("data-cell")))).toEqual(["deleted", "inserted", "inserted", "inserted"])
    await expect(diffRows(ledger, "once")).toHaveCount(0)
    await expect(quietSides(diffRows(ledger, "added", { hasText: "fun describe(entry: Entry): String {" }))).toHaveCount(0)

    await preview.getByRole("button", { name: "Split", exact: true }).click()
    await expect(note).not.toHaveAttribute("aria-expanded")
    // the old line beside the first new one, the two other new lines beside a filled old side
    const split = diffRows(ledger, "changed").filter({ has: page.locator('[data-tint="quiet"]') })
    await expect(split).toHaveCount(3)
    await expect.poll(() => codeOf(quietSides(split, 0))).toEqual([OLD_CALL])
    await expect.poll(() => codeOf(quietSides(split, 1))).toEqual(NEW_CALL)
    await expect(quietSides(split).first()).toHaveAttribute("data-cell", "modified")
    await expect(changedParts(split)).toHaveCount(0)
  })

  test("the row of a file says how many of its changes are left to read, and nothing where every change is read", async ({ page }) => {
    const { preview, ledger } = await openLedgerPreview(page)
    // eleven blocks, the wrapped call among those with nothing to read
    const stops = await ledgerStops(ledger.getByTestId("file-to-read"))
    await expect(ledger).toHaveAttribute("aria-description", `modified, +20 −23, ${stops} of ${LEDGER_CHANGES} changes to read`)
    // Report.kt: every block is a stop
    const report = preview.getByRole("treeitem", { name: "Report.kt" })
    await expect(report).toHaveAttribute("aria-description", "modified, +7 −2")
    await expect(report.getByTestId("file-to-read")).toHaveCount(0)
  })
})

/**
 * Two wrapped calls in src/Wrap.kt, each a block of its own that is a quiet run, between a real change in src/Alpha.kt
 * and one in src/Zeta.kt, in a commit of the branch `wrap`. With `before`, Alpha.kt has that many functions more,
 * each changed, so Wrap.kt starts below the first screen of the review
 */
async function commitWrappedCalls(repo: TestRepositoryBuilder, before = 0): Promise<void> {
  const wrap = (first: string, second: string) =>
    `class Wrap {\n  fun first() {\n${first}    done(a)\n  }\n\n  fun middle() = 0\n\n  fun second() {\n${second}    done(b)\n  }\n}\n`
  const more = (value: number) => Array.from({ length: before }, (_, index) => `fun more${index}() = ${value}\n`).join("")
  await repo.commit(
    "Add the calls",
    {
      "src/Alpha.kt": `fun alpha() = 1\n${more(1)}`,
      "src/Wrap.kt": wrap("    val a = compute(alpha, beta)\n", "    val b = compute(gamma, delta)\n"),
      "src/Zeta.kt": "fun zeta() = 1\n",
    },
    1704120600,
  )
  await repo.pushInTerminal("master")
  await repo.commit(
    "(wrap) wrap the calls",
    {
      "src/Alpha.kt": `fun alpha() = 2\n${more(2)}`,
      "src/Wrap.kt": wrap("    val a = compute(\n      alpha, beta\n    )\n", "    val b = compute(\n      gamma, delta\n    )\n"),
      "src/Zeta.kt": "fun zeta() = 2\n",
    },
    1704121200,
  )
}

/** Whether each note that opens shows its old lines: the `aria-expanded` of each, in the order of the diffs */
function expandedOf(notes: Locator): () => Promise<(string | null)[]> {
  return () => notes.evaluateAll(buttons => buttons.map(button => button.getAttribute("aria-expanded")))
}

/** The two notes of Wrap.kt are closed and whole in view, and name the key of the review that opens them */
async function expectClosedNotesInView(notes: Locator): Promise<void> {
  await expect.poll(expandedOf(notes)).toEqual(["false", "false"])
  await expect(notes.first()).toBeInViewport()
  await expect(notes.last()).toBeInViewport()
  await expect(notes.first()).toHaveAttribute("aria-keyshortcuts", "H")
}

/**
 * In a review window whose O is an action of the window (the pull request at `url`), with the two notes of Wrap.kt in
 * view: O opens the pull request and leaves the notes as they are, closed and open; H shows and hides their old lines
 * and opens nothing; from the diffs and from a row of the tree of the files
 */
async function expectWindowKeyBesideNotesKey(window: Page, code: Locator, url: string): Promise<void> {
  const notes = remarks(code, "formatting")
  const expanded = expandedOf(notes)
  const wrap = window.getByRole("treeitem", { name: "Wrap.kt" })
  await wrap.click()
  // the notes name their own key, never the one of the pull request
  await expectClosedNotesInView(notes)

  // O in the diffs, the notes in view: the pull request opens, the notes stay closed
  await code.locator("[aria-label='Changes of the commit']").focus()
  await window.keyboard.press("o")
  await expect.poll(() => getOpenedUrls(window)).toEqual([url])
  await expect.poll(expanded).toEqual(["false", "false"])
  // H shows the old line of each note and opens nothing
  await window.keyboard.press("h")
  await expect.poll(expanded).toEqual(["true", "true"])
  expect(await getOpenedUrls(window)).toEqual([url])

  // from a row of the tree, the old lines shown: O opens the pull request again and hides nothing, H hides them
  await wrap.click()
  await expect(wrap).toBeFocused()
  await window.keyboard.press("o")
  await expect.poll(() => getOpenedUrls(window)).toEqual([url, url])
  await expect.poll(expanded).toEqual(["true", "true"])
  await window.keyboard.press("h")
  await expect.poll(expanded).toEqual(["false", "false"])
  expect(await getOpenedUrls(window)).toEqual([url, url])
}

test.describe("The marks of a diff in the review window", () => {
  test("N and P stop where the reading of a block starts and pass the wrapped call; the header counts the same stops", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("commit_marks")
    await syncAndWaitForBranches()
    const { window, code, position } = await openReview(page, "ledger", SUBJECT)
    const header = code.getByTestId("review-file-header").filter({ hasText: "Ledger.kt" })
    const stops = await ledgerStops(header.getByTestId("review-file-to-read"))
    // the row of the file in the tree counts the same
    await expect(window.getByRole("treeitem", { name: "Ledger.kt" }).getByTestId("file-to-read")).toHaveText(`${stops} to read`)
    // Report.kt and Store.kt: every block a stop, no count
    await expect(code.getByTestId("review-file-header").filter({ hasText: "Report.kt" }).getByTestId("review-file-to-read")).toHaveCount(0)

    await window.getByRole("treeitem", { name: "Ledger.kt" }).click()
    await code.locator("[aria-label='Changes of the commit']").focus()
    // as many stops in Ledger.kt as its header says, fewer than its blocks, then the three of Report.kt and the one of
    // Store.kt. Each stop is the first row of lines to read of its block, four rows below the header of its file: a
    // changed line after a row that is none (an unchanged line, a note, a line shown once), further down than the last
    const shown: ShownRow[] = []
    for (const file of [...Array.from({ length: stops }, () => 1), 2, 2, 2, 3]) {
      const stop = await pressToStop(page, code, "n", shown.at(-1) ?? null)
      await expect(position).toHaveText(`File ${file} / 3`)
      // the one stop of Store.kt, the last card, shows with the card at the top: the document ends there
      if (!stop.end) {
        expect(READ_KINDS, `the stop ${JSON.stringify(stop)}`).toContain(stop.kind)
        expect(READ_KINDS, `the row above the stop ${JSON.stringify(stop)}`).not.toContain(stop.above)
      }
      shown.push(stop)
    }
    expect(shown.filter(stop => stop.end).map(stop => stop.path)).toEqual(["src/Store.kt"])
    const ledger = shown.slice(0, stops)
    expect(ledger.map(stop => stop.path)).toEqual(ledger.map(() => "src/Ledger.kt"))
    expect(ledger.map(stop => stop.row)).toEqual(ledger.map(stop => stop.row).toSorted((a, b) => a - b))
    // the first stop is the function moved up in the class; the wrapped call, the block after it, is never one
    expect(ledger[0]).toMatchObject({ kind: "added", text: "  fun accounts(): List<String> {" })
    for (const line of [OLD_CALL, ...NEW_CALL]) {
      expect(shown.map(stop => stop.text)).not.toContain(line)
    }
    // the comment worded again is a stop, a label: its note stands above the line, in view
    expect(ledger.find(stop => stop.text.includes("entry with the largest amount."))).toMatchObject({ kind: "inline", above: "note" })
    // the real change is one, and the last stop is the one of the last file
    expect(ledger.filter(stop => stop.text.includes("fun isEmpty(): Boolean = entries."))).toHaveLength(1)
    expect(shown.at(-1)).toMatchObject({ path: "src/Store.kt" })
    // nothing after the last stop: nothing moves
    await page.keyboard.press("n")
    await expect(position).toHaveText("File 3 / 3")
    expect(await shownStop(code)).toEqual(shown.at(-1))
    // back through the same stops, each at the row N showed
    let last = shown.at(-1)!
    for (const stop of shown.slice(0, -1).toReversed()) {
      last = await pressToStop(page, code, "p", last)
      expect(last).toEqual(stop)
    }
    await expect(position).toHaveText("File 1 / 3")
  })

  test("N stops at the first line a block deletes before a comment worded again, never at the comment below them", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("simple")
    // one block: six calls deleted, then the comment of the call after them worded again, a label. Rust starts the
    // reading at the first new line, the comment; the view shows the deleted lines first, in a part before the note
    const body = (calls: string, comment: string) =>
      `class Sweep {\n${Array.from({ length: 12 }, (_, index) => `  val kept${index} = ${index}\n`).join("")}\n  fun sweep() {\n${calls}    keep(1) // ${comment}\n  }\n}\n`
    const gone = Array.from({ length: 6 }, (_, index) => `    gone${index + 1}()\n`).join("")
    // a file after it: the diffs do not end at its card, so its stop goes four rows below its header
    await repo.commit("Add the sweep", { "src/Sweep.kt": body(gone, "the one kept"), "src/Zeta.kt": "fun zeta() = 1\n" }, 1704120600)
    await repo.pushInTerminal("master")
    await repo.commit("(sweep) sweep the calls", { "src/Sweep.kt": body("", "the only one left"), "src/Zeta.kt": "fun zeta() = 2\n" }, 1704121200)
    await syncAndWaitForBranches()
    const { window, code, position } = await openReview(page, "sweep", "sweep the calls")
    // one change, to read: no count beside the lines of the file
    await expect(window.getByRole("treeitem", { name: "Sweep.kt" })).toHaveAttribute("aria-description", "modified, +1 −7")
    await expect(remarks(code, "comments")).toHaveText("Comments only · 1 line")

    await code.locator("[aria-label='Changes of the commit']").focus()
    const stop = await pressToStop(page, code, "n", null)
    await expect(position).toHaveText("File 1 / 2")
    // the first deleted call, below four unchanged lines; the deletion is in view from its start
    expect(stop).toMatchObject({ kind: "deleted", text: "    gone1()", above: "context", end: false })
    await expect(diffRows(code, "deleted", { hasText: "gone1()" })).toBeInViewport()
    // the six deleted calls, then the label and the comment it is about
    await expect
      .poll(() => codeOf(diffRows(code, "deleted")))
      .toEqual(["    gone1()", "    gone2()", "    gone3()", "    gone4()", "    gone5()", "    gone6()", "    keep(1) // the one kept"])
    await expect.poll(() => codeOf(diffRows(code, "added"))).toEqual(["    keep(1) // the only one left"])
  })

  test("N and P pass a file whose every change is a quiet run, which says that nothing of it is to read", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await commitWrappedCalls(await setupRepo("simple"))
    await syncAndWaitForBranches()
    const { window, code, position } = await openReview(page, "wrap", "wrap the calls")
    const wrap = window.getByRole("treeitem", { name: "Wrap.kt" })
    await expect(wrap).toHaveAttribute("aria-description", "modified, +6 −2, None of the 2 changes needs reading")
    await expect(wrap.getByTestId("file-to-read")).toHaveText("Nothing to read")
    await expect(code.getByTestId("review-file-header").filter({ hasText: "Wrap.kt" }).getByTestId("review-file-to-read")).toHaveText("Nothing to read")

    await window.getByRole("treeitem", { name: "Alpha.kt" }).click()
    await code.locator("[aria-label='Changes of the commit']").focus()
    await page.keyboard.press("n")
    await expect(position).toHaveText("File 1 / 3")
    // from the change of Alpha.kt to the one of Zeta.kt, past Wrap.kt
    await page.keyboard.press("n")
    await expect(position).toHaveText("File 3 / 3")
    await page.keyboard.press("p")
    await expect(position).toHaveText("File 1 / 3")
  })

  // the focus stays with the diffs through a click on a button the paint replaces: WebKit differs there
  test("a click on a note opens it and ⌥-click opens and closes every note of its file", { tag: "@webkit" }, async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await commitWrappedCalls(await setupRepo("simple"))
    await syncAndWaitForBranches()
    const { code } = await openReview(page, "wrap", "wrap the calls")
    const notes = remarks(code, "formatting")
    await expect(notes).toHaveText(["Formatting only · 3 lines", "Formatting only · 3 lines"])
    const expanded = expandedOf(notes)
    await expect.poll(expanded).toEqual(["false", "false"])
    const old = diffRows(code, "deleted").filter({ has: page.locator('[data-tint="quiet"]') })

    // a click opens its own note alone; the diffs keep the focus, so their keys go on
    await notes.first().click()
    await expect.poll(expanded).toEqual(["true", "false"])
    await expect.poll(() => codeOf(old)).toEqual(["    val a = compute(alpha, beta)"])
    await expect(code.locator("[aria-label='Changes of the commit']")).toBeFocused()
    // ⌥-click on a closed note opens every note of the file, on an open one closes them all
    await notes.last().click({ modifiers: ["Alt"] })
    await expect.poll(expanded).toEqual(["true", "true"])
    await expect.poll(() => codeOf(old)).toEqual(["    val a = compute(alpha, beta)", "    val b = compute(gamma, delta)"])
    await notes.first().click({ modifiers: ["Alt"] })
    await expect.poll(expanded).toEqual(["false", "false"])
    await expect(old).toHaveCount(0)
    await expect(diffRows(code, "once")).toHaveCount(6)
  })

  // the keys of the review from the diffs and from the tree, and the focus they leave where it was: WebKit differs there
  test(
    "H shows the old lines of the notes in view and hides them again, from the diffs and from the tree of the files",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      // 60 changed lines of Alpha.kt before Wrap.kt: the wrapped calls are below the first screen of the review
      await commitWrappedCalls(await setupRepo("simple"), 60)
      await syncAndWaitForBranches()
      const { window, code, position } = await openReview(page, "wrap", "wrap the calls")
      const notes = remarks(code, "formatting")
      const expanded = expandedOf(notes)
      const old = diffRows(code, "deleted").filter({ has: page.locator('[data-tint="quiet"]') })
      const diffs = code.locator("[aria-label='Changes of the commit']")
      const wrap = window.getByRole("treeitem", { name: "Wrap.kt" })

      // no note in view, Alpha.kt at the top: the key opens nothing, as the notes say once Wrap.kt is shown
      await expect(code.getByTestId("review-file-header").filter({ hasText: "Alpha.kt" })).toBeInViewport()
      await diffs.focus()
      await page.keyboard.press("h")
      await wrap.click()
      await expect(position).toHaveText("File 2 / 3")
      // each note names the key of the review that opens it
      await expectClosedNotesInView(notes)

      // both notes in view: H in the diffs shows the old line of each, the view stays at its file, the diffs keep the focus
      await diffs.focus()
      await page.keyboard.press("h")
      await expect.poll(expanded).toEqual(["true", "true"])
      await expect.poll(() => codeOf(old)).toEqual(["    val a = compute(alpha, beta)", "    val b = compute(gamma, delta)"])
      await expect(diffs).toBeFocused()
      await expect(position).toHaveText("File 2 / 3")
      // they all show: H hides them again
      await page.keyboard.press("h")
      await expect.poll(expanded).toEqual(["false", "false"])
      await expect(old).toHaveCount(0)

      // one opened by a click, the other closed: H opens what is closed, the open one stays
      await notes.first().click()
      await expect.poll(expanded).toEqual(["true", "false"])
      await page.keyboard.press("h")
      await expect.poll(expanded).toEqual(["true", "true"])

      // from a row of the tree, as N and P: the row keeps the focus
      await wrap.click()
      await expect(wrap).toBeFocused()
      await page.keyboard.press("h")
      await expect.poll(expanded).toEqual(["false", "false"])
      await expect(old).toHaveCount(0)
      await expect(wrap).toBeFocused()
      await expect(position).toHaveText("File 2 / 3")
    },
  )

  // the key of an action of the window and the key of the notes, each from the diffs: the focus of the diffs differs in WebKit
  test(
    "O opens the pull request of a reviewed branch while its notes are in view, and H shows and hides their old lines",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const opened = pullRequest("wrap", 42)
      const repo = await setupRepo("simple", { pullRequests: availablePullRequests(opened) })
      await commitWrappedCalls(repo)
      await syncAndWaitForBranches()
      await expect(findBranchRow(page, "wrap").getByTestId("pull-request-chip")).toBeVisible()
      const window = await openLinkedSubWindow(page, await openReviewWindow(page, "wrap"))
      // the title bar names the pull request, which O opens
      await expect(window.getByTestId("branch-review-actions").getByRole("button", { name: /^Open Pull Request #42/ })).toBeVisible()
      await expectWindowKeyBesideNotesKey(window, window.getByTestId("commit-review-code"), opened.url)
    },
  )

  test(
    "O opens a pull request under review on GitHub while its notes are in view, and H shows and hides their old lines",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const request = reviewRequest("wrap", 301, 1, { title: "Wrap the calls" })
      const repo = await setupRepo("simple", { reviewRequests: availableReviewRequests([request]) })
      await commitWrappedCalls(repo)
      await syncAndWaitForBranches()
      const { window, review, fetched } = await openRequestReview(page, 301)
      // Changes waits for the fetch of the pull request
      await fetched
      await expect(review.getByTestId("review-dialog-review")).toBeEnabled()
      await window.keyboard.press("d")
      await expectWindowKeyBesideNotesKey(window, review.getByTestId("commit-review-code"), request.url)
    },
  )

  test("the find searches the old line a note stands for and lands on the note; opened, the hit is in the line", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // 60 changed lines of Alpha.kt before Wrap.kt: its wrapped calls are below the first screen, so a step has to scroll
    await commitWrappedCalls(await setupRepo("simple"), 60)
    await syncAndWaitForBranches()
    const { window, code } = await openReview(page, "wrap", "wrap the calls")
    await expect(code.getByTestId("review-file-header").filter({ hasText: "Alpha.kt" })).toBeInViewport()
    await code.locator("[aria-label='Changes of the commit']").focus()
    await page.keyboard.press("ControlOrMeta+f")
    const field = window.getByRole("textbox", { name: "Find in the review" })
    await expect(field).toBeFocused()
    // only the old line holds the first call on one line: the three new lines break it
    await field.fill("compute(alpha")
    const count = window.getByTestId("diff-find-count")
    await expect(count).toHaveText("1 result")
    // Alpha.kt is still at the top: no row of Wrap.kt is in view
    await expect(code.getByTestId("review-file-header").filter({ hasText: "Alpha.kt" })).toBeInViewport()
    await expect(code.getByTestId("review-file-header").filter({ hasText: "Wrap.kt" })).not.toBeInViewport()

    // Enter steps to it: the view scrolls to the note of the first call, which names the hit it holds, the current one
    await page.keyboard.press("Enter")
    await expect(count).toHaveText("1 of 1")
    const hits = remarkHits(code)
    await expect(hits).toHaveText("1 match in the old line")
    await expect(hits).toHaveAttribute("data-hit", "current")
    await expect(hits).toBeInViewport()
    const first = remarks(code, "formatting").first()
    await expect(first).toBeInViewport()
    await expect(first).toHaveAttribute("aria-expanded", "false")
    await expect(code.getByTestId("review-file-header").filter({ hasText: "Alpha.kt" })).not.toBeInViewport()
    await expect(findMarks(diffRows(code, "once"))).toHaveCount(0)

    // the note opened: the hit is in the old line it stood for, still the current one, and the note names none
    await first.click()
    const old = diffRows(code, "deleted", { hasText: "compute(alpha, beta)" })
    await expect.poll(() => joinedText(findMarks(old, "current"))).toBe("compute(alpha")
    await expect(count).toHaveText("1 of 1")
    await expect(hits).toHaveCount(0)

    // a text of both forms of the call is found in the note and in the new line; the other note holds no hit
    await first.click()
    await expect(first).toHaveAttribute("aria-expanded", "false")
    await field.fill("alpha, beta")
    await expect(count).toHaveText("2 results")
    await expect(hits).toHaveText("1 match in the old line")
    await expect(hits).toHaveAttribute("data-hit", "")
    await expect.poll(() => joinedText(findMarks(diffRows(code, "once")))).toBe("alpha, beta")
  })

  // a selection the view makes in a row painted anew, and the focus of the view for the key: WebKit differs there
  test(
    "Back to a name of an old line that a closed note stands for opens the note and selects the name in the line",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches, browserName }) => {
      await setupRepo("commit_marks")
      await syncAndWaitForBranches()
      const { code } = await openReview(page, "ledger", SUBJECT)
      const note = remarks(code, "formatting")
      await note.click()
      const old = diffRows(code, "deleted", { hasText: "Entry(account, amount, note)" })
      await expect.poll(() => codeOf(old)).toEqual([OLD_CALL])
      // ⌘-click on `Entry` of the old line: its declaration in the old text of the file, the old line the place to return to
      const at = await old.evaluate(row => {
        const walker = document.createTreeWalker(row.querySelector("[data-testid=diff-code]")!, NodeFilter.SHOW_TEXT)
        for (let text = walker.nextNode(); text !== null; text = walker.nextNode()) {
          const index = (text.textContent ?? "").indexOf("Entry")
          if (index >= 0) {
            const range = document.createRange()
            range.setStart(text, index)
            range.setEnd(text, index + "Entry".length)
            const rect = range.getBoundingClientRect()
            return { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 }
          }
        }
        throw new Error("no Entry in the old line")
      })
      await page.keyboard.down("Meta")
      await page.mouse.click(at.x, at.y)
      await page.keyboard.up("Meta")
      await expect.poll(() => selectedInDiffs(page)).toEqual({ text: "Entry", path: "src/Ledger.kt" })
      await expect(diffRows(code, "context", { hasText: "data class Entry(" })).toBeInViewport()

      // the note closed: the old line has no row again
      await scrollDiffsUpTo(code, note)
      await note.click()
      await expect(note).toHaveAttribute("aria-expanded", "false")
      await expect(old).toHaveCount(0)
      // Back: the note opens as a fold does, and the name is selected in the old line. Chromium of the tests says it
      // runs on Windows (Ctrl+Alt+←), WebKit on macOS (⌘[)
      await page.keyboard.press(browserName === "webkit" ? "Meta+BracketLeft" : "Control+Alt+ArrowLeft")
      await expect(note).toHaveAttribute("aria-expanded", "true")
      await expect.poll(() => codeOf(old)).toEqual([OLD_CALL])
      await expect.poll(() => selectedInDiffs(page)).toEqual({ text: "Entry", path: "src/Ledger.kt" })
      await expect(old).toBeInViewport()
    },
  )

  test("a copy takes what a quiet run shows: its new lines, and its old line once the note is opened", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("commit_marks")
    await syncAndWaitForBranches()
    const { code } = await openReview(page, "ledger", SUBJECT)
    const note = remarks(code, "formatting")
    await expect(note).toHaveAttribute("aria-expanded", "false")
    /** Selects from the row of the note to the end of the last line of the run, then copies with the key of the system */
    const copyRun = async (): Promise<string | null> => {
      await code.evaluate(element => {
        const from = element.querySelector("[data-testid=diff-row][data-tone=remark][data-quiet]")!
        const cells = [...element.querySelectorAll('[data-testid=diff-side][data-tint="quiet"] [data-testid=diff-code]')]
        const range = document.createRange()
        range.setStart(from, 0)
        range.setEnd(cells.at(-1)!, cells.at(-1)!.childNodes.length)
        document.getSelection()!.removeAllRanges()
        document.getSelection()!.addRange(range)
      })
      return copiedByKey(page)
    }
    expect(await copyRun()).toBe(NEW_CALL.join("\n"))
    await note.click()
    await expect(note).toHaveAttribute("aria-expanded", "true")
    expect(await copyRun()).toBe([OLD_CALL, ...NEW_CALL].join("\n"))
  })
})

test.describe("The marks of a diff in a review with its plan", () => {
  test("a run that starts its block says its words in the heading of the change, and opens its old line from there", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("simple")
    // the body of `total`: its first statement wrapped over three lines, a quiet run, then a value really changed, one block
    const total = (fold: string, result: string) => `class Total {\n  fun total(items: List<Int>): Int {\n${fold}    return ${result}\n  }\n\n  fun other() = 1\n}\n`
    await repo.commit("Add the total", { "src/Total.kt": total("    val sum = items.fold(0) { a, b -> a + b }\n", "sum") }, 1704120600)
    await repo.pushInTerminal("master")
    await repo.commit(
      "(total) the total counts one more\n\nThe total of the items counts one more.\n",
      { "src/Total.kt": total("    val sum = items.fold(0) { a, b ->\n      a + b\n    }\n", "sum + 1") },
      1704121200,
    )
    turnOnReviewPlan(repo)
    const window = await openPlanWindow(page, syncAndWaitForBranches, "total")
    const code = window.getByTestId("commit-review-code")
    // one row of words before the block: the heading of the change, then the words of the run
    const notes = diffRows(code, "note")
    await expect(notes).toHaveText(["total — body changed · Formatting only · 3 lines"])
    await expect(notes.getByTestId("diff-unit-heading")).toHaveText("total — body changed")
    const note = remarks(notes, "formatting")
    await expect(note).toHaveAttribute("aria-expanded", "false")
    await expect.poll(() => codeOf(diffRows(code, "once"))).toEqual(["    val sum = items.fold(0) { a, b ->", "      a + b", "    }"])

    await note.click()
    await expect(note).toHaveAttribute("aria-expanded", "true")
    await expect.poll(() => codeOf(diffRows(code, "deleted").filter({ has: window.locator('[data-tint="quiet"]') }))).toEqual(["    val sum = items.fold(0) { a, b -> a + b }"])
    await expect(notes).toHaveCount(1)
  })
})

/**
 * The wrapped call of the preview of the template in the theme of the page: its lines and its note have the colour the
 * token `--diff-quiet` names, an inserted line and an unchanged one another; the token is light or dark as the theme
 */
async function expectQuietColour(page: Page, theme: "light" | "dark"): Promise<void> {
  const { ledger } = await openLedgerPreview(page)
  await expect(diffRows(ledger, "once")).toHaveCount(3)
  // the background of the first element of each kind, and the colour the token of the theme names
  const { token, once, note, inserted, context } = await ledger.evaluate(element => {
    const background = (selector: string) => getComputedStyle(element.querySelector(selector)!).backgroundColor
    const probe = document.createElement("div")
    probe.style.background = "var(--diff-quiet)"
    element.append(probe)
    const named = getComputedStyle(probe).backgroundColor
    probe.remove()
    return {
      token: named,
      once: background('[data-testid=diff-side][data-cell="once"]'),
      note: background("[data-testid=diff-row][data-tone=remark][data-quiet]"),
      inserted: background('[data-testid=diff-side][data-cell="inserted"]:not([data-tint])'),
      context: background('[data-testid=diff-side][data-cell="context"]'),
    }
  })
  expect([once, note]).toEqual([token, token])
  expect([inserted, context]).not.toContain(token)
  // the first channel of the colour: light on the light card, dark on the dark one
  expect(Number(/\d+/.exec(token)![0]) > 128 ? "light" : "dark").toBe(theme)
}

test.describe("The quiet colour of the light theme", () => {
  test.use({ colorScheme: "light" })

  test("a quiet line and its note have the quiet colour of the light theme, which is no colour of a change", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("commit_marks")
    await syncAndWaitForBranches()
    await expectQuietColour(page, "light")
  })
})

test.describe("The quiet colour of the dark theme", () => {
  test.use({ colorScheme: "dark" })

  test("a quiet line and its note have the quiet colour of the dark theme, which is no colour of a change", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("commit_marks")
    await syncAndWaitForBranches()
    await expectQuietColour(page, "dark")
  })
})

test.describe("A change of whitespace in a file without a grammar", () => {
  test("stays the pale stop it was: no note, no quiet line, and N stops at it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("simple")
    // two lines indented again in a file no grammar reads, between a real change of the file before and of the one after
    const text = (indent: string) => `first\n${indent}second\n${indent}third\nfourth\n`
    await repo.commit("Add the notes", { "src/Alpha.kt": "fun alpha() = 1\n", "src/Beta.txt": text(""), "src/Zeta.kt": "fun zeta() = 1\n" }, 1704120600)
    await repo.pushInTerminal("master")
    await repo.commit("(plain) indent two lines", { "src/Alpha.kt": "fun alpha() = 2\n", "src/Beta.txt": text("    "), "src/Zeta.kt": "fun zeta() = 2\n" }, 1704121200)
    await syncAndWaitForBranches()
    const { window, code, position } = await openReview(page, "plain", "indent two lines")
    // its one block is to read: no count beside the lines of the file
    const file = window.getByRole("treeitem", { name: "Beta.txt" })
    await expect(file).toHaveAttribute("aria-description", "modified, +2 −2")
    // the indented lines are similar pairs, pale, the indentation in the full colour; the diff says nothing of them
    const indented = diffRows(code, "inline", { hasText: "second" })
    await expect(diffSides(indented)).toHaveAttribute("data-tint", "pale")
    await expect(changedParts(indented)).toHaveText("    ")
    const beta = code.locator('[data-testid=diff-view-file][data-path="src/Beta.txt"]')
    await expect(diffRows(beta, "inline")).toHaveCount(2)
    await expect(remarks(beta)).toHaveCount(0)
    await expect(quietSides(beta)).toHaveCount(0)

    // N goes through the three files: the indentation is a stop
    await window.getByRole("treeitem", { name: "Alpha.kt" }).click()
    await code.locator("[aria-label='Changes of the commit']").focus()
    for (const stop of [1, 2, 3]) {
      await page.keyboard.press("n")
      await expect(position).toHaveText(`File ${stop} / 3`)
    }
  })
})
