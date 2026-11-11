import type { Locator, Page } from "@playwright/test"
import type { DiffBlock, DiffMarks } from "../../../src/lib/bindings"
import { expect } from "../fixtures/base-test"
import { expandBranch, findBranchRow, getBranchCommitRow } from "./branch-helpers"
import { expectOverviewLaidOut } from "./diff-helpers"
import { openCommitFilesSection, openCommitPreview } from "./preview-helpers"
import { openedReview } from "./review-window-helpers"
import { openSubWindowPage } from "./sub-window-helpers"

/*
 * The diffs of the `commit_marks` template (`crates/test-utils/src/repo_template.rs`), whose one commit of the branch
 * `ledger` has a case of every kind of mark of a diff (openspec/specs/commit/quiet-changes/spec.md): what the specs of
 * the marks open.
 */

/** The subject of the commit of the marks, without the prefix of its branch */
export const MARKS_SUBJECT = "tidy the ledger and move the report title to the report"

/** The diff of Ledger.kt in the preview of the commit of the marks: its row of the tree of the files, which holds its diff */
export async function openLedgerPreview(page: Page): Promise<{ preview: Locator; ledger: Locator }> {
  await expandBranch(page, findBranchRow(page, "ledger"))
  const preview = await openCommitPreview(page, getBranchCommitRow(page, "ledger", MARKS_SUBJECT))
  const tree = await openCommitFilesSection(preview)
  const ledger = tree.getByRole("treeitem", { name: "Ledger.kt" })
  await ledger.getByText("Ledger.kt").click()
  await expect(ledger).toContainText("fun record(account: String, amount: Long, note: String) {")
  return { preview, ledger }
}

/** D on the preview of the commit of `branch`: its review window shown in the page, once its files stand where they stay */
export async function openMarksReview(page: Page, branch: string, subject: string): Promise<{ window: Locator; code: Locator; position: Locator }> {
  await expandBranch(page, findBranchRow(page, branch))
  await openCommitPreview(page, getBranchCommitRow(page, branch, subject))
  await page.keyboard.press("d")
  await openSubWindowPage(page, await openedReview(page, 1))
  const window = page.getByTestId("review-window")
  const code = window.getByTestId("commit-review-code")
  await expectOverviewLaidOut(code)
  return { window, code, position: window.getByTestId("review-position") }
}

/**
 * The scopes of the file `path` of the diffs in `code` have come: a fold of it names the declaration around its lines.
 * The backend sends the scopes of a text it parsed late after its diff, and the rows of the file are laid out and
 * painted again as they come, which drops a selection made in them before. A test that selects in the rows of a file
 * waits for this first
 */
export async function expectScopesShown(code: Locator, path: string): Promise<void> {
  await expect(code.getByRole("region", { name: path }).getByTestId("diff-fold-label")).not.toHaveCount(0)
}

/**
 * The code of each of `rows` (rows, or the cells of a side), without its line numbers, in the order of the rows of
 * their files: the painter adds a row to the page as a scroll brings it near, so the order of the page is the order of
 * the paints, never the order a reader sees
 */
export async function codeOf(rows: Locator): Promise<string[]> {
  return rows.evaluateAll(elements =>
    elements
      .map(element => {
        const row = element.closest<HTMLElement>("[data-testid=diff-row]")
        return {
          file: Number(row?.parentElement?.dataset.file),
          row: Number(row?.dataset.row),
          code: (element.matches("[data-testid=diff-code]") ? element : element.querySelector("[data-testid=diff-code]"))?.textContent ?? "",
        }
      })
      .toSorted((a, b) => a.file - b.file || a.row - b.row)
      .map(({ code }) => code),
  )
}

/** ⌘F in the diffs of a review and `text` typed into its field: the field, and the count of its results */
export async function findInReview(window: Locator, code: Locator, text: string): Promise<{ field: Locator; count: Locator }> {
  await code.locator("[aria-label='Changes of the commit']").focus()
  await code.page().keyboard.press("ControlOrMeta+f")
  const field = window.getByRole("textbox", { name: "Find in the review" })
  await expect(field).toBeFocused()
  await field.fill(text)
  return { field, count: window.getByTestId("diff-find-count") }
}

/** The marks of a diff as the stream of `get_file_diffs` carries them, and the lines of a block (`DiffMarks`, `DiffBlock` of the bindings) */
type StreamMarks = Pick<DiffMarks, "runs" | "renames" | "starts">
type StreamBlock = Pick<DiffBlock, "oldStart" | "oldEnd" | "newStart" | "newEnd">

/**
 * The marks of a file as the engine gives them once the kind `renamed` is switched on (`is_quiet` of
 * crates/file-diffs/src/marks.rs, which `PairMarks::as_proven` answers today): a run of a renamed name whose sites are
 * of locals alone is quiet, and a block whose every line is in a quiet run has no line to start its reading at. A run
 * with a site of a name that is not local, or of a name another file declares, stays the label it is. Only whole
 * blocks: a block such a run leaves lines of would start at another line, which Rust alone says
 */
function withRenamedOn(marks: StreamMarks, blocks: readonly StreamBlock[]): void {
  const linesOf = (local: boolean) => new Set(marks.renames.filter(rename => rename.local === local).flatMap(rename => rename.newSites.filter((_, at) => at % 3 === 0)))
  const [local, other] = [linesOf(true), linesOf(false)]
  const switched = new Set<number>()
  for (const run of marks.runs) {
    const lines = Array.from({ length: run.newEnd - run.newStart }, (_, at) => run.newStart + at)
    if (run.kind === "renamed" && !run.quiet && lines.some(line => local.has(line)) && !lines.some(line => other.has(line))) {
      run.quiet = true
      blocks.forEach((block, index) => {
        if (block.newStart <= run.newStart && run.newEnd <= block.newEnd) {
          switched.add(index)
        }
      })
    }
  }
  marks.starts = Array.from({ length: marks.starts.length / 3 }, (_, at) => marks.starts.slice(at * 3, at * 3 + 3)).flatMap(start => {
    const index = start[0]!
    const block = blocks[index]!
    const quiet = marks.runs.filter(run => run.quiet && block.newStart <= run.newStart && run.newEnd <= block.newEnd)
    const count = (side: "old" | "new") => quiet.reduce((lines, run) => lines + run[`${side}End`] - run[`${side}Start`], 0)
    if (count("old") === block.oldEnd - block.oldStart && count("new") === block.newEnd - block.newStart) {
      return []
    }
    if (switched.has(index)) {
      throw new Error(`block ${index} is read in part beside a quiet run of a renamed name: only Rust says where its reading starts`)
    }
    return start
  })
}

/**
 * The answers of `get_file_diffs` in `page`, and in the windows it shows, as the engine gives them once the kind
 * `renamed` is switched on (`withRenamedOn`): the command runs on the server, on the real engine, and its answer is
 * rewritten on the way, the marks of each file in its `content` event and in a later `marks` event of it. Until that
 * switch a run of a renamed name is a label, so what a view shows of a quiet one has no other way into a browser; with
 * the switch this goes and its tests stay
 */
export async function switchRenamedOn(page: Page): Promise<void> {
  await page.route("**/invoke/get_file_diffs", async route => {
    const response = await route.fetch()
    const blocks = new Map<number, readonly StreamBlock[]>()
    // the events of the channel, one a line of the stream (`invoke.rs`)
    const body = (await response.text())
      .split("\n")
      .map(line => {
        if (!line.startsWith("data:")) {
          return line
        }
        const message = JSON.parse(line.slice("data:".length)) as {
          progress?: { kind: string; index: number; content?: { kind: string; blocks: StreamBlock[]; marks: StreamMarks }; marks?: StreamMarks }
        }
        const event = message.progress
        if (event?.kind === "content" && event.content?.kind === "text") {
          blocks.set(event.index, event.content.blocks)
          withRenamedOn(event.content.marks, event.content.blocks)
        } else if (event?.kind === "marks" && event.marks !== undefined) {
          withRenamedOn(event.marks, blocks.get(event.index) ?? [])
        }
        return `data: ${JSON.stringify(message)}`
      })
      .join("\n")
    await route.fulfill({ response, body })
  })
}
