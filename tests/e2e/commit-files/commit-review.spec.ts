// @spec spec/commit/commit-review.spec.md
import type { Locator, Page } from "@playwright/test"
import type { CommitReviewWindowData } from "../../../src/lib/commit-review-window"
import { test as simpleTest, testWithIssueLinks, expect } from "../fixtures/test-fixtures"
import { findBranchRow, expandBranch, getBranchCommitRow } from "../helpers/branch-helpers"
import { getCommitPreview, openCommitPreview } from "../helpers/preview-helpers"
import { getOpenedSubWindows, getSubWindowReadyOpen, openLinkedSubWindow, openSubWindowPage } from "../helpers/sub-window-helpers"
import type { OpenedSubWindow } from "../helpers/sub-window-helpers"
import { holdCommand } from "../helpers/wait-helpers"

// the `commit_tree` template: one commit of the `session` branch, five files in nested directories
const SUBJECT = "a prompt that waits for its turn keeps the session row running"

async function openTreePreview(page: Page): Promise<Locator> {
  await expandBranch(page, findBranchRow(page, "session"))
  return await openCommitPreview(page, getBranchCommitRow(page, "session", SUBJECT))
}

function selectedFile(review: Locator): Locator {
  return review.getByRole("tree", { name: "Changed files" }).locator("[role=treeitem][aria-selected=true]")
}

/** The `count`-th review window the page opened (`open_sub_window`), once it is recorded */
async function openedReview(page: Page, count: number): Promise<OpenedSubWindow<CommitReviewWindowData>> {
  await expect.poll(async () => (await getOpenedSubWindows(page)).length).toBe(count)
  const opened = (await getOpenedSubWindows<CommitReviewWindowData>(page))[count - 1]!
  expect(opened).toMatchObject({ windowId: "commit-review", url: "/commit-review" })
  return opened
}

/** Shows the review window in the page, as the app opens it */
async function showReviewWindow(page: Page, opened: OpenedSubWindow<CommitReviewWindowData>): Promise<Locator> {
  await openSubWindowPage(page, opened)
  const window = page.getByTestId("commit-review-window")
  await expect(window).toBeVisible()
  return window
}

/**
 * D in the preview of "perf db: increase pool size to 50" of JIRA-456: the `issue_links` template has three commits
 * there, each changing one file under db/, and this is the second
 */
async function reviewPoolSizeCommit(page: Page): Promise<OpenedSubWindow<CommitReviewWindowData>> {
  await expandBranch(page, findBranchRow(page, "JIRA-456"))
  await openCommitPreview(page, getBranchCommitRow(page, "JIRA-456", "perf db: increase pool size to 50"))
  await page.keyboard.press("d")
  return await openedReview(page, 1)
}

simpleTest.describe("Commit review", () => {
  simpleTest.beforeEach(async ({ setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("commit_tree")
    await syncAndWaitForBranches()
  })

  simpleTest("should show the tree beside the diffs of all files and follow J, K, N, P and the tree", async ({ page }) => {
    const preview = await openTreePreview(page)
    await page.keyboard.press("d")
    // D in the preview: the review in its window, at the first file; the preview closes
    await expect(preview).toBeHidden()
    const opened = await openedReview(page, 1)
    expect(opened.title).toMatch(new RegExp(`^Review [0-9a-f]{8} ${SUBJECT}$`))
    expect(opened.data).toMatchObject({ index: 0, path: null, listLabel: "session" })

    const window = await showReviewWindow(page, opened)
    const position = window.getByTestId("commit-review-position")
    const code = window.getByTestId("commit-review-code")
    // the files in the order of the tree, the first one selected
    await expect(position).toHaveText("File 1 / 5")
    await expect(selectedFile(window)).toHaveAttribute("aria-label", "RowMapper.kt")
    await expect(code).toContainText("SessionRowMapper")
    // the header of a file names its directory and itself
    await expect(code.getByTestId("review-file-header").first()).toContainText("plugins/air/backend/session/catalog/src/RowMapper.kt")

    await code.locator("[aria-label='Changes of the commit']").focus()
    await page.keyboard.press("j")
    await expect(position).toHaveText("File 2 / 5")
    await expect(selectedFile(window)).toHaveAttribute("aria-label", "RowMapperTest.kt")
    await page.keyboard.press("k")
    await expect(position).toHaveText("File 1 / 5")

    // the blocks of changed lines: two in RowMapper.kt, then the next file
    await page.keyboard.press("n")
    await page.keyboard.press("n")
    await expect(position).toHaveText("File 1 / 5")
    await page.keyboard.press("n")
    await expect(position).toHaveText("File 2 / 5")
    await page.keyboard.press("p")
    await expect(position).toHaveText("File 1 / 5")

    // a file of the tree scrolls the diffs to it, also the last one
    await window.getByRole("treeitem", { name: "README.md" }).click()
    await expect(position).toHaveText("File 5 / 5")
    await expect(code).toContainText("one row each")

    // the view of the diffs is in the title bar of the window
    await window.getByRole("button", { name: "Split" }).click()
    await expect(window.getByRole("button", { name: "Split" })).toHaveAttribute("aria-pressed", "true")
  })

  simpleTest("should open the review at a file with Enter in the tree, and with the footer button", async ({ page }) => {
    const preview = await openTreePreview(page)
    await preview.getByRole("treeitem", { name: "plugins/air" }).focus()
    // plugins/air, backend/session, catalog, src, RowMapper.kt, testSrc, RowMapperTest.kt
    for (let i = 0; i < 6; i++) {
      await page.keyboard.press("ArrowDown")
    }
    await expect(preview.getByRole("treeitem", { name: "RowMapperTest.kt" })).toBeFocused()
    await page.keyboard.press("Enter")
    await expect(preview).toBeHidden()
    const atFile = await openedReview(page, 1)
    expect(atFile.data.path).toBe("plugins/air/backend/session/catalog/testSrc/RowMapperTest.kt")

    // the footer button: the first file
    await openCommitPreview(page, getBranchCommitRow(page, "session", SUBJECT))
    await preview.getByTestId("commit-review-toggle").click()
    await expect(preview).toBeHidden()
    expect((await openedReview(page, 2)).data.path).toBeNull()

    const window = await showReviewWindow(page, atFile)
    await expect(window.getByTestId("commit-review-position")).toHaveText("File 2 / 5")
    await expect(selectedFile(window)).toHaveAttribute("aria-label", "RowMapperTest.kt")
  })
})

testWithIssueLinks("should switch the commit of a review with Alt and the arrows", async ({ page }) => {
  const opened = await reviewPoolSizeCommit(page)
  // the list as it was, the commit of it
  expect(opened.data).toMatchObject({ index: 1, path: null, listLabel: "JIRA-456" })
  expect(opened.data.commits).toHaveLength(3)

  const window = await showReviewWindow(page, opened)
  const position = window.getByTestId("commit-review-position")
  await expect(position).toHaveText("Commit 2 / 3 · File 1 / 1")
  await expect(window.getByTestId("range-review-commit-header")).toContainText("perf db: increase pool size to 50")
  await expect(window.getByTestId("commit-review-code")).toContainText("size: 50")

  await page.keyboard.press("Alt+ArrowDown")
  await expect(window.getByTestId("range-review-commit-header")).toContainText("Fix database connection pool timeout")
  await expect(position).toHaveText("Commit 3 / 3 · File 1 / 1")
  await expect(window.getByTestId("commit-review-window-footer")).toContainText("EscClose")
})

testWithIssueLinks("should show the files while the patch is read, settle with the diffs, and step to a commit read beside it", async ({ page }) => {
  const opened = await reviewPoolSizeCommit(page)
  // the window's own reads of patches, and the patch of its commit held
  const patchReads: unknown[] = []
  let release!: () => void
  const review = await openLinkedSubWindow(page, opened, {
    beforeLoad: async window => {
      window.on("request", request => {
        if (request.url().endsWith("/invoke/get_commit_patch")) {
          patchReads.push(request.postDataJSON())
        }
      })
      release = await holdCommand(window, "get_commit_patch")
    },
  })

  // the tree and the header of the file while git reads the patch; the window waits for the diffs to show
  await expect(review.getByRole("tree", { name: "Changed files" })).toBeVisible()
  await expect(review.getByTestId("review-pending-files").getByTestId("review-file-header")).toHaveCount(1)
  expect(await getSubWindowReadyOpen(review)).toBeNull()

  release()
  await expect(review.getByTestId("commit-review-code")).toContainText("size: 50")
  await expect(review.getByTestId("review-pending-files")).toBeHidden()
  await expect.poll(() => getSubWindowReadyOpen(review)).toBe(1)

  // the commits before and after it were read with it: ⌥↓ shows the next one without a read
  await expect.poll(() => patchReads.length).toBe(3)
  await review.keyboard.press("Alt+ArrowDown")
  await expect(review.getByTestId("range-review-commit-header")).toContainText("Fix database connection pool timeout")
  await expect(review.getByTestId("commit-review-code")).toBeVisible()
  expect(patchReads).toHaveLength(3)
})

testWithIssueLinks("should review a commit of a row in its window with D and with the item of its menu", async ({ page }) => {
  await expandBranch(page, findBranchRow(page, "JIRA-456"))
  const row = getBranchCommitRow(page, "JIRA-456", "perf db: increase pool size to 50")
  const preview = getCommitPreview(page)

  // D over the row: its review in the window, no preview; the branch row under it was not reviewed
  await row.hover()
  await page.keyboard.press("d")
  const byKey = await openedReview(page, 1)
  expect(byKey.title).toMatch(/^Review [0-9a-f]{8} perf db: increase pool size to 50$/)
  expect(byKey.data).toMatchObject({ index: 1, path: null })
  await expect(preview).toBeHidden()

  // Review Changes, the only review item of the row's menu, first
  await row.click({ button: "right" })
  await expect(page.getByRole("menuitem").first()).toHaveText("Review ChangesD")
  await expect(page.getByRole("menuitem", { name: "Open in Window" })).toHaveCount(0)
  await page.getByRole("menuitem", { name: "Review Changes" }).click()
  expect((await openedReview(page, 2)).data).toMatchObject({ index: 1, path: null })
  await expect(page.getByRole("menu")).toBeHidden()

  // an open menu keeps its keys: D reviews nothing; right after Escape, D reviews the row
  await row.click({ button: "right" })
  await expect(page.getByRole("menu")).toBeVisible()
  await page.keyboard.press("d")
  await expect(page.getByRole("menu")).toBeVisible()
  await page.keyboard.press("Escape")
  await row.hover()
  await page.keyboard.press("d")
  await openedReview(page, 3)
  // the D under the open menu opened none: the third open is the one after Escape
  expect(await getOpenedSubWindows(page)).toHaveLength(3)
  await expect(preview).toBeHidden()
})

testWithIssueLinks("should give the focus to the list after the preview, its keys going on from the commit it showed last", async ({ page }) => {
  await expandBranch(page, findBranchRow(page, "JIRA-456"))
  const first = getBranchCommitRow(page, "JIRA-456", "perf db: increase pool size to 50")
  const preview = getCommitPreview(page)
  // a click on a row: its list has the focus; the pointer then leaves the rows, so the keys act on the list
  await first.click()
  await page.mouse.move(1, 1)
  await page.keyboard.press("Space")
  await expect(preview.getByTestId("commit-preview-body")).toBeFocused()
  await page.keyboard.press("ArrowDown")
  await expect(preview.getByRole("heading", { level: 2 })).toHaveText("Fix database connection pool timeout")

  // D: the window of the commit shown; the list has the focus again
  await page.keyboard.press("d")
  expect((await openedReview(page, 1)).data).toMatchObject({ index: 2 })
  await expect(preview).toBeHidden()
  await expect(first.locator("xpath=ancestor::table[1]")).toBeFocused()
  // D and Space act on that commit, not on the clicked one
  await page.keyboard.press("d")
  expect((await openedReview(page, 2)).data).toMatchObject({ index: 2 })
  await page.keyboard.press("Space")
  await expect(preview.getByRole("heading", { level: 2 })).toHaveText("Fix database connection pool timeout")

  // a click of a row is the anchor again
  await page.keyboard.press("Escape")
  await first.click()
  await page.mouse.move(1, 1)
  await page.keyboard.press("d")
  expect((await openedReview(page, 3)).data).toMatchObject({ index: 1 })
})
