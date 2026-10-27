// @spec spec/branch/move-commits.spec.md
import type { Locator, Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { expandBranch, findBranchRow, getBranchCommitRow, getBranchDetailsRow, getMoreButton, getUnassignedCommitRow, verifyCommitCount } from "../helpers/branch-helpers"
import { inlineBranchCreator, submitInlineForm } from "../helpers/inline-form-helpers"
import { clickContextMenuItem, clickGroupIntoBranchButton, dropAt, dropOn, multiSelectCommit, openContextMenu, selectCommit, startDrag } from "../helpers/selection-helpers"
import { finishedSyncCount, runAndWaitForSync } from "../helpers/sync-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"
import type { RewriteCall } from "../helpers/wait-helpers"
import { recordRewrites } from "../helpers/wait-helpers"

const ISSUE = "ABC-123"

/** Names of the branch rows in display order */
/** Moves "Add parser" of the issue branch to docs through Move to Existing of its context menu, and waits for the sync. */
async function moveParserToDocs(page: Page): Promise<void> {
  await expandBranch(page, findBranchRow(page, ISSUE))
  await openContextMenu(page, getBranchCommitRow(page, ISSUE, "Add parser"))
  await page.getByRole("menuitem", { name: "Move to Existing" }).hover()
  await runAndWaitForSync(page, () => clickContextMenuItem(page, "docs"))
}

/**
 * Drags "Add parser" of the issue branch onto docs while another process holds the repository lock (the test server,
 * whose record names it): the move reaches the session and waits there for the lock, the "⋯" of the issue row busy;
 * the moves the page sends from now on
 */
async function lockedParserMoveToDocs(page: Page, repo: TestRepositoryBuilder): Promise<RewriteCall[]> {
  await expandBranch(page, findBranchRow(page, ISSUE))
  const moves = await recordRewrites(page, "move")
  await repo.holdRepositoryLock(true)
  await startDrag(page, getBranchCommitRow(page, ISSUE, "Add parser"))
  await dropOn(page, findBranchRow(page, "docs"))
  await expect(getMoreButton(findBranchRow(page, ISSUE))).toHaveAttribute("aria-busy", "true")
  expect(moves).toHaveLength(1)
  return moves
}

/** Drags `row` onto `target` while the held move waits: the drop is taken, its move waits behind it in the session */
async function dropDuringMove(page: Page, row: Locator, target = "docs"): Promise<void> {
  await startDrag(page, row)
  await expect(findBranchRow(page, target)).toHaveAttribute("data-drop-target", "ready")
  await dropOn(page, findBranchRow(page, target))
}

async function branchOrder(page: Page): Promise<string[]> {
  return page.getByTestId("branch-row").evaluateAll(rows => rows.map(row => row.getAttribute("data-branch-name") ?? ""))
}

test.describe("Move commits", () => {
  let repo: TestRepositoryBuilder

  test.beforeEach(async ({ page, setupRepo, syncAndWaitForBranches }) => {
    repo = await setupRepo("issue_split")
    await syncAndWaitForBranches()
    await verifyCommitCount(findBranchRow(page, ISSUE), 3)
  })

  test("should split commits out of an issue branch into a new branch nested under the issue", async ({ page }) => {
    await expandBranch(page, findBranchRow(page, ISSUE))
    await selectCommit(getBranchCommitRow(page, ISSUE, "Add tokens"))
    await multiSelectCommit(getBranchCommitRow(page, ISSUE, "Add lexer"))
    await expect(getBranchDetailsRow(page, ISSUE).getByTestId("selection-actions")).toContainText("2 selected")

    await clickGroupIntoBranchButton(page)
    await inlineBranchCreator.waitForVisible(page)
    // the branches of the same issue and the newest ones are offered
    await expect(inlineBranchCreator.getPortal(page).getByTestId("branch-target-chips")).toContainText("docs")
    await runAndWaitForSync(page, () => submitInlineForm(inlineBranchCreator.getInput(page), "lexer"))

    const lexerRow = findBranchRow(page, "lexer")
    await verifyCommitCount(lexerRow, 2)
    await verifyCommitCount(findBranchRow(page, ISSUE), 1)
    // nested right under the issue branch
    const order = await branchOrder(page)
    expect(order.indexOf("lexer")).toBe(order.indexOf(ISSUE) + 1)
    // revealed: expanded, the moved commits selected. The issue key stays in the messages, so lexer is a branch of the
    // issue, nested under its row, and its commits leave the key to that row
    await expect(lexerRow).toHaveAttribute("data-state", "open")
    await expect(getBranchCommitRow(page, "lexer", "Add lexer")).toHaveAttribute("data-selected", "true")
    await expect(getBranchCommitRow(page, "lexer", "Add tokens")).toHaveAttribute("data-selected", "true")
    await expect(getBranchDetailsRow(page, "lexer").locator("[data-row-id]").filter({ hasText: "ABC-123" })).toHaveCount(0)
  })

  test("should move a commit to an existing branch from the context menu and undo it", async ({ page }) => {
    await moveParserToDocs(page)

    await verifyCommitCount(findBranchRow(page, "docs"), 2)
    await verifyCommitCount(findBranchRow(page, ISSUE), 2)

    await runAndWaitForSync(page, () => page.getByRole("button", { name: "Undo" }).click())
    await verifyCommitCount(findBranchRow(page, "docs"), 1)
    await verifyCommitCount(findBranchRow(page, ISSUE), 3)
  })

  test("should move a split commit back to its issue and a prefix commit to the unassigned commits", async ({ page }) => {
    await expandBranch(page, findBranchRow(page, ISSUE))
    await openContextMenu(page, getBranchCommitRow(page, ISSUE, "Add lexer"))
    await clickContextMenuItem(page, "Move to…")
    await runAndWaitForSync(page, () => submitInlineForm(inlineBranchCreator.getInput(page), "lexer"))
    await verifyCommitCount(findBranchRow(page, "lexer"), 1)

    await openContextMenu(page, getBranchCommitRow(page, "lexer", "Add lexer"))
    await runAndWaitForSync(page, () => clickContextMenuItem(page, `Move Back to ${ISSUE}`))
    await expect(findBranchRow(page, "lexer")).toHaveCount(0)
    await verifyCommitCount(findBranchRow(page, ISSUE), 3)

    await expandBranch(page, findBranchRow(page, "docs"))
    await openContextMenu(page, getBranchCommitRow(page, "docs", "Update documentation"))
    await runAndWaitForSync(page, () => clickContextMenuItem(page, "Move to Unassigned"))
    await expect(getUnassignedCommitRow(page, "Update documentation")).toBeVisible()
    await expect(findBranchRow(page, "docs")).toHaveCount(0)
  })

  test("should rename a branch and name an issue branch", async ({ page }) => {
    await openContextMenu(page, findBranchRow(page, "docs"))
    await clickContextMenuItem(page, "Rename…")
    const renameInput = page.getByTestId("inline-rename-input").locator('input[type="text"]')
    await expect(renameInput).toHaveValue("docs")
    await runAndWaitForSync(page, () => submitInlineForm(renameInput, "documentation"))
    await verifyCommitCount(findBranchRow(page, "documentation"), 1)
    await expect(findBranchRow(page, "docs")).toHaveCount(0)

    // named by the issue groups, without a rewrite; the issue is shown next to the name, as no branch is named after it any more
    await openContextMenu(page, findBranchRow(page, ISSUE))
    await clickContextMenuItem(page, "Name This Branch…")
    await runAndWaitForSync(page, () => submitInlineForm(page.getByTestId("inline-rename-input").locator('input[type="text"]'), "parser"))
    const parserRow = findBranchRow(page, "parser")
    await verifyCommitCount(parserRow, 3)
    await expect(parserRow.getByTestId("branch-issue-key")).toHaveText(ISSUE)
    await expect(findBranchRow(page, ISSUE)).toHaveCount(0)
  })

  test("should run the Undo of a move pressed while another rewrite runs after it, the toast and its Undo kept until it undid", async ({ page }) => {
    await moveParserToDocs(page)
    const moved = page.locator("[data-slot=toast]", { hasText: "Moved 1 commit to docs" })
    await expect(moved).toBeVisible()

    // a park of the issue branch waits for the repository lock another process holds
    const parks = await recordRewrites(page, "park")
    const undos = await recordRewrites(page, "undo")
    await repo.holdRepositoryLock(true)
    await startDrag(page, findBranchRow(page, ISSUE))
    await dropAt(page, page.getByTestId("drag-target-bar").getByTestId("park-drop-zone"))
    await expect(getMoreButton(findBranchRow(page, ISSUE))).toHaveAttribute("aria-busy", "true")
    expect(parks).toHaveLength(1)

    // the Undo reaches the session and waits behind the park: no refusal, the toast stays with its Undo, nothing undone
    await moved.getByRole("button", { name: "Undo" }).click()
    await expect.poll(() => undos.length).toBe(1)
    await expect(page.locator("[data-slot=toast]", { hasText: "Cannot undo the move" })).toHaveCount(0)
    await expect(moved).toBeVisible()
    await verifyCommitCount(findBranchRow(page, "docs"), 2)

    // the holder is done: the park runs, then the undo, which closes the toast
    await repo.holdRepositoryLock(false)
    await expect(page.locator("[data-slot=toast]", { hasText: "Parked ABC-123" })).toBeVisible()
    await expect(moved).toHaveCount(0)
    await verifyCommitCount(findBranchRow(page, "docs"), 1)
    await verifyCommitCount(findBranchRow(page, ISSUE), 3)
    expect(undos).toHaveLength(1)
  })

  test("should drag the selection onto a branch, but not onto its own branch", async ({ page }) => {
    await expandBranch(page, findBranchRow(page, ISSUE))
    await selectCommit(getBranchCommitRow(page, ISSUE, "Add tokens"))
    await multiSelectCommit(getBranchCommitRow(page, ISSUE, "Add parser"))

    await startDrag(page, getBranchCommitRow(page, ISSUE, "Add tokens"))
    await expect(findBranchRow(page, "docs")).toHaveAttribute("data-drop-target", "ready")
    // its own branch and the unassigned commits (no dragged commit has a prefix to remove) are not targets
    await expect(findBranchRow(page, ISSUE)).not.toHaveAttribute("data-drop-target")
    await expect(page.getByTestId("unassigned-commits-section")).not.toHaveAttribute("data-drop-target")
    await runAndWaitForSync(page, () => dropOn(page, findBranchRow(page, "docs")))

    await verifyCommitCount(findBranchRow(page, "docs"), 3)
    await verifyCommitCount(findBranchRow(page, ISSUE), 1)
    await expect(findBranchRow(page, "docs")).not.toHaveAttribute("data-drop-target")
  })

  test("should take a second drag while the first move runs, wait for it in the queue and move the commit as the first move left it", async ({ page }) => {
    // the first move gives the oldest commit a prefix: every commit after it gets another id
    const moves = await lockedParserMoveToDocs(page, repo)

    // a drop while it runs is taken: no refusal, its move waits in the session behind the first one, the row says so
    await dropDuringMove(page, getUnassignedCommitRow(page, "Refactor build script"))
    const more = getMoreButton(findBranchRow(page, "docs"))
    await expect(more).toHaveAttribute("aria-busy", "true")
    await more.hover()
    await expect(page.locator("[data-slot=tooltip-content][data-open]", { hasText: "Waits for Moving 1 commit to docs…" })).toBeVisible()
    await expect.poll(() => moves.length).toBe(2)

    // the holder is done: the second move runs beside the sync of the first, with the id the first move gave the commit
    // (the old id would fail with "Cannot move the commits to docs"), and the two syncs fold into two runs at most
    const syncs = await finishedSyncCount(page)
    await repo.holdRepositoryLock(false)
    await verifyCommitCount(findBranchRow(page, "docs"), 3)
    await verifyCommitCount(findBranchRow(page, ISSUE), 2)
    await expect(page.getByTestId("unassigned-commits-section")).toHaveCount(0)
    await expect(more).not.toHaveAttribute("aria-busy")
    expect(await finishedSyncCount(page)).toBeLessThanOrEqual(syncs + 2)
    await expect(page.locator("[data-slot=toast]", { hasText: /^Cannot move/ })).toHaveCount(0)
  })

  test("should move a second commit of the same branch dragged onto the same target while the first move runs: a move of its own, never joined to the first", async ({ page }) => {
    const moves = await lockedParserMoveToDocs(page, repo)
    // the same action of the same subject (the source branch), but other commits: it waits behind the first one
    await dropDuringMove(page, getBranchCommitRow(page, ISSUE, "Add lexer"))
    await expect.poll(() => moves.length).toBe(2)

    // both commits reached docs, each in a move of its own
    await repo.holdRepositoryLock(false)
    await verifyCommitCount(findBranchRow(page, "docs"), 3)
    await verifyCommitCount(findBranchRow(page, ISSUE), 1)
    await expect(page.locator("[data-slot=toast]", { hasText: /^Cannot move/ })).toHaveCount(0)
  })

  test("should move two drags into different branches, the first waiting for the repository lock, both in place", async ({ page }) => {
    const moves = await lockedParserMoveToDocs(page, repo)
    await dropDuringMove(page, getUnassignedCommitRow(page, "Refactor build script"), ISSUE)
    await expect.poll(() => moves.map(move => move.rewrite.targetBranch)).toEqual(["docs", ISSUE])

    await repo.holdRepositoryLock(false)
    await verifyCommitCount(findBranchRow(page, "docs"), 2)
    await verifyCommitCount(findBranchRow(page, ISSUE), 3)
    await expect(getBranchCommitRow(page, ISSUE, "Refactor build script")).toBeVisible()
    await expect(page.getByTestId("unassigned-commits-section")).toHaveCount(0)
    await expect(page.locator("[data-slot=toast]", { hasText: /^Cannot move/ })).toHaveCount(0)
  })

  test("should drag an unassigned commit onto the new branch zone", async ({ page }) => {
    await startDrag(page, getUnassignedCommitRow(page, "Refactor build script"))
    const zone = page.getByTestId("new-branch-drop-zone")
    await expect(zone).toBeVisible()
    await dropOn(page, zone)

    await inlineBranchCreator.waitForVisible(page)
    await runAndWaitForSync(page, () => submitInlineForm(inlineBranchCreator.getInput(page), "build"))
    await verifyCommitCount(findBranchRow(page, "build"), 1)
    await expect(page.getByTestId("unassigned-commits-section")).toHaveCount(0)
  })
})
