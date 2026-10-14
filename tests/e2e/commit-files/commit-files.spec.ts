// @spec spec/commit/commit-files.spec.md
import type { Locator, Page } from "@playwright/test"
import { test as simpleTest, testWithIssueLinks, expect } from "../fixtures/test-fixtures"
import { findBranchRow, expandBranch, getBranchCommitRow } from "../helpers/branch-helpers"
import { readClipboard } from "../helpers/clipboard-helpers"
import { clickContextMenuItem, multiSelectCommit, openContextMenu, selectCommit } from "../helpers/selection-helpers"
import { waitForBranchSyncComplete } from "../helpers/sync-helpers"

// the `issue_links` template: JIRA-456 has three commits, newest first; each changes one file under db/
const BRANCH = "JIRA-456"
const RETRY = "Add connection retry logic"
const POOL_SIZE = "perf db: increase pool size to 50"

function getPreview(page: Page): Locator {
  return page.getByTestId("commit-preview")
}

async function expandIssueBranch(page: Page) {
  await expandBranch(page, findBranchRow(page, BRANCH))
}

/** Opens the preview of the commit row under the pointer with Space and waits until it has the focus */
async function openPreview(page: Page, row: Locator): Promise<Locator> {
  await row.hover()
  await page.keyboard.press("Space")
  const preview = getPreview(page)
  await expect(preview.getByTestId("commit-preview-body")).toBeFocused()
  return preview
}

simpleTest.describe("Commit files in the simple repository", () => {
  simpleTest.beforeEach(async ({ setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("simple")
    await syncAndWaitForBranches()
  })

  simpleTest("should show the file count and the line counts in the meta line of each commit row", async ({ page }) => {
    await waitForBranchSyncComplete(page, "test-branch")
    await expandBranch(page, findBranchRow(page, "test-branch"))

    // the branch has one commit: "foo 1", the root commit, is origin/master (the root commit's files: the Rust tests)
    const row = getBranchCommitRow(page, "test-branch", "foo 2")
    await expect(row.getByTestId("commit-file-count")).toHaveText("1 file")
    await expect(row.getByTestId("commit-line-counts")).toHaveText("+1 −0")
    // muted in a row
    await expect(row.getByTestId("commit-line-counts").locator(".text-success")).toHaveCount(0)
  })

  simpleTest("should show the meta line with the files in the commits of the branch preview", async ({ page }) => {
    await waitForBranchSyncComplete(page, "test-branch")
    await findBranchRow(page, "test-branch").hover()
    await page.keyboard.press("Space")
    const commits = page.getByTestId("branch-preview").getByTestId("branch-preview-commit")
    await expect(commits).toHaveCount(1)
    await expect(commits.getByTestId("commit-file-count")).toHaveText("1 file")
    await expect(commits.getByTestId("commit-line-counts")).toHaveText("+1 −0")
  })
})

testWithIssueLinks.describe("Commit files", () => {
  testWithIssueLinks("should list the files of the commit in the preview and load a diff when a file is expanded", async ({ page }) => {
    await expandIssueBranch(page)
    const preview = await openPreview(page, getBranchCommitRow(page, BRANCH, POOL_SIZE))

    // the header colours the line counts
    const header = preview.getByTestId("commit-line-counts")
    await expect(header).toHaveText("+1 −1")
    await expect(header.locator(".text-success")).toHaveText("+1")
    await expect(header.locator(".text-destructive")).toHaveText("−1")

    const files = preview.getByRole("region", { name: "Files" })
    await expect(files).toHaveAttribute("data-testid", "commit-files")
    const file = files.getByRole("button", { name: "db/pool.js" })
    await expect(file).toBeVisible()
    await expect(files.getByText("modified")).toBeVisible()
    await expect(files.getByTestId("file-line-counts")).toHaveText("+1 −1")
    // the diff view mode of the conflict viewer
    await expect(files.getByRole("button", { name: "Unified" })).toHaveAttribute("aria-pressed", "true")

    // the highlighted diff splits a line into tokens: match the text of the section
    await expect(files).not.toContainText("size: 50")
    await file.click()
    await expect(files).toContainText("size: 50")

    // the next commit shows its own files, collapsed
    await preview.getByTestId("commit-preview-body").focus()
    await page.keyboard.press("ArrowDown")
    await expect(preview.getByRole("heading")).toHaveText("Fix database connection pool timeout")
    await expect(files.getByTestId("file-line-counts")).toHaveText("+2 −0")
    await expect(files).not.toContainText("size: 50")
  })

  testWithIssueLinks("should show and hide the files with F and the footer button", async ({ page }) => {
    await expandIssueBranch(page)
    const preview = await openPreview(page, getBranchCommitRow(page, BRANCH, RETRY))
    const files = preview.getByTestId("commit-files")
    const toggle = preview.getByTestId("commit-files-toggle")
    await expect(files).toBeVisible()
    await expect(toggle).toHaveAttribute("aria-pressed", "true")

    await page.keyboard.press("f")
    await expect(files).toHaveCount(0)
    await expect(toggle).toHaveAttribute("aria-pressed", "false")
    // the message stays
    await expect(preview.getByTestId("commit-message-body")).toBeVisible()

    // the choice holds for the next preview
    await page.keyboard.press("Escape")
    await openPreview(page, getBranchCommitRow(page, BRANCH, POOL_SIZE))
    await expect(files).toHaveCount(0)

    await toggle.click()
    await expect(files).toBeVisible()
    await expect(files.getByRole("button", { name: "db/pool.js" })).toBeVisible()
  })

  testWithIssueLinks("should copy the changed paths of the commits from the context menu", async ({ page }) => {
    await expandIssueBranch(page)
    const poolSize = getBranchCommitRow(page, BRANCH, POOL_SIZE)
    await openContextMenu(page, poolSize)
    await clickContextMenuItem(page, "Copy Changed Paths")
    await expect.poll(() => readClipboard(page)).toBe("db/pool.js")

    // two commits: each path once
    await selectCommit(page, getBranchCommitRow(page, BRANCH, RETRY))
    await multiSelectCommit(page, poolSize)
    await multiSelectCommit(page, getBranchCommitRow(page, BRANCH, "Fix database connection pool timeout"))
    await openContextMenu(page, poolSize)
    await clickContextMenuItem(page, "Copy Changed Paths of 3 Commits")
    await expect.poll(async () => (await readClipboard(page)).split("\n").sort()).toEqual(["db/pool.js", "db/retry.js"])
  })
})
