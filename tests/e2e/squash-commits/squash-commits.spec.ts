// @spec spec/commit/squash.spec.md
import type { Locator, Page } from "@playwright/test"
import { expect, test, testWithUnassigned } from "../fixtures/test-fixtures"
import { expandBranch, findBranchRow, getBranchCommitRow, getBranchDetailsRow, getUnassignedCommitRow, verifyCommitCount } from "../helpers/branch-helpers"
import { clickContextMenuItem, multiSelectCommit, openContextMenu, selectCommit, shiftSelectCommit } from "../helpers/selection-helpers"
import { syncFromMenu } from "../helpers/sync-helpers"

const ISSUE = "ABC-123"

function squashDialog(page: Page): Locator {
  return page.getByTestId("squash-dialog")
}

/** Selects "Add tokens" and "Add parser" (not adjacent: "Add lexer" is in between) and presses Squash of the selection */
async function openSquashOfParserAndTokens(page: Page): Promise<void> {
  await selectCommit(getBranchCommitRow(page, ISSUE, "Add tokens"))
  await multiSelectCommit(getBranchCommitRow(page, ISSUE, "Add parser"))
  await getBranchDetailsRow(page, ISSUE).getByTestId("selection-actions").getByRole("button", { name: "Squash" }).click()
}

test.describe("Squash commits", () => {
  test("should squash selected branch commits into the oldest one and undo it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("issue_split")
    await syncAndWaitForBranches()
    const issueRow = findBranchRow(page, ISSUE)
    await verifyCommitCount(issueRow, "3 commits")
    await expandBranch(page, issueRow)

    // not adjacent: "Add lexer" stays in between; the dialog lists the commits oldest first, as the message reads
    await openSquashOfParserAndTokens(page)

    const dialog = squashDialog(page)
    await expect(dialog).toBeVisible()
    await expect(dialog.getByRole("heading")).toHaveText("Squash 2 commits")
    await expect(dialog.getByTestId("squash-commits").locator("li")).toHaveText([/Add parser/, /Add tokens/])
    const editor = dialog.getByTestId("squash-message-editor")
    await expect(editor).toBeFocused()
    // the newer subject loses the issue key it is grouped by, as a prefix would be removed
    await expect(editor).toHaveValue("ABC-123: Add parser\n\nAdd tokens")
    await expect(dialog).toContainText("The first line is the subject")

    await page.keyboard.press("ControlOrMeta+Enter")
    await expect(dialog).toBeHidden()
    await expect(page.getByText("Squashed 2 commits")).toBeVisible()
    await verifyCommitCount(issueRow, "2 commits")
    await expect(getBranchCommitRow(page, ISSUE, "Add tokens")).toHaveCount(0)
    await expect(getBranchCommitRow(page, ISSUE, "Add lexer")).toBeVisible()
    await expect(getBranchDetailsRow(page, ISSUE).getByTestId("selection-actions")).toHaveCount(0)

    await page.getByRole("button", { name: "Undo" }).click()
    await verifyCommitCount(issueRow, "3 commits")
    await expect(getBranchCommitRow(page, ISSUE, "Add tokens")).toBeVisible()
  })

  test("should refuse to undo a squash after a commit on top", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("issue_split")
    await syncAndWaitForBranches()
    const issueRow = findBranchRow(page, ISSUE)
    await expandBranch(page, issueRow)
    await openSquashOfParserAndTokens(page)
    // the key reaches the dialog's handler only once the dialog moved the focus into its message
    await expect(squashDialog(page).getByTestId("squash-message-editor")).toBeFocused()
    await page.keyboard.press("ControlOrMeta+Enter")
    const squashed = page.locator("[data-slot=toast]").filter({ hasText: "Squashed 2 commits" })
    await expect(squashed).toBeVisible()
    await verifyCommitCount(issueRow, "2 commits")

    // a commit in a terminal: the branch moved on since the squash, so its undo is stale for good
    await repo.commit(`${ISSUE}: Add the grammar`, { "grammar.js": "export const grammar = {}\n" }, 1704300000)
    await squashed.getByRole("button", { name: "Undo" }).click()
    const refusal = page.locator("[data-slot=toast]").filter({ hasText: "Cannot undo the squash" })
    await expect(refusal).toContainText(/moved: HEAD is .*; it can't be undone any more/)
    await expect(refusal).not.toContainText("retry")

    // nothing was undone: the next sync shows the squash and the new commit
    await syncFromMenu(page)
    await verifyCommitCount(issueRow, "3 commits")
    await expect(getBranchCommitRow(page, ISSUE, "Add the grammar")).toBeVisible()
    await expect(getBranchCommitRow(page, ISSUE, "Add tokens")).toHaveCount(0)
  })

  test("should offer squash in the context menu of two or more commits and cancel with Escape", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("issue_split")
    await syncAndWaitForBranches()
    const issueRow = findBranchRow(page, ISSUE)
    await expandBranch(page, issueRow)

    await openContextMenu(page, getBranchCommitRow(page, ISSUE, "Add parser"))
    await expect(page.getByRole("menuitem", { name: "Move to…" })).toBeVisible()
    await expect(page.getByRole("menuitem", { name: /Squash/ })).toHaveCount(0)
    await page.keyboard.press("Escape")

    await selectCommit(getBranchCommitRow(page, ISSUE, "Add tokens"))
    await shiftSelectCommit(getBranchCommitRow(page, ISSUE, "Add parser"))
    await openContextMenu(page, getBranchCommitRow(page, ISSUE, "Add lexer"))
    await clickContextMenuItem(page, "Squash 3 Commits…")

    const dialog = squashDialog(page)
    const editor = dialog.getByTestId("squash-message-editor")
    await expect(editor).toHaveValue("ABC-123: Add parser\n\nAdd lexer\n\nAdd tokens")
    // a new prefix moves the squashed commit to another branch
    await editor.fill("(parser) ABC-123: Add the parser")
    await expect(dialog.getByTestId("commit-prefix-change")).toHaveText("Squashing moves the commit to the branch parser")

    await page.keyboard.press("Escape")
    await expect(dialog).toBeHidden()
    await verifyCommitCount(issueRow, "3 commits")
  })
})

testWithUnassigned("should squash unassigned commits with the S key", async ({ page, setupUnassignedRepo }) => {
  await setupUnassignedRepo()

  await selectCommit(getUnassignedCommitRow(page, "Fix login validation bug"))
  await shiftSelectCommit(getUnassignedCommitRow(page, "Add user authentication"))
  await page.keyboard.press("s")

  const dialog = squashDialog(page)
  const editor = dialog.getByTestId("squash-message-editor")
  await expect(editor).toHaveValue("Add user authentication\n\nFix login validation bug")
  await editor.fill("Add user authentication\n\nWith the login validation")
  await dialog.getByRole("button", { name: /Squash/ }).click()

  await expect(dialog).toBeHidden()
  await expect(getUnassignedCommitRow(page, "Fix login validation bug")).toHaveCount(0)
  await expect(getUnassignedCommitRow(page, "Add user authentication")).toBeVisible()
  await expect(page.getByTestId("unassigned-commits-section").locator("[data-row-id]")).toHaveCount(1)
})
