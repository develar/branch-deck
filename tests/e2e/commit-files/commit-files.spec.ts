// @spec openspec/specs/commit/commit-files/spec.md
import type { Locator, Page } from "@playwright/test"
import { test as simpleTest, testWithIssueLinks, expect } from "../fixtures/test-fixtures"
import { findBranchRow, expandBranch, getBranchCommitRow } from "../helpers/branch-helpers"
import { readClipboard } from "../helpers/clipboard-helpers"
import { changedParts, deletedParts, diffLineNumbers, diffRows, foldButtons, horizontalBars, quietSides, remarks, visualLines } from "../helpers/diff-helpers"
import { chooseWhitespace, openCommitFilesSection, openCommitPreview as openPreview } from "../helpers/preview-helpers"
import { openPlanWindow, reviewSidebar, turnOnReviewPlan } from "../helpers/review-plan-helpers"
import { clickContextMenuItem, multiSelectCommit, openContextMenu, selectCommit } from "../helpers/selection-helpers"
import { openSettingsWindow } from "../helpers/settings-helpers"
import { SPLIT_DIFFS } from "../helpers/setup-options"
import { waitForBranchSyncComplete } from "../helpers/sync-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"

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
    // the layout of every diff view, Inline by default: the first of the toggle, then Unified and Split
    const layouts = files.getByRole("group", { name: "Diff layout" }).getByRole("button")
    await expect(layouts).toHaveText(["Inline", "Unified", "Split"])
    await expect.poll(() => layouts.evaluateAll(buttons => buttons.map(button => button.getAttribute("aria-pressed")))).toEqual(["true", "false", "false"])

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

/** Where the text of a path stands in its box: its left and right gaps (negative where it overflows), and whether it overflows */
async function pathBox(path: Locator): Promise<{ textLeft: number; textRight: number; overflows: boolean }> {
  return await path.evaluate(element => {
    const range = document.createRange()
    range.selectNodeContents(element)
    const text = range.getBoundingClientRect()
    const box = element.getBoundingClientRect()
    return { textLeft: text.left - box.left, textRight: box.right - text.right, overflows: element.scrollWidth > element.clientWidth }
  })
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

    // two changes with 30 unchanged lines between them, 4 shown next to each: the fold names the scope of its lines
    const mapper = tree.getByRole("treeitem", { name: "RowMapper.kt" })
    await mapper.getByText("RowMapper.kt").click()
    await expect(mapper).toContainText("SessionRowMapper")
    await expect(mapper).not.toContainText("field10")
    const fold = foldButtons(mapper)
    await expect(fold).toHaveAttribute("aria-label", "Expand 22 unchanged lines in SessionRowMapper")
    // the folds nest as IntelliJ's: 8 lines next to each change, then all
    await fold.click()
    await expect(fold).toHaveAttribute("aria-label", "Expand 14 unchanged lines in SessionRowMapper")
    await fold.click()
    await expect(mapper).toContainText("val field10 = 10")
    await expect(fold).toHaveCount(0)
  })

  simpleTest("Inline shows a lightly edited line as one row", async ({ page }) => {
    // the diff engine finds line 2 a similar pair: ` + 1` inserted after `state`
    const { preview, reducer } = await openReducerDiff(page)
    // the default, nothing chosen
    await expect(preview.getByRole("button", { name: "Inline" })).toHaveAttribute("aria-pressed", "true")

    // the pair is one row: the new line with the deleted part of the old line before the inserted part
    const inlineRow = diffRows(reducer, "inline")
    await expect(inlineRow).toHaveCount(1)
    // a pure insertion: the old line has no deleted part
    await expect(inlineRow).toContainText("settings: SessionSettings): Int = state + 1")
    await expect(deletedParts(inlineRow)).toHaveCount(0)

    // Unified shows the pair as two rows again
    await preview.getByRole("button", { name: "Unified" }).click()
    await expect(diffRows(reducer, "deleted")).toHaveCount(1)
    await expect(diffRows(reducer, "added")).toHaveCount(1)
    await expect(inlineRow).toHaveCount(0)
  })

  simpleTest("should break a long line with Soft Wraps on in Settings, and scroll it with them off", async ({ page, testStore }) => {
    // Inline, the default: the one column of the row has the scroll bar of the new side, and nothing is stored
    const { reducer, additions } = await openReducerDiff(page)
    await expectSoftWrapsBreakAndScroll(page, testStore, reducer, additions, {})
  })
})

simpleTest.describe("Commit files in Split", () => {
  simpleTest.beforeEach(async ({ setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("commit_tree", SPLIT_DIFFS)
    await syncAndWaitForBranches()
  })

  simpleTest("Split scrolls both sides together", async ({ page }) => {
    const { reducer, deletions, additions } = await openReducerDiff(page)
    await expect.poll(() => overflowsSideways(deletions)).toBe(true)

    // a wheel on the old side scrolls the new side to the same place, the line numbers stay
    await reducer.locator('[data-testid=diff-side][data-side="0"] [data-testid=diff-code]').first().hover()
    await page.mouse.wheel(160, 0)
    await expect.poll(() => scrollLeft(deletions)).toBeGreaterThan(0)
    await expect.poll(async () => (await scrollLeft(additions)) === (await scrollLeft(deletions))).toBe(true)
    // and back from the new side
    await reducer.locator('[data-testid=diff-side][data-side="1"] [data-testid=diff-code]').first().hover()
    await page.mouse.wheel(-40, 0)
    await expect.poll(async () => (await scrollLeft(deletions)) === (await scrollLeft(additions)) && (await scrollLeft(additions)) < 160).toBe(true)
  })

  simpleTest("Split shows an added file as its one side, with its line numbers, as IntelliJ's one-sided viewer", async ({ page }) => {
    const { tree, preview } = await openTreePreview(page)
    // the stored choice, over the Inline default
    await expect(preview.getByRole("button", { name: "Split" })).toHaveAttribute("aria-pressed", "true")
    const added = tree.getByRole("treeitem", { name: "RowMapperTest.kt" })
    await added.getByText("RowMapperTest.kt").click()
    // one column the width of the view, each row its line number and its line, inserted; no empty half beside it
    const rows = diffRows(added, "added")
    await expect(rows).toHaveCount(6)
    await expect(rows.first().locator("[data-testid=diff-side]")).toHaveCount(1)
    await expect(rows.first().locator("[data-testid=diff-line-number]")).toHaveText(["1"])
    await expect(rows.first().locator("[data-testid=diff-side]")).toHaveAttribute("data-cell", "inserted")
    // a line longer than the column keeps its horizontal scroll: the one column has the scroll bar of its side
    await page.setViewportSize({ width: 480, height: 800 })
    const bar = horizontalBars(added)
    await expect(bar).toHaveCount(1)
    await expect(bar).toHaveAttribute("data-side", "1")
    await expect.poll(() => overflowsSideways(bar)).toBe(true)
  })

  simpleTest("Split breaks a long line on each side with Soft Wraps on, and scrolls it with them off", async ({ page, testStore }) => {
    const { reducer, deletions } = await openReducerDiff(page)
    await expectSoftWrapsBreakAndScroll(page, testStore, reducer, deletions, { mode: "split" })
  })
})

/**
 * Soft Wraps of Settings → Diffs on and off again over the diff of `Reducer.kt`, whose long line `bar` scrolls: on, the
 * view breaks the line itself and has no scroll bar; `stored` is what the settings hold of the diff view beside it
 */
async function expectSoftWrapsBreakAndScroll(page: Page, testStore: { get: (key: string) => Promise<unknown> }, reducer: Locator, bar: Locator, stored: Record<string, unknown>) {
  // off by default, as in IntelliJ: the line keeps its shape and scrolls
  await expect.poll(() => overflowsSideways(bar)).toBe(true)

  const settings = await openSettingsWindow(page, { page: "diffs" })
  const softWraps = settings.getByTestId("setting-softWraps")
  await expect(softWraps.getByRole("switch")).not.toBeChecked()
  await softWraps.click()
  await expect.poll(() => testStore.get("diffView")).toEqual({ ...stored, softWraps: true })
  // the view breaks the line itself: its visual lines, and no scroll bar
  await expect.poll(() => visualLines(reducer).count()).toBeGreaterThan(1)
  await expect(bar).toHaveCount(0)

  await softWraps.click()
  await expect.poll(() => overflowsSideways(bar)).toBe(true)
  await expect(visualLines(reducer)).toHaveCount(0)
  // only what differs from the default is stored
  await expect.poll(() => testStore.get("diffView")).toEqual(Object.keys(stored).length === 0 ? null : stored)
}

/**
 * The diff of `Reducer.kt`, whose changed line is longer than a side of Split, and the horizontal scroll bars of the
 * sides, which have no role or name: the old and the new one in Split, the new one alone in the one column of Inline
 * and Unified
 */
async function openReducerDiff(page: Page) {
  const { preview, tree } = await openTreePreview(page)
  const reducer = tree.getByRole("treeitem", { name: "Reducer.kt" })
  await reducer.getByText("Reducer.kt").click()
  await expect(reducer).toContainText("SessionSettings")
  return { preview, reducer, deletions: horizontalBars(reducer, 0), additions: horizontalBars(reducer, 1) }
}

function overflowsSideways(column: Locator): Promise<boolean> {
  return column.evaluate(element => element.scrollWidth > element.clientWidth)
}

function scrollLeft(column: Locator): Promise<number> {
  return column.evaluate(element => element.scrollLeft)
}

/**
 * The diff of Format.kt of the `commit_whitespace` template in the preview of its commit: it indents two lines again,
 * adds a blank line apart from them and changes `3` to `4`; Main.kt changes one line. `file`: the same change in a file
 * of that name, committed by `commitPlainFormat`
 */
async function openFormatDiff(page: Page, file = "Format.kt", branch = "format") {
  await expandBranch(page, findBranchRow(page, branch))
  const preview = await openPreview(page, getBranchCommitRow(page, branch, "indent the block of first and return 4"))
  const tree = await openCommitFilesSection(preview)
  const format = tree.getByRole("treeitem", { name: file })
  await format.getByText(file).click()
  await expect(format).toContainText("fun middle() = 0")
  return { preview, tree, format, choose: (label: string) => chooseWhitespace(page, preview, label) }
}

/**
 * The change of Format.kt of the `commit_whitespace` template in `src/format.txt`, a file of no grammar, in a commit of
 * the branch `text`: its tokens cannot be read, so its change of indentation is no quiet run of the marks and shows as
 * IntelliJ shows it
 */
async function commitPlainFormat(repo: TestRepositoryBuilder): Promise<void> {
  const before = "class Format {\n  fun first() {\n    val a = 1\n    val b = 2\n  }\n  fun middle() = 0\n  fun second() = 3\n}\n"
  const after = "class Format {\n  fun first() {\n      val a = 1\n      val b = 2\n  }\n\n  fun middle() = 0\n  fun second() = 4\n}\n"
  await repo.commit("Add the format as text", { "src/format.txt": before }, 1704120600)
  await repo.pushInTerminal("master")
  await repo.commit("(text) indent the block of first and return 4", { "src/format.txt": after }, 1704121200)
}

simpleTest(
  "Inline shows a re-indented line of a file without a grammar as one pale row, its indentation in the full colour, and the whitespace policy hides it",
  async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await commitPlainFormat(await setupRepo("simple"))
    await syncAndWaitForBranches()
    const { format, choose } = await openFormatDiff(page, "format.txt", "text")
    const pairs = diffRows(format, "inline")
    const added = diffRows(format, "added")

    // Do Not Ignore: the two re-indented lines and the line of `second` are similar pairs, a row each; the blank line is
    // an added row. A re-indented row is pale, its inserted indentation in the full colour, nothing deleted in it. No
    // grammar reads the file, so the diff says nothing of its lines: no note, no quiet line
    await expect(pairs).toHaveCount(3)
    const indentation = pairs.filter({ hasText: "val a = 1" })
    await expect(indentation.locator('[data-testid=diff-side][data-tint="pale"]')).toHaveCount(1)
    await expect(changedParts(indentation)).toHaveText(/^ +$/)
    await expect(deletedParts(indentation)).toHaveCount(0)
    await expect(added).toHaveCount(1)
    await expect(remarks(format)).toHaveCount(0)
    await expect(format.getByTestId("file-to-read")).toHaveCount(0)
    // the row of `second`: the deleted `3` struck through before the inserted `4`
    const second = pairs.filter({ hasText: "fun second()" })
    await expect(deletedParts(second)).toHaveText("3")
    await expect(changedParts(second)).toHaveText("4")

    // Trim Whitespace: the indentation is unchanged lines; the blank line and `4` stay
    await choose("Trim Whitespace")
    await expect(format).toHaveAttribute("aria-description", "modified, +2 −1")
    await expect(pairs).toHaveCount(1)
    await expect(format).toContainText("val a = 1")
    await expect(added.locator("[data-testid=diff-side][data-tint]")).toHaveCount(0)
    // Ignore Whitespace: the blank line is a whitespace-only change, pale
    await choose("Ignore Whitespace")
    await expect(added.locator('[data-testid=diff-side][data-tint="whitespace"]')).toHaveCount(1)
    // and Blank Lines: the blank line is ignored, drawn as an unchanged line; the row of `second` is the one change
    await choose("Ignore Whitespace and Blank Lines")
    await expect(format).toHaveAttribute("aria-description", "modified, +1 −1, 1 line hidden by Whitespace")
    await expect(added).toHaveCount(0)
    await expect(pairs).toHaveCount(1)
  },
)

simpleTest(
  "should start the path of a directory at its icon and cut a long one from its start, its last folder kept",
  { tag: "@webkit" },
  async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("simple")
    // one directory a word long, one whose single folders merge into a path wider than the row
    const deep =
      "community/platform/platform-impl/src/com/intellij/openapi/application/impl/processWatcher/daemon/applications/uri-handler/src/main/kotlin/com/jetbrains/daemon/watcher"
    await repo.commit("(paths) watch the daemon", { "src/App.kt": "class App\n", [`${deep}/Watcher.kt`]: "class Watcher\n" }, 1704120600)
    await syncAndWaitForBranches()
    await expandBranch(page, findBranchRow(page, "paths"))
    const tree = await openCommitFilesSection(await openPreview(page, getBranchCommitRow(page, "paths", "watch the daemon")))

    // a short path starts at the left of its box, which runs to the counts of the row: never by the counts
    const short = tree.getByRole("treeitem", { name: "src", exact: true }).getByTestId("review-directory-path")
    await expect(short).toHaveText("src")
    const shortBox = await pathBox(short)
    expect(shortBox.overflows).toBe(false)
    expect(Math.abs(shortBox.textLeft)).toBeLessThanOrEqual(1)
    expect(shortBox.textRight).toBeGreaterThan(50)

    // a path wider than its row gives way from its start: its end, the last folder, stays at the right of its box
    const long = tree.getByRole("treeitem", { name: deep, exact: true }).getByTestId("review-directory-path")
    await expect(long).toHaveText(deep)
    const longBox = await pathBox(long)
    expect(longBox.overflows).toBe(true)
    expect(longBox.textLeft).toBeLessThan(0)
    // WebKit rounds the edge of the text to a whole pixel
    expect(Math.abs(longBox.textRight)).toBeLessThanOrEqual(1)
  },
)

simpleTest("Inline scrolls a row of a similar pair as far as its deleted part reaches, past the widest line of both texts", async ({ page, setupRepo, syncAndWaitForBranches }) => {
  const repo = await setupRepo("simple")
  // one long line whose last name changes: its row in Inline holds both names, so it is wider than either line
  const line = (timeout: string) => `  fun reduce(state: Int, event: SessionEvent, clock: Clock, log: Logger, metrics: Metrics, timeout: ${timeout}): Int = state\n`
  await repo.commit("Add the reducer", { "src/Reducer.kt": `class Reducer {\n${line("FallbackTimeoutMillisecondsOfTheSession")}}\n` }, 1704120600)
  await repo.pushInTerminal("master")
  await repo.commit("(wide) rename the timeout", { "src/Reducer.kt": `class Reducer {\n${line("DefaultRetryDelayMillisecondsOfTheSession")}}\n` }, 1704121200)
  await syncAndWaitForBranches()
  await expandBranch(page, findBranchRow(page, "wide"))
  const preview = await openPreview(page, getBranchCommitRow(page, "wide", "rename the timeout"))
  const tree = await openCommitFilesSection(preview)
  const reducer = tree.getByRole("treeitem", { name: "Reducer.kt" })
  await reducer.getByText("Reducer.kt").click()
  const pair = diffRows(reducer, "inline")
  await expect(deletedParts(pair)).toHaveText("FallbackTimeoutMillisecondsOfTheSession")

  /** How far the end of the text of the row is past the right edge of its column, in px */
  const cutOff = () =>
    pair.evaluate(row => {
      const text = document.createRange()
      text.selectNodeContents(row.querySelector("[data-testid=diff-code][data-side]")!)
      return Math.round(text.getBoundingClientRect().right - row.getBoundingClientRect().right)
    })
  // the row is wider than its column, and the scroll bar of the column reaches its end
  await expect.poll(cutOff).toBeGreaterThan(0)
  const bar = horizontalBars(reducer)
  await expect(bar).toHaveCount(1)
  await bar.evaluate(element => {
    element.scrollLeft = element.scrollWidth
  })
  await expect.poll(cutOff).toBeLessThanOrEqual(0)
  // Unified shows each line whole on a row of its own: the scroll reaches the end of the longer one
  await preview.getByRole("button", { name: "Unified" }).click()
  await expect(diffRows(reducer, "inline")).toHaveCount(0)
  await expect(bar).toHaveCount(1)
})

simpleTest.describe("Whitespace of the diffs in Split", () => {
  simpleTest.beforeEach(async ({ setupRepo, syncAndWaitForBranches }) => {
    // Split stored: it colours a block by its type on both sides
    await setupRepo("commit_whitespace", SPLIT_DIFFS)
    await syncAndWaitForBranches()
  })

  simpleTest("Ignore Whitespace hides an indentation-only change, and the counts say how many lines the policy hides", async ({ page }) => {
    const { tree, format, choose } = await openFormatDiff(page)
    await expect(format).toContainText("fun second() = 4")
    const tinted = format.locator("[data-testid=diff-side][data-tint]")
    const indentation = diffRows(format, "changed", { hasText: "val a = 1" })

    // Do Not Ignore: the indentation, the blank line and `4` are changes. The tokens of the Kotlin file prove the
    // indentation a change of formatting alone: its lines are quiet on both sides, nothing in them strong, and two of
    // the three changes are left to read
    await expect(format).toHaveAttribute("aria-description", "modified, +4 −3, 2 of 3 changes to read")
    // the policy ignores no block: the counts have no mark
    const hidden = format.getByTestId("file-hidden-lines")
    await expect(hidden).toHaveCount(0)
    await expect(indentation).toHaveCount(1)
    await expect(quietSides(indentation)).toHaveCount(2)
    await expect(remarks(format, "formatting")).toHaveText("Formatting only · 2 lines")
    // the two lines of the run have no changed part; the line of `second` has its `4`
    await expect(changedParts(diffRows(format, "changed"))).toHaveText(["3", "4"])
    // the engine counts 10 lines in the 9 lines and the final line end of Format.kt: the empty 10th line does not show
    await expect(format.locator('[data-testid=diff-side][data-side="1"] [data-testid=diff-line-number]', { hasText: /^9$/ })).toHaveCount(1)
    await expect(diffLineNumbers(format, { hasText: /^10$/ })).toHaveCount(0)

    // Trim Whitespace: the indentation is unchanged lines; the blank line stays a change, in the full colour, since it has
    // no inner ranges; `4` is the only tinted row, pale with `4` in the full colour
    await choose("Trim Whitespace")
    await expect(format).toHaveAttribute("aria-description", "modified, +2 −1")
    await expect(indentation).toHaveCount(0)
    await expect(remarks(format)).toHaveCount(0)
    const second = diffRows(format, "changed", { hasText: "fun second()" })
    await expect(tinted).toHaveCount(2)
    await expect(second.locator('[data-testid=diff-side][data-tint="pale"]')).toHaveCount(2)
    await expect(second.locator('[data-testid=diff-side][data-side="1"] [data-change]')).toHaveText("4")
    await expect(format).toContainText("val a = 1")
    // Ignore Whitespace: the blank line holds no character the engine compares, so it is a whitespace-only change, as
    // in IntelliJ: pale, and still counted
    await choose("Ignore Whitespace")
    await expect(format).toHaveAttribute("aria-description", "modified, +2 −1")
    await expect(indentation).toHaveCount(0)
    await expect(format.locator('[data-testid=diff-row][data-kind=changed] [data-testid=diff-side][data-side="1"][data-tint="whitespace"]')).toHaveCount(1)

    // and Blank Lines: the blank line is ignored, drawn as an unchanged line; only `4` is left. The counts mark the line
    // the policy hides, as their hover and the description of the row say; the indentation the policies above compare
    // as equal is no block, so no count of them marks a line
    await choose("Ignore Whitespace and Blank Lines")
    await expect(format).toHaveAttribute("aria-description", "modified, +1 −1, 1 line hidden by Whitespace")
    await expect(hidden).toHaveAttribute("title", "1 line hidden by Whitespace")
    await expect(diffRows(format, "changed")).toHaveCount(1)
    // the directory of the two files sums their lines under the policy, and marks the line hidden in Format.kt
    const src = tree.getByRole("treeitem", { name: "src", exact: true })
    await expect(src).toHaveAttribute("aria-description", "2 files, +2 −2, 1 line hidden by Whitespace")
    await expect(src.getByTestId("directory-line-counts")).toHaveAttribute("title", "1 line hidden by Whitespace")

    await choose("Do Not Ignore")
    await expect(format).toHaveAttribute("aria-description", "modified, +4 −3, 2 of 3 changes to read")
    await expect(hidden).toHaveCount(0)
    await expect(src).toHaveAttribute("aria-description", "2 files, +5 −4")
    await expect(src.getByTestId("directory-line-counts")).not.toHaveAttribute("title")
  })
})

simpleTest.describe("Whitespace of the diffs in the review of a branch", () => {
  // the `review_whitespace` template: the branch `spacing` of two commits with bodies, a step each; the first adds a blank
  // line apart from a real change in Format.kt and changes Main.kt, the second adds Other.kt
  simpleTest(
    "the row of a step, its commit, marks the lines the whitespace policy hides in its files once their diffs are read",
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const repo = await setupRepo("review_whitespace")
      turnOnReviewPlan(repo)
      const window = await openPlanWindow(page, syncAndWaitForBranches, "spacing")
      const { tree } = reviewSidebar(window)
      const step = tree.getByRole("treeitem", { name: "1 · feat(format): the second format returns 4", exact: true })
      const other = tree.getByRole("treeitem", { name: "2 · feat(other): the other format", exact: true })
      const format = tree.getByRole("treeitem", { name: "src/Format.kt", exact: true })
      const counts = step.getByTestId("directory-line-counts")
      const choose = (label: string) => chooseWhitespace(window, window.getByTestId("review-window"), label)

      // Do Not Ignore: the blank line is a change, the policy hides nothing, so the counts of the step have no mark
      await expect(format).toHaveAttribute("aria-description", "modified, +2 −1")
      await expect(step).toHaveAttribute("aria-description", /^2 files, \+3 −2, commit [0-9a-f]{7,}$/)
      await expect(counts).not.toHaveAttribute("title")

      // Ignore Whitespace and Blank Lines: the blank line of Format.kt is hidden; the row of its step sums the lines of its
      // files under the policy and says, as the row of the file does, how many the policy hides
      await choose("Ignore Whitespace and Blank Lines")
      await expect(format).toHaveAttribute("aria-description", "modified, +1 −1, 1 line hidden by Whitespace")
      await expect(step).toHaveAttribute("aria-description", /^2 files, \+2 −2, 1 line hidden by Whitespace, commit [0-9a-f]{7,}$/)
      await expect(counts).toHaveText("+2 −2")
      await expect(counts).toHaveAttribute("title", "1 line hidden by Whitespace")
      // the step of Other.kt hides no line: no mark
      await expect(other).toHaveAttribute("aria-description", /^1 file, \+3 −0, commit [0-9a-f]{7,}$/)
      await expect(other.getByTestId("directory-line-counts")).not.toHaveAttribute("title")

      await choose("Do Not Ignore")
      await expect(step).toHaveAttribute("aria-description", /^2 files, \+3 −2, commit [0-9a-f]{7,}$/)
      await expect(counts).not.toHaveAttribute("title")
    },
  )
})
