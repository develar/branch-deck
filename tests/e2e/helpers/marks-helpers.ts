import { existsSync, readFileSync, writeFileSync } from "node:fs"
import path from "node:path"
import type { Locator, Page } from "@playwright/test"
import type { DiffMarks } from "../../../src/lib/bindings"
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

/** The marks of a diff as the stream of `get_file_diffs` carries them (`DiffMarks` of the bindings) */
type StreamMarks = Pick<DiffMarks, "runs" | "renames" | "starts" | "subject">

/**
 * The marks of a file as its content gives them where its pair had to be compared, before the files of its subject say
 * anything of it: without what they say (`subject`) and without the runs they add, the lines that changed in a name
 * another file declares alone (`SubjectMarks.used`). Only where no such run holds a site of a rename of the file
 * itself, which the pair may have cut by itself
 */
function withoutSubject(marks: StreamMarks): void {
  const used = new Set((marks.subject?.used ?? []).flatMap(it => it.rename.newSites.filter((_, at) => at % 3 === 0)))
  const own = new Set(marks.renames.flatMap(rename => rename.newSites.filter((_, at) => at % 3 === 0)))
  marks.runs = marks.runs.filter(run => {
    const lines = Array.from({ length: run.newEnd - run.newStart }, (_, at) => run.newStart + at)
    if (run.kind !== "renamed" || !lines.some(line => used.has(line))) {
      return true
    }
    if (lines.some(line => own.has(line))) {
      throw new Error("a run of a name another file declares holds a rename of the file too: only Rust says what the pair alone gives")
    }
    return false
  })
  marks.subject = null
}

/**
 * The marks of the subject come late for the file `path` in `page`, and in the windows it shows: its diff says nothing
 * of its subject, as where its pair had to be compared (`withoutSubject`), and they come whole with the answer of the
 * index of the repository, which `holdCommand(page, "get_index_marks")` holds for as long as a test reads the rows
 * before it. The server sends the marks of a subject right after the contents, in one answer, which a test cannot stop
 * between two of its events: this is that late event, held. The command runs on the test server, on the real engine,
 * and the marks of the file change in the `content` event of its diff on their way; a later `marks` event of it is
 * left out. No page answer of a read: the server answered
 */
export async function lateSubjectMarks(page: Page, path: string): Promise<void> {
  await page.route("**/invoke/get_file_diffs", async route => {
    const response = await route.fetch()
    const paths: string[] = []
    // the events of the channel, one a line of the stream (`invoke.rs`)
    const body = (await response.text())
      .split("\n")
      .flatMap(line => {
        if (!line.startsWith("data:")) {
          return [line]
        }
        const message = JSON.parse(line.slice("data:".length)) as {
          progress?: {
            kind: string
            index: number
            files?: { path: string }[]
            content?: { kind: string; marks: StreamMarks }
            marks?: StreamMarks
          }
        }
        const event = message.progress
        if (event?.kind === "files") {
          paths.push(...(event.files ?? []).map(file => file.path))
        }
        if (event === undefined || paths[event.index] !== path) {
          return [line]
        }
        if (event.kind === "content" && event.content?.kind === "text") {
          withoutSubject(event.content.marks)
        } else if (event.kind === "marks") {
          // what the other files say of it comes with the answer of the index
          return []
        }
        return [`data: ${JSON.stringify(message)}`]
      })
      .join("\n")
    await route.fulfill({ response, body })
  })
}

/**
 * What the index of the repository answers of the function the store of the `commit_marks` template declares and
 * renamed (`openStore` to `connectStore`, src/Store.kt, used twice in src/Report.kt), as accio answers
 * (`accio renames`, `RenameAnswer` of crates/diff-marks/src/index.rs, final): the fixture of the test server
 * (`FixtureIndex`), written into the repository at `repositoryPath` before a view asks. Its declaration and its two
 * calls in both trees, every use a place of the rename: a proof. With `outside` a call more of the old name in the
 * old tree, in a file the commit left alone: the rename is proven no more, and the name is used outside the change
 */
export function answerStoreRename(repositoryPath: string, { outside = false }: { outside?: boolean } = {}): void {
  const use = (file: string, line: number, column: number, end: number, kind: string) => ({ file, line, column, end_column: end, kind, resolved: "import" })
  // the name is 9 characters in the old tree and 12 in the new one; a file the commit left alone still calls the old one
  const uses = (length: number, more: ReturnType<typeof use>[] = []) => {
    const listed = [
      use("src/Report.kt", 5, 16, 16 + length, "call"),
      use("src/Report.kt", 11, 16, 16 + length, "call"),
      use("src/Store.kt", 19, 4, 4 + length, "declaration"),
      ...more,
    ]
    return { total: listed.length, uses: listed }
  }
  const kotlin = { language: "kotlin" }
  const base = uses(9, outside ? [use("src/Main.kt", 3, 14, 23, "call")] : [])
  const renamed = { same: true, complete: true, final: true, old: kotlin, new: kotlin, base, head: uses(12), conflicts: [] }
  writeFixtureIndex(repositoryPath, { renames: { "src/Store.kt:19:5": renamed } })
}

/** Writes `answers` into the fixture of the index of the repository (`FixtureIndex`), beside what it holds already */
function writeFixtureIndex(repositoryPath: string, answers: Record<string, unknown>): void {
  const file = path.join(repositoryPath, ".git", "branch-deck-accio.json")
  const held = existsSync(file) ? (JSON.parse(readFileSync(file, "utf8")) as Record<string, unknown>) : {}
  writeFileSync(file, JSON.stringify({ ...held, ...answers }))
}

/** The subject of the commit of the `declaration_moves` template, without the prefix of its branch `ledger` */
export const MOVES_SUBJECT = "move the summary and the title to the report"

/**
 * The index of the repository of the commit of the `declaration_moves` template (`crates/test-utils/src/repo_template.rs`)
 * says that `summary` and `reportTitle` moved whole from the ledger to the report (`accio moves`, `FixtureIndex`, as
 * `declaration_moves_index` writes it): `summary` from its four lines 7 to 10 of the ledger (from 1) to the seven
 * lines 12 to 18 of the report, its parameters one a line, `reportTitle` as it is, from 12 to 15 to 7 to 10. The diffs
 * of the commit mark the summary as a declaration that moved, by its name, once the index is asked; the title is a
 * row of the same lines before that and stays one
 */
export function answerStoreMoves(repositoryPath: string): void {
  const at = (file: string, line: number, endLine: number, name: string) => ({ file, line, end_line: endLine, column: 4, kind: "function", name, language: "kotlin" })
  const moved = (name: string, [oldLine, oldEnd]: [number, number], [newLine, newEnd]: [number, number]) => ({
    old: at("src/Ledger.kt", oldLine, oldEnd, name),
    new: at("src/Report.kt", newLine, newEnd, name),
    file: true,
    owner: false,
  })
  writeFixtureIndex(repositoryPath, {
    moves: { moved: [moved("summary", [7, 10], [12, 18]), moved("reportTitle", [12, 15], [7, 10])], reordered: [], total: 2, complete: true },
  })
}
