// @spec openspec/specs/branch/move-commits/spec.md
import type { Locator, Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { activityButton, activityEntry, activityPanel } from "../helpers/activity-helpers"
import { expandBranch, findBranchRow, getBranchCommitRow, getBranchDetailsRow, getMoreButton, getUnassignedCommitRow, verifyCommitCount } from "../helpers/branch-helpers"
import { inlineBranchCreator, submitInlineForm } from "../helpers/inline-form-helpers"
import {
  clickContextMenuItem,
  clickGroupIntoBranchButton,
  dropAt,
  dropOn,
  multiSelectCommit,
  openContextMenu,
  selectCommit,
  shiftSelectCommit,
  startDrag,
} from "../helpers/selection-helpers"
import { finishedSyncCount, pressSyncHeld, runAndWaitForSync, syncFromMenu } from "../helpers/sync-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"
import { closeToast } from "../helpers/toast-helpers"
import type { RewriteCall } from "../helpers/wait-helpers"
import { recordRewrites, expectClosed } from "../helpers/wait-helpers"

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

/** Moves "Add parser" to docs: its toast, with the Undo, and the undos the page sends from now on */
async function movedWithUndo(page: Page): Promise<{ moved: Locator; undos: RewriteCall[] }> {
  await moveParserToDocs(page)
  const moved = page.locator("[data-slot=toast]", { hasText: "Moved 1 commit to docs" })
  await expect(moved).toBeVisible()
  return { moved, undos: await recordRewrites(page, "undo") }
}

/** Presses the Undo of `moved` while something holds it back: it reaches the session, no refusal, the toast stays */
async function pressUndoThatWaits(page: Page, moved: Locator, undos: RewriteCall[]): Promise<void> {
  await moved.getByRole("button", { name: "Undo" }).click()
  await expect.poll(() => undos.length).toBe(1)
  await expect(page.locator("[data-slot=toast]", { hasText: "Cannot undo the move" })).toHaveCount(0)
  await expect(moved).toBeVisible()
}

/** The move was undone once: "Add parser" is back in the issue branch, and its toast closed */
async function expectUndoneOnce(page: Page, moved: Locator, undos: RewriteCall[]): Promise<void> {
  await verifyCommitCount(findBranchRow(page, "docs"), 1)
  await verifyCommitCount(findBranchRow(page, ISSUE), 3)
  await expectClosed(moved)
  expect(undos).toHaveLength(1)
}

/** Renames docs to documentation through Rename… of its context menu, and waits for the sync */
async function renameDocs(page: Page): Promise<void> {
  await openContextMenu(page, findBranchRow(page, "docs"))
  await clickContextMenuItem(page, "Rename…")
  const renameInput = page.getByTestId("inline-rename-input").locator('input[type="text"]')
  await expect(renameInput).toHaveValue("docs")
  await runAndWaitForSync(page, () => submitInlineForm(renameInput, "documentation"))
}

/** The toast of a rewrite and its entry in Activity, each with the Undo of the rewrite */
interface UndoSurfaces {
  readonly toast: Locator
  readonly entry: Locator
}

/** The toast still tells the rewrite, without its Undo, and its entry has none, undone or not */
async function expectUndoTakenBack({ toast, entry }: UndoSurfaces): Promise<void> {
  await expect(toast).toBeVisible()
  await expect(toast.getByRole("button", { name: "Undo" })).toHaveCount(0)
  await expect(entry.getByTestId("activity-undo")).toHaveCount(0)
  await expect(entry.getByTestId("activity-undone")).toHaveCount(0)
}

/** How far the top of `element` is below the top of `anchor` */
async function offsetBelow(element: Locator, anchor: Locator): Promise<number> {
  // both asked at once: one round trip to the page, which under load answers each in a quarter of a second
  const [elementBox, anchorBox] = await Promise.all([element.boundingBox(), anchor.boundingBox()])
  expect(elementBox).not.toBeNull()
  expect(anchorBox).not.toBeNull()
  return elementBox!.y - anchorBox!.y
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
    // the way back names the issue branch; with the selection cleared, no row names the key
    await expect(getBranchDetailsRow(page, "lexer").getByTestId("selection-actions")).toContainText("Move Back to ABC-123")
    await getBranchCommitRow(page, "lexer", "Add lexer").click({ modifiers: ["ControlOrMeta"] })
    await getBranchCommitRow(page, "lexer", "Add tokens").click({ modifiers: ["ControlOrMeta"] })
    await expect(getBranchDetailsRow(page, "lexer").getByTestId("selection-actions")).toHaveCount(0)
    await expect(getBranchDetailsRow(page, "lexer").locator("[data-row-id]").filter({ hasText: "ABC-123" })).toHaveCount(0)
  })

  test("should keep the rows in place as commits are selected and deselected", async ({ page }) => {
    const issueRow = findBranchRow(page, ISSUE)
    await expandBranch(page, issueRow)
    const [tokens, lexer, parser] = ["Add tokens", "Add lexer", "Add parser"].map(subject => getBranchCommitRow(page, ISSUE, subject))
    const unassignedCard = page.getByTestId("unassigned-commits-section")
    const unassignedCommit = getUnassignedCommitRow(page, "Refactor build script")
    // no line above the commits: the first one sits right below the row of its branch
    const issueRowBox = await issueRow.boundingBox()
    expect(await offsetBelow(tokens!, issueRow)).toBeLessThan(issueRowBox!.height + 8)
    // from the row of the branch and the top of the card: a scroll moves them alike
    const offsets = () => Promise.all([offsetBelow(lexer!, issueRow), offsetBelow(parser!, issueRow), offsetBelow(unassignedCommit, unassignedCard)])
    const idle = await offsets()

    // the actions on the meta line of the row clicked last, the lead of the selection
    await selectCommit(tokens!)
    await expect(tokens!.getByTestId("selection-actions")).toContainText("1 selected")
    expect(await offsets()).toEqual(idle)
    await shiftSelectCommit(parser!)
    await expect(parser!.getByTestId("selection-actions")).toContainText("3 selected")
    await expect(tokens!.getByTestId("selection-actions")).toHaveCount(0)
    expect(await offsets()).toEqual(idle)

    // its press keeps the selection, and the form opens right below the lead, the count staying on its row
    await parser!.getByRole("button", { name: "Move to…" }).click()
    await inlineBranchCreator.waitForVisible(page)
    await expect(parser!.getByTestId("selection-actions")).toHaveText("3 selected")
    expect(await offsetBelow(inlineBranchCreator.getPortal(page), parser!)).toBeGreaterThan(0)
    await inlineBranchCreator.getCancelButton(page).click()
    await inlineBranchCreator.waitForHidden(page)
    expect(await offsets()).toEqual(idle)

    // a Cmd/Ctrl click takes the lead out: the actions go to the selected row nearest to it
    await parser!.click({ modifiers: ["ControlOrMeta"] })
    await expect(parser!).toHaveAttribute("data-selected", "false")
    await expect(lexer!.getByTestId("selection-actions")).toContainText("2 selected")
    await lexer!.click({ modifiers: ["ControlOrMeta"] })
    await tokens!.click({ modifiers: ["ControlOrMeta"] })
    await expect(tokens!).toHaveAttribute("data-selected", "false")
    await expect(getBranchDetailsRow(page, ISSUE).getByTestId("selection-actions")).toHaveCount(0)
    expect(await offsets()).toEqual(idle)

    // in the unassigned card as in a branch: on the row, the form below it
    await selectCommit(unassignedCommit)
    const unassignedActions = unassignedCommit.getByTestId("selection-actions")
    await expect(unassignedActions).toContainText("1 selected")
    expect(await offsets()).toEqual(idle)
    await unassignedActions.getByRole("button", { name: "Move to…" }).click()
    await inlineBranchCreator.waitForVisible(page)
    await expect(unassignedActions).toHaveText("1 selected")
    expect(await offsetBelow(inlineBranchCreator.getPortal(page), unassignedCommit)).toBeGreaterThan(0)
    await inlineBranchCreator.getCancelButton(page).click()
    await inlineBranchCreator.waitForHidden(page)
    await expect(unassignedActions.getByRole("button", { name: "Move to…" })).toBeVisible()
  })

  // WebKit keeps the focus on an element that went away: Escape must give it back to the list there too
  test("should leave the keys of the selection's actions and of its form to them", { tag: "@webkit" }, async ({ page }) => {
    await expandBranch(page, findBranchRow(page, ISSUE))
    const tokens = getBranchCommitRow(page, ISSUE, "Add tokens")
    const lexer = getBranchCommitRow(page, ISSUE, "Add lexer")
    await selectCommit(tokens)
    await multiSelectCommit(lexer)
    const actions = lexer.getByTestId("selection-actions")

    // Enter presses the focused button: Squash opens the squash, not the "Move to…" form of the list's Enter
    const squash = actions.getByRole("button", { name: "Squash" })
    await squash.focus()
    await page.keyboard.press("Enter")
    const dialog = page.getByTestId("squash-dialog")
    await expect(dialog).toBeVisible()
    await expect(inlineBranchCreator.getPortal(page)).toHaveCount(0)
    await expect(dialog.getByTestId("squash-message-editor")).toBeFocused()
    await page.keyboard.press("Escape")
    await expectClosed(dialog)
    await expect(actions).toContainText("2 selected")

    // "m" typed into the name of the form is a letter of the name
    const moveTo = actions.getByRole("button", { name: "Move to…" })
    await moveTo.click()
    const input = inlineBranchCreator.getInput(page)
    await expect(input).toBeFocused()
    await page.keyboard.type("mm")
    await expect(input).toHaveValue("mm")
    await inlineBranchCreator.getCancelButton(page).click()
    await inlineBranchCreator.waitForHidden(page)

    // Escape on a button of the actions clears the selection, and the focus goes back to the list
    await moveTo.focus()
    await page.keyboard.press("Escape")
    await expect(lexer).toHaveAttribute("data-selected", "false")
    await expect(tokens).toHaveAttribute("data-selected", "false")
    await expect(getBranchDetailsRow(page, ISSUE).getByRole("table")).toBeFocused()
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
    // a move back that only removed a prefix says so once, in the title
    const removed = page.getByRole("dialog", { name: "Removed the prefix of 1 commit", exact: true })
    await expect(removed).toHaveAccessibleDescription("")
    await closeToast(removed)

    await expandBranch(page, findBranchRow(page, "docs"))
    await openContextMenu(page, getBranchCommitRow(page, "docs", "Update documentation"))
    await runAndWaitForSync(page, () => clickContextMenuItem(page, "Move to Unassigned"))
    await expect(getUnassignedCommitRow(page, "Update documentation")).toBeVisible()
    await expect(findBranchRow(page, "docs")).toHaveCount(0)
    await expect(removed).toHaveAccessibleDescription("")
  })

  test("should rename a branch and name an issue branch", async ({ page }) => {
    await renameDocs(page)
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

  /** The move of "Add parser" to docs, its toast and its entry in the side panel of Activity, both with their Undo */
  async function movedWithUndoInActivity(page: Page): Promise<UndoSurfaces> {
    await activityButton(page).click()
    await moveParserToDocs(page)
    const toast = page.locator("[data-slot=toast]", { hasText: "Moved 1 commit to docs" })
    const entry = activityEntry(activityPanel(page), "Moved 1 commit to docs")
    await expect(toast.getByRole("button", { name: "Undo" })).toBeVisible()
    await expect(entry.getByTestId("activity-undo")).toBeVisible()
    return { toast, entry }
  }

  test("should take the Undo of a move back from its toast and Activity once a rename ran after it, whose own Undo works", async ({ page }) => {
    const moved = await movedWithUndoInActivity(page)

    // a rename of docs moves HEAD on: the Undo of the move could only fail now, the rename's takes its place
    await renameDocs(page)
    const renamed = page.locator("[data-slot=toast]", { hasText: "Renamed docs to documentation (2 commits)" })
    await expect(renamed.getByRole("button", { name: "Undo" })).toBeVisible()
    await expectUndoTakenBack(moved)
    await expect(activityEntry(activityPanel(page), "Renamed docs to documentation (2 commits)").getByTestId("activity-undo")).toBeVisible()

    await runAndWaitForSync(page, () => renamed.getByRole("button", { name: "Undo" }).click())
    await verifyCommitCount(findBranchRow(page, "docs"), 2)
    await expect(findBranchRow(page, "documentation")).toHaveCount(0)
  })

  test("should take the Undo of a move back from its toast and Activity once a sync read a commit made outside the app", async ({ page }) => {
    const moved = await movedWithUndoInActivity(page)

    // a commit in the IDE: the session knows nothing of it until a sync reads it
    await repo.commit("Note the parser", { "parser.md": "parser\n" }, 1704182460)
    await expect(moved.toast.getByRole("button", { name: "Undo" })).toBeVisible()
    await syncFromMenu(page)
    await expectUndoTakenBack(moved)
    await verifyCommitCount(findBranchRow(page, "docs"), 2)
  })

  test("should run the Undo of a move pressed while another rewrite runs after it, the toast and its Undo kept until it undid", async ({ page }) => {
    const { moved, undos } = await movedWithUndo(page)
    // a park of the issue branch waits for the repository lock another process holds
    const parks = await recordRewrites(page, "park")
    await repo.holdRepositoryLock(true)
    await startDrag(page, findBranchRow(page, ISSUE))
    await dropAt(page, page.getByTestId("drag-target-bar").getByTestId("park-drop-zone"))
    await expect(getMoreButton(findBranchRow(page, ISSUE))).toHaveAttribute("aria-busy", "true")
    expect(parks).toHaveLength(1)

    // the Undo reaches the session and waits behind the park: no refusal, the toast stays with its Undo, nothing undone
    await pressUndoThatWaits(page, moved, undos)
    await verifyCommitCount(findBranchRow(page, "docs"), 2)

    // the holder is done: the park runs, then the undo, which closes the toast
    await runAndWaitForSync(page, () => repo.holdRepositoryLock(false))
    await expect(page.locator("[data-slot=toast]", { hasText: "Parked ABC-123" })).toBeVisible()
    await expectUndoneOnce(page, moved, undos)
  })

  test("should undo a move pressed during a held sync once that sync ended, without a refusal, one sync for the undo", async ({ page }) => {
    const { moved, undos } = await movedWithUndo(page)
    const syncs = await pressSyncHeld(page, repo)
    // the Undo goes to the session at once, never refused; its answer waits for the sync it asked, after the held one,
    // so the toast stays with its Undo
    await pressUndoThatWaits(page, moved, undos)

    await repo.holdRemoteAnswers({ release: ["sync"] })
    await expectUndoneOnce(page, moved, undos)
    // the held sync and the one the undo asked
    await expect.poll(() => finishedSyncCount(page)).toBe(syncs + 2)
    await expect(page.locator("[data-slot=toast]", { hasText: /^Cannot / })).toHaveCount(0)
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
    await expect(page.locator("[data-slot=tooltip-content][data-open]", { hasText: "Queued behind Moving 1 commit to docs…" })).toBeVisible()
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
