// @spec spec/commit/commit-files.spec.md
import type { Locator, Page } from "@playwright/test"
import { test as simpleTest, testWithIssueLinks, expect } from "../fixtures/test-fixtures"
import { findBranchRow, expandBranch, getBranchCommitRow } from "../helpers/branch-helpers"
import { readClipboard } from "../helpers/clipboard-helpers"
import { chooseWhitespace, openCommitFilesSection, openCommitPreview as openPreview } from "../helpers/preview-helpers"
import { clickContextMenuItem, multiSelectCommit, openContextMenu, selectCommit } from "../helpers/selection-helpers"
import { openSettingsWindow } from "../helpers/settings-helpers"
import { waitForBranchSyncComplete } from "../helpers/sync-helpers"

// the `issue_links` template: JIRA-456 has three commits, newest first; each changes one file under db/
const BRANCH = "JIRA-456"
const RETRY = "Add connection retry logic"
const POOL_SIZE = "perf db: increase pool size to 50"

async function expandIssueBranch(page: Page) {
  await expandBranch(page, findBranchRow(page, BRANCH))
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
    // closed at first: its header with the count, no tree
    await expect(files.getByTestId("commit-files-header")).toHaveText("Files1")
    await expect(files.getByRole("tree")).toHaveCount(0)
    await openCommitFilesSection(preview)
    // grouped by directory: `db/`, then the file by its name, the status and the counts in its description
    await expect(files.getByRole("treeitem", { name: "db" })).toHaveAttribute("aria-level", "1")
    const file = files.getByRole("treeitem", { name: "pool.js" })
    await expect(file).toHaveAttribute("aria-level", "2")
    await expect(file).toHaveAttribute("aria-description", "modified, +1 −1")
    await expect(file).toHaveAttribute("aria-expanded", "false")
    await expect(files.getByTestId("file-line-counts")).toHaveText("+1 −1")
    // the layout of every diff view, Split by default as in IntelliJ
    await expect(files.getByRole("button", { name: "Split" })).toHaveAttribute("aria-pressed", "true")

    // the highlighted diff splits a line into tokens: match the text of the section
    await expect(files).not.toContainText("size: 50")
    await file.getByText("pool.js").click()
    await expect(file).toHaveAttribute("aria-expanded", "true")
    await expect(files).toContainText("size: 50")

    // the next commit shows its own files, collapsed
    await preview.getByTestId("commit-preview-body").focus()
    await page.keyboard.press("ArrowDown")
    await expect(preview.getByRole("heading", { level: 2 })).toHaveText("JIRA-456 Fix database connection pool timeout")
    await expect(files.getByTestId("file-line-counts")).toHaveText("+2 −0")
    await expect(files).not.toContainText("size: 50")
  })

  testWithIssueLinks("should open and close the files with their header, F and the footer button", async ({ page }) => {
    await expandIssueBranch(page)
    const preview = await openPreview(page, getBranchCommitRow(page, BRANCH, RETRY))
    const header = preview.getByTestId("commit-files-header")
    const tree = preview.getByRole("tree", { name: "Changed files" })
    const toggle = preview.getByTestId("commit-files-toggle")
    // closed at first: the header, no tree
    await expect(header).toHaveAttribute("aria-expanded", "false")
    await expect(toggle).toHaveAttribute("aria-pressed", "false")
    await expect(tree).toHaveCount(0)

    await page.keyboard.press("f")
    await expect(header).toHaveAttribute("aria-expanded", "true")
    await expect(toggle).toHaveAttribute("aria-pressed", "true")
    await expect(tree.getByRole("treeitem", { name: "retry.js" })).toBeVisible()
    await page.keyboard.press("f")
    await expect(header).toHaveAttribute("aria-expanded", "false")
    await expect(tree).toHaveCount(0)
    // the message stays
    await expect(preview.getByTestId("commit-message-body")).toBeVisible()

    // the header opens it, and the choice holds for the next preview
    await header.click()
    await expect(tree.getByRole("treeitem", { name: "retry.js" })).toBeVisible()
    await page.keyboard.press("Escape")
    await openPreview(page, getBranchCommitRow(page, BRANCH, POOL_SIZE))
    await expect(header).toHaveAttribute("aria-expanded", "true")
    await expect(tree.getByRole("treeitem", { name: "pool.js" })).toBeVisible()

    // the footer button closes it
    await toggle.click()
    await expect(header).toHaveAttribute("aria-expanded", "false")
    await expect(tree).toHaveCount(0)
  })

  testWithIssueLinks("should move through the tree with the keyboard and switch commits with Alt", async ({ page }) => {
    await expandIssueBranch(page)
    const preview = await openPreview(page, getBranchCommitRow(page, BRANCH, POOL_SIZE))
    const heading = preview.getByRole("heading", { level: 2 })
    const files = preview.getByTestId("commit-files")
    await openCommitFilesSection(preview)
    const directory = files.getByRole("treeitem", { name: "db" })
    const file = files.getByRole("treeitem", { name: "pool.js" })

    // one row is in the tab order
    await expect(directory).toHaveAttribute("tabindex", "0")
    await expect(file).toHaveAttribute("tabindex", "-1")
    await directory.focus()
    await page.keyboard.press("ArrowDown")
    await expect(file).toBeFocused()
    // the arrows stay in the tree, also past its last row: the commit stays
    await page.keyboard.press("ArrowDown")
    await expect(file).toBeFocused()
    await expect(heading).toHaveText(/increase pool size to 50/)

    await page.keyboard.press("ArrowRight")
    await expect(file).toHaveAttribute("aria-expanded", "true")
    await expect(files).toContainText("size: 50")
    await page.keyboard.press("ArrowLeft")
    await expect(file).toHaveAttribute("aria-expanded", "false")
    await page.keyboard.press("ArrowLeft")
    await expect(directory).toBeFocused()
    await page.keyboard.press("ArrowLeft")
    await expect(directory).toHaveAttribute("aria-expanded", "false")
    await expect(file).toHaveCount(0)
    // Space toggles a row, the preview stays open
    await page.keyboard.press("Space")
    await expect(directory).toHaveAttribute("aria-expanded", "true")
    await expect(preview).toBeVisible()

    // Alt with an arrow is the dialog's: the next commit
    await page.keyboard.press("Alt+ArrowDown")
    await expect(heading).toHaveText("JIRA-456 Fix database connection pool timeout")
  })

  testWithIssueLinks("should copy the changed paths of the commits from the context menu", async ({ page }) => {
    await expandIssueBranch(page)
    const poolSize = getBranchCommitRow(page, BRANCH, POOL_SIZE)
    await openContextMenu(page, poolSize)
    await clickContextMenuItem(page, "Copy Changed Paths")
    await expect.poll(() => readClipboard(page)).toBe("db/pool.js")

    // two commits: each path once
    await selectCommit(getBranchCommitRow(page, BRANCH, RETRY))
    await multiSelectCommit(poolSize)
    await multiSelectCommit(getBranchCommitRow(page, BRANCH, "Fix database connection pool timeout"))
    await openContextMenu(page, poolSize)
    await clickContextMenuItem(page, "Copy Changed Paths of 3 Commits")
    await expect.poll(async () => (await readClipboard(page)).split("\n").sort()).toEqual(["db/pool.js", "db/retry.js"])
  })
})

/** The preview of the one commit of the `session` branch of the `commit_tree` template, and its tree */
async function openTreePreview(page: Page) {
  await expandBranch(page, findBranchRow(page, "session"))
  const preview = await openPreview(page, getBranchCommitRow(page, "session", "a prompt that waits for its turn keeps the session row running"))
  return { preview, tree: await openCommitFilesSection(preview) }
}

simpleTest.describe("Commit files in nested directories", () => {
  simpleTest.beforeEach(async ({ setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("commit_tree")
    await syncAndWaitForBranches()
  })

  simpleTest("should copy the path of a file or the changed paths of a directory from the menu of its row, and with ⌘C", async ({ page }) => {
    const { preview, tree } = await openTreePreview(page)

    await tree.getByRole("treeitem", { name: "README.md" }).click({ button: "right" })
    await expect(page.getByRole("menuitem")).toHaveText([/^Copy Path/])
    await clickContextMenuItem(page, "Copy Path")
    await expect.poll(() => readClipboard(page)).toBe("README.md")

    await tree.getByRole("treeitem", { name: "plugins/air" }).click({ button: "right" })
    await expect(page.getByRole("menuitem")).toHaveText([/^Copy Path/, "Copy Changed Paths"])
    await clickContextMenuItem(page, "Copy Changed Paths")
    await expect
      .poll(async () => (await readClipboard(page)).split("\n"))
      .toEqual([
        "plugins/air/backend/session/catalog/src/RowMapper.kt",
        "plugins/air/backend/session/catalog/testSrc/RowMapperTest.kt",
        "plugins/air/backend/session/projection/src/Reducer.kt",
        "plugins/air/build.gradle.kts",
      ])

    // ⌘C on a focused row copies its path (Ctrl+C where the page is not on macOS), not while text is selected
    const copyKey = (await page.evaluate(() => navigator.userAgent.toLowerCase().includes("mac"))) ? "Meta+c" : "Control+c"
    const gradle = tree.getByRole("treeitem", { name: "build.gradle.kts" })
    await gradle.focus()
    await page.keyboard.press(copyKey)
    await expect.poll(() => readClipboard(page)).toBe("plugins/air/build.gradle.kts")
    await tree.getByRole("treeitem", { name: "README.md" }).focus()
    await preview.getByRole("heading", { level: 2 }).evaluate(heading => window.getSelection()?.selectAllChildren(heading))
    await page.keyboard.press(copyKey)
    await expect.poll(() => readClipboard(page)).not.toBe("README.md")
  })

  simpleTest("should group the files by directory and expand the unchanged lines of a diff", async ({ page }) => {
    const { tree } = await openTreePreview(page)
    const rows = tree.getByRole("treeitem")
    // a directory that holds one directory only is merged with it; directories first
    await expect
      .poll(async () => {
        const outline: string[] = []
        for (const row of await rows.all()) {
          outline.push(`${await row.getAttribute("aria-level")} ${await row.getAttribute("aria-label")}`)
        }
        return outline
      })
      .toEqual([
        "1 plugins/air",
        "2 backend/session",
        "3 catalog",
        "4 src",
        "5 RowMapper.kt",
        "4 testSrc",
        "5 RowMapperTest.kt",
        "3 projection/src",
        "4 Reducer.kt",
        "2 build.gradle.kts",
        "1 README.md",
      ])
    // a directory sums the lines of its files
    await expect(tree.getByRole("treeitem", { name: "catalog" })).toHaveAttribute("aria-description", "2 files, +8 −2")
    await expect(tree.getByRole("treeitem", { name: "RowMapperTest.kt" })).toHaveAttribute("aria-description", "added, +6 −0")

    // two changes with 20 unchanged lines between them, 4 shown next to each: the fold names the scope of its lines
    const mapper = tree.getByRole("treeitem", { name: "RowMapper.kt" })
    await mapper.getByText("RowMapper.kt").click()
    await expect(mapper).toContainText("SessionRowMapper")
    await expect(mapper).not.toContainText("field10")
    const fold = mapper.locator(".dv-fold-button")
    await expect(fold).toHaveAttribute("aria-label", "Expand 12 unchanged lines in SessionRowMapper")
    // the folds nest as IntelliJ's: 8 lines next to each change, then all
    await fold.click()
    await expect(fold).toHaveAttribute("aria-label", "Expand 4 unchanged lines in SessionRowMapper")
    await fold.click()
    await expect(mapper).toContainText("val field10 = 10")
    await expect(fold).toHaveCount(0)
  })

  simpleTest("Split scrolls both sides together", async ({ page }) => {
    const { reducer, deletions, additions } = await openReducerDiff(page)
    await expect.poll(() => overflowsSideways(deletions)).toBe(true)

    // a wheel on the old side scrolls the new side to the same place, the line numbers stay
    await reducer.locator('.dv-half[data-side="0"] .dv-code').first().hover()
    await page.mouse.wheel(160, 0)
    await expect.poll(() => scrollLeft(deletions)).toBeGreaterThan(0)
    await expect.poll(async () => (await scrollLeft(additions)) === (await scrollLeft(deletions))).toBe(true)
    // and back from the new side
    await reducer.locator('.dv-half[data-side="1"] .dv-code').first().hover()
    await page.mouse.wheel(-40, 0)
    await expect.poll(async () => (await scrollLeft(deletions)) === (await scrollLeft(additions)) && (await scrollLeft(additions)) < 160).toBe(true)
  })

  simpleTest("Split shows an added file as its one side, with its line numbers, as IntelliJ's one-sided viewer", async ({ page }) => {
    const { tree, preview } = await openTreePreview(page)
    await expect(preview.getByRole("button", { name: "Split" })).toHaveAttribute("aria-pressed", "true")
    const added = tree.getByRole("treeitem", { name: "RowMapperTest.kt" })
    await added.getByText("RowMapperTest.kt").click()
    // one column the width of the view, each row its line number and its line, inserted; no empty half beside it
    const rows = added.locator(".dv-row[data-kind=added]")
    await expect(rows).toHaveCount(6)
    await expect(rows.first().locator(".dv-half")).toHaveCount(1)
    await expect(rows.first().locator(".dv-num")).toHaveText(["1"])
    await expect(rows.first().locator(".dv-half")).toHaveAttribute("data-cell", "inserted")
    // a line longer than the column keeps its horizontal scroll: the one column has the scroll bar of its side
    await page.setViewportSize({ width: 480, height: 800 })
    const bar = added.locator(".dv-hscroll")
    await expect(bar).toHaveCount(1)
    await expect(bar).toHaveAttribute("data-side", "1")
    await expect.poll(() => overflowsSideways(bar)).toBe(true)
  })

  simpleTest("Inline shows a lightly edited line as one row", async ({ page }) => {
    // the diff engine finds line 2 a similar pair: ` + 1` inserted after `state`
    const { preview, reducer } = await openReducerDiff(page)
    await preview.getByRole("button", { name: "Inline" }).click()
    await expect(preview.getByRole("button", { name: "Inline" })).toHaveAttribute("aria-pressed", "true")

    // the pair is one row: the new line with the deleted part of the old line before the inserted part
    const inlineRow = reducer.locator(".dv-row[data-kind=inline]")
    await expect(inlineRow).toHaveCount(1)
    // a pure insertion: the old line has no deleted part
    await expect(inlineRow).toContainText("settings: SessionSettings): Int = state + 1")
    await expect(inlineRow.locator("[data-inline-deleted]")).toHaveCount(0)

    // Unified shows the pair as two rows again
    await preview.getByRole("button", { name: "Unified" }).click()
    await expect(reducer.locator(".dv-row[data-kind=deleted]")).toHaveCount(1)
    await expect(reducer.locator(".dv-row[data-kind=added]")).toHaveCount(1)
    await expect(inlineRow).toHaveCount(0)
  })

  simpleTest("should break a long line with Soft Wraps on in Settings, and scroll it with them off", async ({ page, testStore }) => {
    const { reducer, deletions } = await openReducerDiff(page)
    // off by default, as in IntelliJ: the line keeps its shape and scrolls
    await expect.poll(() => overflowsSideways(deletions)).toBe(true)

    const settings = await openSettingsWindow(page, { page: "diffs" })
    const softWraps = settings.getByTestId("setting-softWraps")
    await expect(softWraps.getByRole("switch")).not.toBeChecked()
    await softWraps.click()
    await expect.poll(() => testStore.get("diffView")).toEqual({ softWraps: true })
    // the view breaks the line itself: its visual lines, and no scroll bar
    await expect.poll(() => reducer.locator(".dv-vl").count()).toBeGreaterThan(1)
    await expect(deletions).toHaveCount(0)

    await softWraps.click()
    await expect.poll(() => overflowsSideways(deletions)).toBe(true)
    await expect(reducer.locator(".dv-vl")).toHaveCount(0)
    await expect.poll(() => testStore.get("diffView")).toBeNull()
  })
})

/**
 * The diff of `Reducer.kt`, whose changed line is longer than a side of Split, and the horizontal scroll bars of the old
 * and the new side of Split, which have no role or name
 */
async function openReducerDiff(page: Page) {
  const { preview, tree } = await openTreePreview(page)
  const reducer = tree.getByRole("treeitem", { name: "Reducer.kt" })
  await reducer.getByText("Reducer.kt").click()
  await expect(reducer).toContainText("SessionSettings")
  return { preview, reducer, deletions: reducer.locator('.dv-hscroll[data-side="0"]'), additions: reducer.locator('.dv-hscroll[data-side="1"]') }
}

function overflowsSideways(column: Locator): Promise<boolean> {
  return column.evaluate(element => element.scrollWidth > element.clientWidth)
}

function scrollLeft(column: Locator): Promise<number> {
  return column.evaluate(element => element.scrollLeft)
}

simpleTest.describe("Whitespace of the diffs", () => {
  simpleTest.beforeEach(async ({ setupRepo, syncAndWaitForBranches }) => {
    // the `commit_whitespace` template: Format.kt indents two lines again, adds a blank line apart from them and changes
    // `3` to `4`; Main.kt changes one line
    await setupRepo("commit_whitespace")
    await syncAndWaitForBranches()
  })

  simpleTest("Ignore Whitespace hides an indentation-only change", async ({ page }) => {
    await expandBranch(page, findBranchRow(page, "format"))
    const preview = await openPreview(page, getBranchCommitRow(page, "format", "indent the block of first and return 4"))
    const tree = await openCommitFilesSection(preview)
    const format = tree.getByRole("treeitem", { name: "Format.kt" })
    await format.getByText("Format.kt").click()
    await expect(format).toContainText("fun second() = 4")
    const tinted = format.locator(".dv-half[data-tint]")
    const indentation = format.locator(".dv-row[data-kind=changed]", { hasText: "val a = 1" })
    const choose = (label: string) => chooseWhitespace(page, preview, label)

    // Do Not Ignore: the indentation, the blank line and `4` are changes. As in IntelliJ, the re-indented lines are pale
    // and their inner ranges, the indentation, carry the full colour
    await expect(format).toHaveAttribute("aria-description", "modified, +4 −3")
    await expect(indentation).toHaveCount(1)
    await expect(indentation.locator('.dv-half[data-tint="pale"]')).toHaveCount(2)
    await expect(indentation.locator('.dv-half[data-side="1"] .dv-changed')).toHaveText(/^ +$/)
    // the engine counts 10 lines in the 9 lines and the final line end of Format.kt: the empty 10th line does not show
    await expect(format.locator('.dv-half[data-side="1"] .dv-num', { hasText: /^9$/ })).toHaveCount(1)
    await expect(format.locator(".dv-num", { hasText: /^10$/ })).toHaveCount(0)

    // Trim Whitespace: the indentation is unchanged lines; the blank line stays a change, in the full colour, since it has
    // no inner ranges; `4` is the only tinted row, pale with `4` in the full colour
    await choose("Trim Whitespace")
    await expect(format).toHaveAttribute("aria-description", "modified, +2 −1")
    await expect(indentation).toHaveCount(0)
    const second = format.locator(".dv-row[data-kind=changed]", { hasText: "fun second()" })
    await expect(tinted).toHaveCount(2)
    await expect(second.locator('.dv-half[data-tint="pale"]')).toHaveCount(2)
    await expect(second.locator('.dv-half[data-side="1"] .dv-changed')).toHaveText("4")
    await expect(format).toContainText("val a = 1")
    // Ignore Whitespace: the blank line holds no character the engine compares, so it is a whitespace-only change, as
    // in IntelliJ: pale, and still counted
    await choose("Ignore Whitespace")
    await expect(format).toHaveAttribute("aria-description", "modified, +2 −1")
    await expect(indentation).toHaveCount(0)
    await expect(format.locator('.dv-row[data-kind=changed] .dv-half[data-side="1"][data-tint="whitespace"]')).toHaveCount(1)

    // and Blank Lines: the blank line is ignored, drawn as an unchanged line; only `4` is left
    await choose("Ignore Whitespace and Blank Lines")
    await expect(format).toHaveAttribute("aria-description", "modified, +1 −1")
    await expect(format.locator(".dv-row[data-kind=changed]")).toHaveCount(1)

    await choose("Do Not Ignore")
    await expect(format).toHaveAttribute("aria-description", "modified, +4 −3")
  })
})
