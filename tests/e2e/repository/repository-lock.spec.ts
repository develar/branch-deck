// @spec spec/branch/move-commits.spec.md
import type { Locator, Page } from "@playwright/test"
import { expect, test } from "../fixtures/test-fixtures"
import { expandBranch, findBranchRow, getBranchCommitRow, getBranchDetailsRow, getMoreButton, getPushButton, verifyCommitCount } from "../helpers/branch-helpers"
import { clickContextMenuItem, multiSelectCommit, openContextMenu, selectCommit } from "../helpers/selection-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"
import { recordCommand, recordRewrites } from "../helpers/wait-helpers"

const ISSUE = "ABC-123"

/** Whom a rewrite or a push waits for while another process holds the repository lock: here the test server */
const WAITING = /^Waiting for `[^`]+` \(pid \d+, for \d+ s\)/

/** The `issue_split` repository synced, its issue branch expanded */
async function setup(page: Page, setupRepo: (template: string) => Promise<TestRepositoryBuilder>, syncAndWaitForBranches: () => Promise<unknown>): Promise<TestRepositoryBuilder> {
  const repo = await setupRepo("issue_split")
  await syncAndWaitForBranches()
  await verifyCommitCount(findBranchRow(page, ISSUE), 3)
  await expandBranch(page, findBranchRow(page, ISSUE))
  return repo
}

/** Opens the squash dialog of "Add tokens" and "Add parser" and presses Squash; the dialog */
async function squashTwo(page: Page): Promise<Locator> {
  await selectCommit(getBranchCommitRow(page, ISSUE, "Add tokens"))
  await multiSelectCommit(getBranchCommitRow(page, ISSUE, "Add parser"))
  await getBranchDetailsRow(page, ISSUE).getByTestId("selection-actions").getByRole("button", { name: "Squash" }).click()
  const dialog = page.getByTestId("squash-dialog")
  await expect(dialog.getByTestId("squash-message-editor")).toBeFocused()
  await page.keyboard.press("ControlOrMeta+Enter")
  return dialog
}

/** Moves "Add parser" of the issue branch to docs through Move to Existing of its context menu */
async function moveParserToDocsFromMenu(page: Page): Promise<void> {
  await openContextMenu(page, getBranchCommitRow(page, ISSUE, "Add parser"))
  await page.getByRole("menuitem", { name: "Move to Existing" }).hover()
  await clickContextMenuItem(page, "docs")
}

test.describe("Rewrites while another process holds the repository lock", () => {
  test("should wait for the holder in the squash dialog, say whom for, and squash once it released the lock", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setup(page, setupRepo, syncAndWaitForBranches)
    const squashes = await recordRewrites(page, "squash")
    // another process holds the lock (`bd pick --follow` of an agent): here the test server, whose record names it
    await repo.holdRepositoryLock(true)
    const dialog = await squashTwo(page)

    // the squash reached the backend and waits there, without a bound while the holder lives; the dialog says whom for
    const notice = dialog.getByTestId("lock-wait-notice")
    await expect(notice).toContainText(WAITING)
    await expect(notice.getByRole("button", { name: "Stop Waiting" })).toBeVisible()
    expect(squashes).toHaveLength(1)
    await verifyCommitCount(findBranchRow(page, ISSUE), 3)
    await expect(page.getByText("Cannot squash the commits")).toHaveCount(0)

    await repo.holdRepositoryLock(false)
    await expect(page.getByText("Squashed 2 commits")).toBeVisible()
    await expect(dialog).toBeHidden()
    await verifyCommitCount(findBranchRow(page, ISSUE), 2)
  })

  test("should stop the wait of a squash with Stop Waiting in its dialog, squash nothing, and squash on the next press", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setup(page, setupRepo, syncAndWaitForBranches)
    await repo.holdRepositoryLock(true)
    const dialog = await squashTwo(page)
    const notice = dialog.getByTestId("lock-wait-notice")
    await expect(notice).toContainText(WAITING)

    await notice.getByRole("button", { name: "Stop Waiting" }).click()
    await expect(page.getByText("Canceled squashing 2 commits before it started")).toBeVisible()
    await expect(notice).toHaveCount(0)
    await expect(page.getByText("Cannot squash the commits")).toHaveCount(0)
    // nothing changed, and the dialog stays with its message: the next press squashes, once the holder is done
    await verifyCommitCount(findBranchRow(page, ISSUE), 3)
    await repo.holdRepositoryLock(false)
    await dialog.getByRole("button", { name: /Squash/ }).click()
    await expect(page.getByText("Squashed 2 commits")).toBeVisible()
    await verifyCommitCount(findBranchRow(page, ISSUE), 2)
  })

  test("should show the wait of a move started from a menu on the row's ⋯, whose menu stops it, nothing moved", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setup(page, setupRepo, syncAndWaitForBranches)
    const moves = await recordRewrites(page, "move")
    await repo.holdRepositoryLock(true)
    await moveParserToDocsFromMenu(page)

    // a start from a menu shows on the "⋯" of its row: the spinner, whom it waits for on hover, Stop Waiting on a click
    const more = getMoreButton(findBranchRow(page, ISSUE))
    await expect(more).toHaveAttribute("aria-busy", "true")
    await more.hover()
    await expect(page.getByRole("tooltip")).toHaveText(WAITING)
    expect(moves).toHaveLength(1)
    const cancels = await recordCommand(page, "cancel_operation")
    await more.click()
    await page.getByRole("menuitem", { name: "Stop Waiting" }).click()
    await expect(page.getByText("Canceled moving 1 commit to docs before it started")).toBeVisible()
    // the stop is the cancel of the operation of the session
    expect(cancels).toHaveLength(1)
    await expect(more).not.toHaveAttribute("aria-busy", "true")
    await verifyCommitCount(findBranchRow(page, "docs"), 1)
    await verifyCommitCount(findBranchRow(page, ISSUE), 3)

    // the same move once the holder is done: it runs at once
    await repo.holdRepositoryLock(false)
    await moveParserToDocsFromMenu(page)
    await verifyCommitCount(findBranchRow(page, "docs"), 2)
    await verifyCommitCount(findBranchRow(page, ISSUE), 2)
    expect(moves).toHaveLength(2)
  })

  test("should say whom a push waits for in the hover of its button, whose Stop Waiting pushes nothing", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("issue_links")
    await syncAndWaitForBranches()
    const pushes = await recordCommand(page, "execute")
    await repo.holdRepositoryLock(true)
    const push = getPushButton(findBranchRow(page, "feature-auth"))
    await push.click()
    await expect(push).toHaveAttribute("aria-busy", "true")
    // the hover of the push says whom it waits for and offers Stop Waiting
    await push.hover()
    const description = page.getByTestId("push-description")
    await expect(description.getByTestId("push-progress")).toContainText(WAITING)
    await description.getByRole("button", { name: "Stop Waiting" }).click()
    await expect(page.getByText("Canceled the push of branch feature-auth before it started")).toBeVisible()
    await expect(push).not.toHaveAttribute("aria-busy", "true")
    expect(pushes).toHaveLength(1)
    await expect(page.getByText("Cannot push branch feature-auth")).toHaveCount(0)
    // nothing was pushed: the button offers the same push
    await expect(push).toBeEnabled()
    await repo.holdRepositoryLock(false)
  })
})
