// @spec spec/branch/branch-preview.spec.md
import type { Locator, Page } from "@playwright/test"
import { testWithConflicts, testWithIssueLinks as test, expect } from "../fixtures/test-fixtures"
import { findBranchRow, expandBranch, getBranchDetailsRow, verifyBranchState } from "../helpers/branch-helpers"
import { confirmForcePush, expectPushedToast, routePush } from "../helpers/push-helpers"
import { getOpenedSubWindows } from "../helpers/sub-window-helpers"
import { enableAutoSyncOnFocus, syncFromMenu, syncOnWindowFocus } from "../helpers/sync-helpers"

// the `issue_links` template: JIRA-456 is the newest branch, its newest commit has a body
const BRANCH = "JIRA-456"
const NEXT_BRANCH = "ABC-123"
const BRANCH_COUNT = 7

function getPreview(page: Page): Locator {
  return page.getByTestId("branch-preview")
}

/** The preview takes the focus a frame after it opens; its keys only work from then on. */
async function expectPreviewFocused(preview: Locator) {
  await expect(preview.getByTestId("branch-preview-body")).toBeFocused()
}

async function openPreviewOfHoveredBranch(page: Page, name: string): Promise<Locator> {
  await findBranchRow(page, name).hover()
  await page.keyboard.press("Space")
  const preview = getPreview(page)
  await expectPreviewFocused(preview)
  return preview
}

test.describe("Branch preview", () => {
  test("should open the preview of the hovered branch with Space and step through the branches with the arrows", async ({ page }) => {
    const preview = await openPreviewOfHoveredBranch(page, BRANCH)
    const title = preview.getByRole("heading")
    const position = preview.getByTestId("branch-preview-position")
    await expect(title).toHaveText(BRANCH)
    await expect(position).toHaveText(`1 / ${BRANCH_COUNT}`)
    // the row stays as it was: Space does not toggle it
    await verifyBranchState(findBranchRow(page, BRANCH), "closed")

    await page.keyboard.press("ArrowDown")
    await expect(title).toHaveText(NEXT_BRANCH)
    await expect(position).toHaveText(`2 / ${BRANCH_COUNT}`)
    await page.keyboard.press("ArrowUp")
    await page.keyboard.press("ArrowUp")
    await expect(title).toHaveText(BRANCH)
    await expect(position).toHaveText(`1 / ${BRANCH_COUNT}`)

    await page.keyboard.press("Space")
    await expect(preview).toHaveCount(0)
  })

  test("should give the focus to the row of the branch it showed last, so D and Space go on from there", async ({ page }) => {
    const preview = await openPreviewOfHoveredBranch(page, BRANCH)
    await page.keyboard.press("ArrowDown")
    await expect(preview.getByRole("heading")).toHaveText(NEXT_BRANCH)
    // Escape: the row of the branch it showed, not of the one it was opened on
    await page.keyboard.press("Escape")
    await expect(preview).toBeHidden()
    await expect(findBranchRow(page, NEXT_BRANCH)).toBeFocused()
    // the focused row is what D reviews, though the pointer is still over the first one
    await page.keyboard.press("d")
    await expect.poll(async () => (await getOpenedSubWindows(page)).map(it => it.title)).toEqual([`Review ${NEXT_BRANCH}`])

    // D in the preview after ↑: the window of the branch shown, the focus on its row
    await page.keyboard.press("Space")
    await expectPreviewFocused(preview)
    await page.keyboard.press("ArrowUp")
    await expect(preview.getByRole("heading")).toHaveText(BRANCH)
    await page.keyboard.press("d")
    await expect.poll(async () => (await getOpenedSubWindows(page)).map(it => it.title)).toEqual([`Review ${NEXT_BRANCH}`, `Review ${BRANCH}`])
    await expect(preview).toBeHidden()
    await expect(findBranchRow(page, BRANCH)).toBeFocused()
  })

  test("should show the branch header and its commits newest first with their messages", async ({ page }) => {
    const preview = await openPreviewOfHoveredBranch(page, BRANCH)
    // the summary is the latest commit subject, as in the row
    await expect(preview.getByText("Add connection retry logic").first()).toBeVisible()
    const meta = preview.getByTestId("branch-preview-status").locator("..")
    await expect(meta).toContainText("3 commits")
    await expect(preview.getByTestId("branch-preview-status")).toHaveText("not pushed")

    const commits = preview.getByTestId("branch-preview-commit")
    await expect(commits).toHaveCount(3)
    await expect(commits.nth(0)).toContainText("Add connection retry logic")
    await expect(commits.nth(1)).toContainText("perf db: increase pool size to 50")
    await expect(commits.nth(2)).toContainText("Fix database connection pool timeout")
    // the body reads as in the commit preview; a commit without one shows its subject only
    await expect(commits.nth(0).getByTestId("commit-message-body")).toContainText("The pool gave up after the first failed connection attempt")
    await expect(commits.nth(1).getByTestId("commit-message-body")).toHaveCount(0)
    await expect(preview.getByTestId("branch-preview-conflict")).toHaveCount(0)

    await page.keyboard.press("Escape")
    await expect(preview).toHaveCount(0)
  })

  test("should show the issue of a prefix branch", async ({ page }) => {
    const preview = await openPreviewOfHoveredBranch(page, "feature-auth")
    await expect(preview.getByTestId("branch-issue-key")).toHaveText("JIRA-123")
  })

  test("should open the preview of the last toggled branch with Space while the table has the focus", async ({ page }) => {
    const branchRow = findBranchRow(page, NEXT_BRANCH)
    await branchRow.getByText(NEXT_BRANCH, { exact: true }).click()
    await verifyBranchState(branchRow, "open")

    // away from every row: the focused table falls back to its last toggled row
    await page.mouse.move(0, 0)
    await page.keyboard.press("Space")
    const preview = getPreview(page)
    await expect(preview.getByRole("heading")).toHaveText(NEXT_BRANCH)
    await expectPreviewFocused(preview)
    await expect(page.getByTestId("commit-preview")).toHaveCount(0)
  })

  test("should leave Space to a commit list nested in the branch table", async ({ page }) => {
    await expandBranch(page, findBranchRow(page, BRANCH))
    const commitRow = getBranchDetailsRow(page, BRANCH).locator("tr[data-row-id]").filter({ hasText: "perf db: increase pool size to 50" })

    // over a commit row: the commit, not the branch around it
    await commitRow.hover()
    await page.keyboard.press("Space")
    const commitPreview = page.getByTestId("commit-preview")
    await expect(commitPreview.getByRole("heading", { level: 2 })).toHaveText("perf db: increase pool size to 50")
    await expect(getPreview(page)).toHaveCount(0)
    await page.keyboard.press("Escape")
    await expect(commitPreview).toHaveCount(0)

    // the focus in the commit list, the pointer away from every row: the last clicked commit
    await commitRow.getByText("increase pool size to 50").click()
    await page.mouse.move(0, 0)
    await page.keyboard.press("Space")
    await expect(commitPreview.getByRole("heading", { level: 2 })).toHaveText("perf db: increase pool size to 50")
    await expect(getPreview(page)).toHaveCount(0)
  })

  test("should push the branch with P", async ({ page }) => {
    const pushes = await routePush(page, {})

    const preview = await openPreviewOfHoveredBranch(page, BRANCH)
    const pushButton = preview.getByRole("button", { name: /Push \(\d+\)/ })
    await expect(pushButton).toBeEnabled()

    await page.keyboard.press("p")
    await expectPushedToast(page, BRANCH)
    expect(pushes.map(push => push.branchName)).toEqual([BRANCH])
    await expect(preview.getByTestId("branch-preview-status")).toHaveText("up to date")
    await expect(preview.getByTestId("branch-preview-last-push")).toHaveText(/^Pushed /)
    // nothing left to push
    await expect(preview.getByRole("button", { name: /Push/ })).toBeDisabled()
    await expect(preview).toBeVisible()
  })

  test("should ask before P force pushes over a commit the branch lacks", async ({ page }) => {
    const pushes = await routePush(page, {
      // someone pushed a commit to the remote branch meanwhile
      [BRANCH]: [
        {
          unpushedCommits: ["j1"],
          commitsBehind: 1,
          remoteTip: "their-head",
          droppedCommits: [{ id: "c0ffee0123456789", subject: "Fix login", author: "Ann", authorTime: 1704178800 }],
        },
        {},
      ],
    })
    const preview = await openPreviewOfHoveredBranch(page, BRANCH)
    await page.keyboard.press("p")
    await expect(preview.getByRole("button", { name: /Force Push \(1\)/ })).toBeEnabled()

    // the footer's push button opens its description with what the push drops
    await page.keyboard.press("p")
    await expect(page.getByTestId("push-description")).toContainText("Fix login")
    expect(pushes).toHaveLength(1)
    await confirmForcePush(page, "Force Push (1)")

    await expect.poll(() => pushes.map(push => push.expectedRemote)).toEqual([null, "their-head"])
    await expect(preview).toBeVisible()
  })

  test("should keep the preview on its branch across a sync", async ({ page }) => {
    const preview = await openPreviewOfHoveredBranch(page, BRANCH)
    await page.keyboard.press("ArrowDown")
    await expect(preview.getByTestId("branch-preview-position")).toHaveText(`2 / ${BRANCH_COUNT}`)

    await syncFromMenu(page)

    await expect(preview.getByRole("heading")).toHaveText(NEXT_BRANCH)
    await expect(preview.getByTestId("branch-preview-position")).toHaveText(`2 / ${BRANCH_COUNT}`)
    await page.keyboard.press("ArrowDown")
    await expect(preview.getByTestId("branch-preview-position")).toHaveText(`3 / ${BRANCH_COUNT}`)
  })

  test("should close the preview when a sync removes its branch", async ({ page, issueLinksRepo: repo }) => {
    await enableAutoSyncOnFocus(page)
    await expandBranch(page, findBranchRow(page, "feature-docs"))
    const hash = await getBranchDetailsRow(page, "feature-docs").locator("tr[data-row-id]").getAttribute("data-row-id")
    const preview = await openPreviewOfHoveredBranch(page, "feature-docs")

    // the commit is moved to another branch in a terminal
    await repo.reword(hash!, "(feature-documentation) Update documentation")
    await syncOnWindowFocus(page)

    await expect(preview).toHaveCount(0)
    await expect(findBranchRow(page, "feature-documentation")).toBeVisible()
  })
})

testWithConflicts.describe("Branch preview of a conflicting branch", () => {
  testWithConflicts("should show the merge conflict above the commits", async ({ page, setupConflictRepo }) => {
    await setupConflictRepo("conflict_branches")
    const preview = await openPreviewOfHoveredBranch(page, "bug-fix")

    const notice = preview.getByTestId("branch-preview-conflict")
    await expect(notice).toContainText("Merge conflict")
    await expect(notice).toContainText("“Implement secure password hashing” does not apply cleanly")
    // where the commits it misses are, and the fix below
    await expect(notice).toContainText("1 commit of feature-auth changed these files first, as did 1 unassigned commit")
    await expect(preview.getByTestId("conflict-fix")).toHaveAttribute("data-state", "found")
    await expect(preview.getByTestId("branch-preview-commit").first()).toContainText("Merge conflict")
    // a branch with a conflict has nothing to push
    await expect(preview.getByRole("button", { name: /Push/ })).toHaveCount(0)
  })
})
