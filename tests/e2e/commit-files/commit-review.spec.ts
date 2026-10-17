// @spec spec/commit/commit-review.spec.md
import type { Locator, Page } from "@playwright/test"
import type { CommitReviewWindowData } from "../../../src/lib/commit-review-window"
import { test as simpleTest, testWithIssueLinks, expect } from "../fixtures/test-fixtures"
import { findBranchRow, expandBranch, getBranchCommitRow } from "../helpers/branch-helpers"
import { getCommitPreview, openCommitPreview } from "../helpers/preview-helpers"
import { getOpenedSubWindows, openSubWindowPage } from "../helpers/sub-window-helpers"

// the `commit_tree` template: one commit of the `session` branch, five files in nested directories
const SUBJECT = "a prompt that waits for its turn keeps the session row running"

async function openTreePreview(page: Page): Promise<Locator> {
  await expandBranch(page, findBranchRow(page, "session"))
  return await openCommitPreview(page, getBranchCommitRow(page, "session", SUBJECT))
}

function selectedFile(review: Locator): Locator {
  return review.getByRole("tree", { name: "Changed files" }).locator("[role=treeitem][aria-selected=true]")
}

simpleTest.describe("Commit review", () => {
  simpleTest.beforeEach(async ({ setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("commit_tree")
    await syncAndWaitForBranches()
  })

  simpleTest("should show the tree beside the diffs of all files and follow J, K, N, P and the tree", async ({ page }) => {
    const preview = await openTreePreview(page)
    await page.keyboard.press("d")
    const review = preview.getByTestId("commit-review")
    const position = preview.getByTestId("commit-review-position")
    const code = review.getByTestId("commit-review-code")
    await expect(review).toBeVisible()
    // the files in the order of the tree, the first one selected; the diffs have the focus
    await expect(position).toHaveText("File 1 / 5")
    await expect(selectedFile(review)).toHaveAttribute("aria-label", "RowMapper.kt")
    await expect(code).toContainText("SessionRowMapper")
    await expect(code.locator("[aria-label='Changes of the commit']")).toBeFocused()
    // the header of a file names its directory and itself
    await expect(code.getByTestId("review-file-header").first()).toContainText("plugins/air/backend/session/catalog/src/RowMapper.kt")

    await page.keyboard.press("j")
    await expect(position).toHaveText("File 2 / 5")
    await expect(selectedFile(review)).toHaveAttribute("aria-label", "RowMapperTest.kt")
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
    await review.getByRole("treeitem", { name: "README.md" }).click()
    await expect(position).toHaveText("File 5 / 5")
    await expect(code).toContainText("one row each")

    // the view of the diffs is in the header of the dialog, beside what it switches
    await preview.getByRole("button", { name: "Split" }).click()
    await expect(preview.getByRole("button", { name: "Split" })).toHaveAttribute("aria-pressed", "true")

    // Escape goes back to the preview of the commit, the next one closes it
    await page.keyboard.press("Escape")
    await expect(review).toHaveCount(0)
    await expect(preview.getByRole("heading", { level: 2 })).toHaveText(SUBJECT)
    await expect(preview.getByTestId("commit-preview-body")).toBeFocused()
    await page.keyboard.press("Escape")
    await expect(getCommitPreview(page)).toHaveCount(0)
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
    const review = preview.getByTestId("commit-review")
    await expect(preview.getByTestId("commit-review-position")).toHaveText("File 2 / 5")
    await expect(selectedFile(review)).toHaveAttribute("aria-label", "RowMapperTest.kt")

    await page.keyboard.press("Escape")
    await expect(review).toHaveCount(0)
    await preview.getByTestId("commit-review-toggle").click()
    await expect(preview.getByTestId("commit-review-position")).toHaveText("File 1 / 5")
  })
})

testWithIssueLinks("should switch the commit of a review with Alt and the arrows", async ({ page }) => {
  // the `issue_links` template: JIRA-456 has three commits, each changing one file under db/
  await expandBranch(page, findBranchRow(page, "JIRA-456"))
  const preview = await openCommitPreview(page, getBranchCommitRow(page, "JIRA-456", "perf db: increase pool size to 50"))
  await page.keyboard.press("d")
  const position = preview.getByTestId("commit-review-position")
  await expect(position).toHaveText("Commit 2 / 3 · File 1 / 1")
  await expect(preview.getByTestId("commit-review-code")).toContainText("size: 50")

  await page.keyboard.press("Alt+ArrowDown")
  await expect(preview.getByRole("heading", { level: 2 })).toHaveText("Fix database connection pool timeout")
  await expect(position).toHaveText("Commit 3 / 3 · File 1 / 1")
  await expect(preview.getByTestId("commit-review")).toBeVisible()
})

testWithIssueLinks("should review a commit of a row with D, in its window with W, where the review was", async ({ page }) => {
  await expandBranch(page, findBranchRow(page, "JIRA-456"))
  const row = getBranchCommitRow(page, "JIRA-456", "perf db: increase pool size to 50")
  const preview = getCommitPreview(page)

  // D over the row: the review at once, Escape closes it
  await row.hover()
  await page.keyboard.press("d")
  await expect(preview.getByTestId("commit-review-position")).toHaveText("Commit 2 / 3 · File 1 / 1")
  await expect(preview.locator("[aria-label='Changes of the commit']")).toBeFocused()
  await expect(preview).toContainText("EscClose")
  await page.keyboard.press("Escape")
  await expect(preview).toBeHidden()
  // the branch row under it was not reviewed
  await expect(page.getByTestId("branch-preview")).toBeHidden()

  // Review Changes, the first item of the row's menu: the review at once
  await row.click({ button: "right" })
  await expect(page.getByRole("menuitem").first()).toHaveText("Review ChangesD")
  await page.getByRole("menuitem", { name: "Review Changes" }).click()
  await expect(preview.getByTestId("commit-review-position")).toHaveText("Commit 2 / 3 · File 1 / 1")
  await page.keyboard.press("Escape")
  await expect(preview).toBeHidden()

  // an open menu keeps its keys: D reviews nothing; right after Escape, D reviews the row (below)
  await row.click({ button: "right" })
  await expect(page.getByRole("menu")).toBeVisible()
  await page.keyboard.press("d")
  await expect(page.getByRole("menu")).toBeVisible()
  await page.keyboard.press("Escape")

  // W in the review: its window, at the commit of the list and the file
  await row.hover()
  await page.keyboard.press("d")
  await expect(preview.getByTestId("commit-review-position")).toHaveText("Commit 2 / 3 · File 1 / 1")
  await preview.getByTestId("commit-review-open-in-window").click()
  await expect(preview).toBeHidden()
  const [opened] = await getOpenedSubWindows<CommitReviewWindowData>(page)
  expect(opened).toMatchObject({
    windowId: "commit-review",
    url: "/commit-review",
    title: expect.stringMatching(/^Review [0-9a-f]{8} perf db: increase pool size to 50$/) as unknown as string,
  })
  expect(opened!.data).toMatchObject({ index: 1, path: "db/pool.js" })
  expect(opened!.data.commits.map(commit => commit.subject)).toHaveLength(3)

  // while it is open, D on a row reviews there
  await row.hover()
  await page.keyboard.press("d")
  await expect.poll(async () => (await getOpenedSubWindows(page)).length).toBe(2)
  await expect(preview).toBeHidden()

  // the window: the commits of the list beside the diffs, ⌥↓ the next one, Escape closes it
  await openSubWindowPage(page, opened!)
  const window = page.getByTestId("commit-review-window")
  await expect(window.getByTestId("commit-review-position")).toHaveText("Commit 2 / 3 · File 1 / 1")
  await expect(window.getByTestId("range-review-commit-header")).toContainText("perf db: increase pool size to 50")
  await page.keyboard.press("Alt+ArrowDown")
  await expect(window.getByTestId("commit-review-position")).toHaveText("Commit 3 / 3 · File 1 / 1")
  await expect(window.getByTestId("commit-review-window-footer")).toContainText("EscClose")
})
