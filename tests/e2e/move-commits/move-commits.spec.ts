// @spec spec/branch/move-commits.spec.md
import type { Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { expandBranch, findBranchRow, getBranchCommitRow, getBranchDetailsRow, getUnassignedCommitRow, verifyCommitCount } from "../helpers/branch-helpers"
import { inlineBranchCreator, submitInlineForm } from "../helpers/inline-form-helpers"
import { clickContextMenuItem, clickGroupIntoBranchButton, dropOn, multiSelectCommit, openContextMenu, selectCommit, startDrag } from "../helpers/selection-helpers"
import { runAndWaitForSync } from "../helpers/sync-helpers"

const ISSUE = "ABC-123"

/** Names of the branch rows in display order */
async function branchOrder(page: Page): Promise<string[]> {
  return page.getByTestId("branch-row").evaluateAll(rows => rows.map(row => row.getAttribute("data-branch-name") ?? ""))
}

test.describe("Move commits", () => {
  test.beforeEach(async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("issue_split")
    await syncAndWaitForBranches()
    await verifyCommitCount(findBranchRow(page, ISSUE), "3 commits")
  })

  test("should split commits out of an issue branch into a new branch nested under the issue", async ({ page }) => {
    await expandBranch(page, findBranchRow(page, ISSUE))
    await selectCommit(page, getBranchCommitRow(page, ISSUE, "Add tokens"))
    await multiSelectCommit(page, getBranchCommitRow(page, ISSUE, "Add lexer"))
    await expect(getBranchDetailsRow(page, ISSUE).getByTestId("selection-actions")).toContainText("2 selected")

    await clickGroupIntoBranchButton(page)
    await inlineBranchCreator.waitForVisible(page)
    // the branches of the same issue and the newest ones are offered
    await expect(inlineBranchCreator.getPortal(page).getByTestId("branch-target-chips")).toContainText("docs")
    await runAndWaitForSync(page, () => submitInlineForm(inlineBranchCreator.getInput(page), "lexer"))

    const lexerRow = findBranchRow(page, "lexer")
    await verifyCommitCount(lexerRow, "2 commits")
    await verifyCommitCount(findBranchRow(page, ISSUE), "1 commit")
    // nested right under the issue branch
    const order = await branchOrder(page)
    expect(order.indexOf("lexer")).toBe(order.indexOf(ISSUE) + 1)
    // revealed: expanded, the moved commits selected; the issue key stays in the messages
    await expect(lexerRow).toHaveAttribute("data-state", "open")
    await expect(getBranchCommitRow(page, "lexer", "ABC-123: Add lexer")).toHaveAttribute("data-selected", "true")
    await expect(getBranchCommitRow(page, "lexer", "ABC-123: Add tokens")).toHaveAttribute("data-selected", "true")
  })

  test("should move a commit to an existing branch from the context menu and undo it", async ({ page }) => {
    await expandBranch(page, findBranchRow(page, ISSUE))
    await openContextMenu(page, getBranchCommitRow(page, ISSUE, "Add parser"))
    await page.getByRole("menuitem", { name: "Move to Existing" }).hover()
    await runAndWaitForSync(page, () => clickContextMenuItem(page, "docs"))

    await verifyCommitCount(findBranchRow(page, "docs"), "2 commits")
    await verifyCommitCount(findBranchRow(page, ISSUE), "2 commits")

    await runAndWaitForSync(page, () => page.getByRole("button", { name: "Undo" }).click())
    await verifyCommitCount(findBranchRow(page, "docs"), "1 commit")
    await verifyCommitCount(findBranchRow(page, ISSUE), "3 commits")
  })

  test("should move a split commit back to its issue and a prefix commit to the unassigned commits", async ({ page }) => {
    await expandBranch(page, findBranchRow(page, ISSUE))
    await openContextMenu(page, getBranchCommitRow(page, ISSUE, "Add lexer"))
    await clickContextMenuItem(page, "Move to…")
    await runAndWaitForSync(page, () => submitInlineForm(inlineBranchCreator.getInput(page), "lexer"))
    await verifyCommitCount(findBranchRow(page, "lexer"), "1 commit")

    await openContextMenu(page, getBranchCommitRow(page, "lexer", "Add lexer"))
    await runAndWaitForSync(page, () => clickContextMenuItem(page, `Move Back to ${ISSUE}`))
    await expect(findBranchRow(page, "lexer")).toHaveCount(0)
    await verifyCommitCount(findBranchRow(page, ISSUE), "3 commits")

    await expandBranch(page, findBranchRow(page, "docs"))
    await openContextMenu(page, getBranchCommitRow(page, "docs", "Update documentation"))
    await runAndWaitForSync(page, () => clickContextMenuItem(page, "Move to Unassigned"))
    await expect(getUnassignedCommitRow(page, "Update documentation")).toBeVisible()
    await expect(findBranchRow(page, "docs")).toHaveCount(0)
  })

  test("should rename a branch and name an issue branch", async ({ page }) => {
    await openContextMenu(page, findBranchRow(page, "docs"))
    await clickContextMenuItem(page, "Rename…")
    const renameInput = page.getByTestId("inline-rename-input").locator("input[type=\"text\"]")
    await expect(renameInput).toHaveValue("docs")
    await runAndWaitForSync(page, () => submitInlineForm(renameInput, "documentation"))
    await verifyCommitCount(findBranchRow(page, "documentation"), "1 commit")
    await expect(findBranchRow(page, "docs")).toHaveCount(0)

    // named by the issue groups, without a rewrite; the issue is shown next to the name, as no branch is named after it any more
    await openContextMenu(page, findBranchRow(page, ISSUE))
    await clickContextMenuItem(page, "Name This Branch…")
    await runAndWaitForSync(page, () => submitInlineForm(page.getByTestId("inline-rename-input").locator("input[type=\"text\"]"), "parser"))
    const parserRow = findBranchRow(page, "parser")
    await verifyCommitCount(parserRow, "3 commits")
    await expect(parserRow.getByTestId("branch-issue-key")).toHaveText(ISSUE)
    await expect(findBranchRow(page, ISSUE)).toHaveCount(0)
  })

  test("should drag the selection onto a branch, but not onto its own branch", async ({ page }) => {
    await expandBranch(page, findBranchRow(page, ISSUE))
    await selectCommit(page, getBranchCommitRow(page, ISSUE, "Add tokens"))
    await multiSelectCommit(page, getBranchCommitRow(page, ISSUE, "Add parser"))

    await startDrag(page, getBranchCommitRow(page, ISSUE, "Add tokens"))
    await expect(findBranchRow(page, "docs")).toHaveAttribute("data-drop-target", "ready")
    // its own branch and the unassigned commits (no dragged commit has a prefix to remove) are not targets
    await expect(findBranchRow(page, ISSUE)).not.toHaveAttribute("data-drop-target")
    await expect(page.getByTestId("unassigned-commits-section")).not.toHaveAttribute("data-drop-target")
    await runAndWaitForSync(page, () => dropOn(page, findBranchRow(page, "docs")))

    await verifyCommitCount(findBranchRow(page, "docs"), "3 commits")
    await verifyCommitCount(findBranchRow(page, ISSUE), "1 commit")
    await expect(findBranchRow(page, "docs")).not.toHaveAttribute("data-drop-target")
  })

  test("should drag an unassigned commit onto the new branch zone", async ({ page }) => {
    await startDrag(page, getUnassignedCommitRow(page, "Refactor build script"))
    const zone = page.getByTestId("new-branch-drop-zone")
    await expect(zone).toBeVisible()
    await dropOn(page, zone)

    await inlineBranchCreator.waitForVisible(page)
    await runAndWaitForSync(page, () => submitInlineForm(inlineBranchCreator.getInput(page), "build"))
    await verifyCommitCount(findBranchRow(page, "build"), "1 commit")
    await expect(page.getByTestId("unassigned-commits-section")).toHaveCount(0)
  })
})
