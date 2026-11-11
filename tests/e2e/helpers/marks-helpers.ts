import type { Locator, Page } from "@playwright/test"
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
