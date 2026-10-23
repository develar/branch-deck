// @spec spec/commit/commit-review.spec.md
import type { Locator, Page } from "@playwright/test"
import type { CommitReviewWindowData } from "../../../src/lib/commit-review-window"
import type { BranchReviewWindowData } from "../../../src/types/sub-window"
import { test as simpleTest, testWithIssueLinks, expect } from "../fixtures/test-fixtures"
import { activityButton, activityPanel } from "../helpers/activity-helpers"
import { findBranchRow, expandBranch, expectNothingToPush, getBranchCommitRow, getPushButton, getUnassignedCommitRow } from "../helpers/branch-helpers"
import { chooseWhitespace, getCommitPreview, openCommitFilesSection, openCommitPreview } from "../helpers/preview-helpers"
import { getOpenedSubWindows, getSubWindowReadyOpen, openLinkedSubWindow, openSubWindowPage, reopenLinkedSubWindow } from "../helpers/sub-window-helpers"
import type { OpenedSubWindow } from "../helpers/sub-window-helpers"
import { holdCommand, waitForStorePersistence } from "../helpers/wait-helpers"

// the `commit_tree` template: one commit of the `session` branch, five files in nested directories
const SUBJECT = "a prompt that waits for its turn keeps the session row running"

async function openTreePreview(page: Page): Promise<Locator> {
  await expandBranch(page, findBranchRow(page, "session"))
  return await openCommitPreview(page, getBranchCommitRow(page, "session", SUBJECT))
}

/**
 * The pixels from the top of the diffs to the header of the file `name`, and whether a row of the diffs shows in the
 * gap above it (the point 5 px below the top of the diffs, in the middle)
 */
async function headerPlace(review: Locator, name: string): Promise<{ header: number; rowInGap: boolean }> {
  return review.getByTestId("diff-view").evaluate((view, fileName) => {
    const box = view.getBoundingClientRect()
    const section = view.querySelector(`[data-testid=diff-view-file][data-path$="/${fileName}"]`)!
    const header = section.querySelector("[data-testid=review-file-header]")!.getBoundingClientRect()
    const inGap = document.elementFromPoint(box.left + box.width / 2, box.top + 5)
    return { header: Math.round(header.top - box.top), rowInGap: inGap?.closest(".dv-row") != null }
  }, name)
}

function selectedFile(review: Locator): Locator {
  return review.getByRole("tree", { name: "Changed files" }).locator("[role=treeitem][aria-selected=true]")
}

/** The `count`-th window the page opened (`open_sub_window`), once it is recorded */
async function openedWindow<T>(page: Page, count: number): Promise<OpenedSubWindow<T>> {
  await expect.poll(async () => (await getOpenedSubWindows(page)).length).toBe(count)
  return (await getOpenedSubWindows<T>(page))[count - 1]!
}

/** The `count`-th open, the review of a commit of a branch: the review window of the branch, at the commit */
async function openedReview(page: Page, count: number): Promise<OpenedSubWindow<BranchReviewWindowData>> {
  const opened = await openedWindow<BranchReviewWindowData>(page, count)
  expect(opened).toMatchObject({ windowId: "branch-review", url: "/branch-review" })
  return opened
}

/** The `count`-th open, the review window of a commit of no branch it can be reviewed with */
async function openedCommitReview(page: Page, count: number): Promise<OpenedSubWindow<CommitReviewWindowData>> {
  const opened = await openedWindow<CommitReviewWindowData>(page, count)
  expect(opened).toMatchObject({ windowId: "commit-review", url: "/commit-review" })
  return opened
}

/** Shows the review window of a branch in the page, as the app opens it */
async function showReviewWindow(page: Page, opened: OpenedSubWindow<BranchReviewWindowData>): Promise<Locator> {
  await openSubWindowPage(page, opened)
  const window = page.getByTestId("branch-review")
  await expect(window).toBeVisible()
  return window
}

/** The commit a review of a branch opens at: the copy of the commit in the virtual branch */
function focusedCommit(opened: OpenedSubWindow<BranchReviewWindowData>): string {
  const commitId = opened.data.focus.commitId
  expect(commitId).toMatch(/^[0-9a-f]{40}$/)
  return commitId!
}

/**
 * D in the preview of "perf db: increase pool size to 50" of JIRA-456: the `issue_links` template has three commits
 * there, each changing one file under db/, and this is the second
 */
async function reviewPoolSizeCommit(page: Page): Promise<OpenedSubWindow<BranchReviewWindowData>> {
  await expandBranch(page, findBranchRow(page, "JIRA-456"))
  await openCommitPreview(page, getBranchCommitRow(page, "JIRA-456", "perf db: increase pool size to 50"))
  await page.keyboard.press("d")
  return await openedReview(page, 1)
}

/** D on the preview of the tree commit: its review window, shown in the page */
async function openTreeReview(page: Page): Promise<Locator> {
  await openTreePreview(page)
  await page.keyboard.press("d")
  return showReviewWindow(page, await openedReview(page, 1))
}

/** The colour a `--syntax-*` token of the theme resolves to, as `getComputedStyle` gives a colour */
async function tokenColor(review: Locator, token: string): Promise<string> {
  return review.evaluate((element, name) => {
    const probe = document.createElement("span")
    probe.style.color = `var(${name})`
    element.append(probe)
    const color = getComputedStyle(probe).color
    probe.remove()
    return color
  }, token)
}

simpleTest.describe("Commit review", () => {
  simpleTest.beforeEach(async ({ setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("commit_tree")
    await syncAndWaitForBranches()
  })

  simpleTest("should colour Kotlin as IntelliJ does: a property as a field, a declaration, an annotation, a keyword", async ({ page }) => {
    const window = await openTreeReview(page)
    const code = window.getByTestId("commit-review-code")
    // the tree-sitter highlighting of the backend: what IntelliJ resolves and the syntax shows, `field1` a property
    await expect(code.getByText("field1", { exact: true }).first()).toHaveCSS("color", await tokenColor(window, "--syntax-field"))
    await expect(code.getByText("map", { exact: true }).first()).toHaveCSS("color", await tokenColor(window, "--syntax-function-declaration"))
    await expect(code.getByText("@Test", { exact: true })).toHaveCSS("color", await tokenColor(window, "--syntax-annotation"))
    await expect(code.getByText("fun", { exact: true }).first()).toHaveCSS("color", await tokenColor(window, "--syntax-keyword"))
  })

  simpleTest("should show the tree beside the diffs of all files and follow J, K, N, P and the tree", async ({ page }) => {
    const preview = await openTreePreview(page)
    await page.keyboard.press("d")
    // D in the preview: the review in its window, at the first file; the preview closes
    await expect(preview).toBeHidden()
    const opened = await openedReview(page, 1)
    expect(opened.title).toMatch(/^Review session · [0-9a-f]{8}$/)
    expect(opened.data).toMatchObject({ chain: ["session"], focus: { range: "own", path: null } })
    focusedCommit(opened)

    const window = await showReviewWindow(page, opened)
    // the branch of the commit in the title bar, with its actions
    await expect(window.getByTestId("branch-review-segment")).toHaveAttribute("data-name", "session")
    const position = window.getByTestId("branch-review-position")
    const code = window.getByTestId("commit-review-code")
    // the files in the order of the tree, the first one selected
    await expect(position).toHaveText("File 1 / 5")
    await expect(selectedFile(window)).toHaveAttribute("aria-label", "RowMapper.kt")
    await expect(code).toContainText("SessionRowMapper")
    // five files are read in one look: no search of them
    await expect(window.getByRole("textbox", { name: "Filter files" })).toHaveCount(0)
    // the header of a file names itself, then its directory, as every list of changed files does
    await expect(code.getByTestId("review-file-header").first()).toContainText("RowMapper.ktplugins/air/backend/session/catalog/src")
    // Expand All of the header shows every unchanged line of its file
    await expect(code).not.toContainText("val field10 = 10")
    await code.getByRole("button", { name: "Expand all unchanged lines of RowMapper.kt" }).click()
    await expect(code).toContainText("val field10 = 10")
    await expect(code.getByRole("button", { name: "Expand all unchanged lines of RowMapper.kt" })).toHaveCount(0)

    // a window low enough for the second file to reach the top
    const viewport = page.viewportSize()!
    await page.setViewportSize({ width: viewport.width, height: 420 })
    await code.locator("[aria-label='Changes of the commit']").focus()
    await page.keyboard.press("j")
    await expect(position).toHaveText("File 2 / 5")
    await expect(selectedFile(window)).toHaveAttribute("aria-label", "RowMapperTest.kt")
    // the card at the top keeps the gap above it that the first card has
    await expect.poll(() => headerPlace(window, "RowMapperTest.kt")).toEqual({ header: 10, rowInGap: false })
    // its header stuck while its rows scroll under it: the same gap, never the rows above it
    await window.getByTestId("diff-view").evaluate(view => view.scrollBy(0, 120))
    await expect.poll(() => headerPlace(window, "RowMapperTest.kt")).toEqual({ header: 10, rowInGap: false })
    await page.setViewportSize(viewport)
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
    await window.getByRole("button", { name: "Unified" }).click()
    await expect(window.getByRole("button", { name: "Unified" })).toHaveAttribute("aria-pressed", "true")
  })

  simpleTest("should give the sidebar the width of its names, and hide and show it with F and its button, the choice kept", async ({ page }) => {
    await page.setViewportSize({ width: 2000, height: 1000 })
    const window = await openTreeReview(page)
    const sidebar = window.getByTestId("review-sidebar")
    const toggle = window.getByTestId("review-sidebar-toggle")
    // 19rem in a maximized window, not a share of it
    await expect.poll(async () => Math.round((await sidebar.boundingBox())?.width ?? 0)).toBe(304)
    await expect(window.getByTestId("branch-review-footer")).toContainText("FSidebar")
    await expect(toggle).toHaveAttribute("aria-pressed", "true")

    await window.locator("[aria-label='Changes of the commit']").focus()
    await page.keyboard.press("f")
    await expect(sidebar).toHaveCount(0)
    await expect(toggle).toHaveAttribute("aria-pressed", "false")
    await waitForStorePersistence(page, "appSettings", settings => (settings as { reviewSidebar?: boolean } | undefined)?.reviewSidebar === false)
    // the choice stays for the next review: no window brings the sidebar back by itself
    await page.reload()
    await expect(page.getByTestId("branch-review").getByTestId("review-sidebar-toggle")).toHaveAttribute("aria-pressed", "false")
    await expect(page.getByTestId("review-sidebar")).toHaveCount(0)
    await page.getByTestId("review-sidebar-toggle").click()
    await expect(page.getByTestId("review-sidebar")).toBeVisible()
  })

  simpleTest("should drag the divider of the sides of Split, keep its share, and make the sides equal with a double click", async ({ page }) => {
    await page.setViewportSize({ width: 1600, height: 900 })
    const window = await openTreeReview(page)
    const row = window.locator(".dv-row[data-kind=changed]").first()
    await expect(row).toBeVisible()
    const share = () => row.evaluate(element => element.children[0]!.getBoundingClientRect().width / element.getBoundingClientRect().width)
    await expect.poll(share).toBeCloseTo(0.5, 2)
    // the divider is the line between the halves of a row
    const line = async () => {
      const box = (await row.locator('.dv-half[data-side="1"]').boundingBox())!
      return { x: box.x, y: box.y + box.height / 2 }
    }

    const at = await line()
    await page.mouse.move(at.x, at.y)
    await page.mouse.down()
    await page.mouse.move(at.x - 300, at.y, { steps: 5 })
    await page.mouse.up()
    const dragged = await share()
    expect(dragged).toBeLessThan(0.35)
    await waitForStorePersistence(page, "diffView", settings => Math.abs(((settings as { splitRatio?: number } | undefined)?.splitRatio ?? 0.5) - dragged) < 0.01)

    // IntelliJ's DiffSplitter: a double click makes the sides equal
    const moved = await line()
    await page.mouse.dblclick(moved.x, moved.y)
    await expect.poll(share).toBeCloseTo(0.5, 2)
  })

  simpleTest("should copy the lines of a selection of the diffs from the model, one side in Split, without the line numbers", async ({ page }) => {
    const window = await openTreeReview(page)
    const code = window.getByTestId("commit-review-code")
    const newSide = code.locator('.dv-half[data-side="1"] .dv-code[data-side]')
    await expect(newSide.nth(2)).toBeVisible()
    // a press on the new side keeps the selection there; the copy reads the lines of the rows from the texts
    await newSide.nth(0).click()
    const copied = await code.evaluate(element => {
      const cells = element.querySelectorAll('.dv-half[data-side="1"] .dv-code[data-side]')
      const range = document.createRange()
      range.setStart(cells[0]!, 0)
      range.setEnd(cells[2]!, cells[2]!.childNodes.length)
      document.getSelection()!.removeAllRanges()
      document.getSelection()!.addRange(range)
      const data = new DataTransfer()
      element.querySelector("[data-testid=diff-view]")!.dispatchEvent(new ClipboardEvent("copy", { clipboardData: data, bubbles: true, cancelable: true }))
      return { text: data.getData("text/plain"), lines: [...cells].slice(0, 3).map(cell => cell.textContent) }
    })
    expect(copied.text).toBe(copied.lines.join("\n"))
    expect(copied.text.split("\n")).toHaveLength(3)
  })

  simpleTest("should find a text in the diffs from the model, also in the folded lines, with ⌘F, Enter and Escape", async ({ page }) => {
    const window = await openTreeReview(page)
    const code = window.getByTestId("commit-review-code")
    await expect(code).toContainText("SessionRowMapper")
    await code.locator("[aria-label='Changes of the commit']").focus()

    await page.keyboard.press("ControlOrMeta+f")
    const field = window.getByRole("textbox", { name: "Find in the diffs" })
    await expect(field).toBeFocused()
    // line 11 of RowMapper.kt lies in a fold: only the find in the folded lines sees it
    await field.fill("field10")
    const count = window.getByTestId("diff-find-count")
    await expect(count).toHaveText("No results")
    await window.getByText("In folded lines").click()
    await expect(count).toHaveText("1 result")
    // Enter goes to the hit: its fold opens, the hit is marked
    await field.press("Enter")
    await expect(count).toHaveText("1 of 1")
    await expect(code.locator(".dv-hit-current")).toHaveText("field10")
    await expect(code).toContainText("val field10 = 10")

    // Escape closes the find, the diffs keep the focus
    await field.press("Escape")
    await expect(window.getByTestId("diff-find-bar")).toHaveCount(0)
    await expect(code.locator("[aria-label='Changes of the commit']")).toBeFocused()
    await expect(code.locator(".dv-hit")).toHaveCount(0)
  })

  simpleTest("should open the review at a file with Enter in the tree, and with the footer button", async ({ page }) => {
    const preview = await openTreePreview(page)
    await openCommitFilesSection(preview)
    await preview.getByRole("treeitem", { name: "plugins/air" }).focus()
    // plugins/air, backend/session, catalog, src, RowMapper.kt, testSrc, RowMapperTest.kt
    for (let i = 0; i < 6; i++) {
      await page.keyboard.press("ArrowDown")
    }
    await expect(preview.getByRole("treeitem", { name: "RowMapperTest.kt" })).toBeFocused()
    await page.keyboard.press("Enter")
    await expect(preview).toBeHidden()
    const atFile = await openedReview(page, 1)
    expect(atFile.data.focus.path).toBe("plugins/air/backend/session/catalog/testSrc/RowMapperTest.kt")

    // the footer button: the first file
    await openCommitPreview(page, getBranchCommitRow(page, "session", SUBJECT))
    await preview.getByTestId("commit-review-toggle").click()
    await expect(preview).toBeHidden()
    expect((await openedReview(page, 2)).data.focus.path).toBeNull()

    const window = await showReviewWindow(page, atFile)
    await expect(window.getByTestId("branch-review-position")).toHaveText("File 2 / 5")
    await expect(selectedFile(window)).toHaveAttribute("aria-label", "RowMapperTest.kt")
  })
})

simpleTest("should colour TSX as IntelliJ does: a name by its declaration, a component, a method, the brackets of a tag", async ({ page, setupRepo, syncAndWaitForBranches }) => {
  // the `script_colors` template: one commit of `colors` adds src/Counter.tsx
  await setupRepo("script_colors")
  await syncAndWaitForBranches()
  await expandBranch(page, findBranchRow(page, "colors"))
  await openCommitPreview(page, getBranchCommitRow(page, "colors", "add the counter"))
  await page.keyboard.press("d")
  const window = await showReviewWindow(page, await openedReview(page, 1))
  const code = window.getByTestId("commit-review-code")
  await expect(code).toContainText("useState(start)")
  const colorOf = (text: string) => code.getByText(text, { exact: true }).first()
  // an imported function the file calls, a parameter, a local variable, a component, a method at its call
  await expect(colorOf("useState")).toHaveCSS("color", await tokenColor(window, "--syntax-global-function"))
  await expect(colorOf("setCount")).toHaveCSS("color", await tokenColor(window, "--syntax-local-variable"))
  await expect(colorOf("Button")).toHaveCSS("color", await tokenColor(window, "--syntax-component-tag"))
  await expect(colorOf("toFixed")).toHaveCSS("color", await tokenColor(window, "--syntax-method"))
  // a parameter is underlined in the light scheme of the browser, as `JS.PARAMETER` of IntelliJ Light
  await expect(colorOf("start")).toHaveCSS("text-decoration-line", "underline")
  await expect(colorOf("</")).toHaveCSS("color", await tokenColor(window, "--syntax-tag-bracket"))
})

simpleTest("should strike a deleted file through, never its status letter, in the tree and above its diff", async ({ page, setupRepo, syncAndWaitForBranches }) => {
  // the `commit_deleted` template: one commit of `cleanup` deletes src/LegacyMapper.kt and changes src/Mapper.kt
  await setupRepo("commit_deleted")
  await syncAndWaitForBranches()
  await expandBranch(page, findBranchRow(page, "cleanup"))
  const preview = await openCommitPreview(page, getBranchCommitRow(page, "cleanup", "drop the legacy mapper"))
  await openCommitFilesSection(preview)

  const expectStruckName = async (file: Locator) => {
    await expect(file.getByText("LegacyMapper.kt", { exact: true })).toHaveCSS("text-decoration-line", "line-through")
    await expect(file.getByText("D", { exact: true })).toHaveCSS("text-decoration-line", "none")
  }
  // the tree of the preview, beside a modified file that is not struck through
  await expectStruckName(preview.getByRole("treeitem", { name: "LegacyMapper.kt" }))
  await expect(preview.getByRole("treeitem", { name: "Mapper.kt", exact: true }).getByText("Mapper.kt", { exact: true })).toHaveCSS("text-decoration-line", "none")

  // the header of its diff in the review window
  await page.keyboard.press("d")
  const window = await showReviewWindow(page, await openedReview(page, 1))
  await expectStruckName(window.locator("[data-testid=review-file-header][data-path='src/LegacyMapper.kt']"))
})

testWithIssueLinks("should keep the Activity panel the main window opened when a review window hides its sidebar right after", async ({ page }) => {
  await page.setViewportSize({ width: 2000, height: 1000 })
  // a push, so that the main window has activity to show
  const pushed = findBranchRow(page, "ABC-123")
  await getPushButton(pushed).click()
  await expectNothingToPush(pushed)
  const window = await openLinkedSubWindow(page, await reviewPoolSizeCommit(page))
  const toggle = window.getByTestId("review-sidebar-toggle")
  await expect(toggle).toHaveAttribute("aria-pressed", "true")

  // the main window opens its panel, and the review window hides its sidebar before it heard of the panel
  await activityButton(page).click()
  await expect(activityPanel(page)).toBeVisible()
  await toggle.click()
  await expect(toggle).toHaveAttribute("aria-pressed", "false")
  // the main window takes only the sidebar from the review window: its panel stays open, and both are saved
  await waitForStorePersistence(page, "appSettings", settings => {
    const saved = settings as { activityPanel?: boolean; reviewSidebar?: boolean } | undefined
    return saved?.reviewSidebar === false && saved.activityPanel === true
  })
  await expect(activityPanel(page)).toBeVisible()
})

testWithIssueLinks("should switch the commit of a review with Alt and the arrows", async ({ page }) => {
  const opened = await reviewPoolSizeCommit(page)
  // the branch, at the commit
  expect(opened.data).toMatchObject({ chain: ["JIRA-456"], focus: { range: "own", path: null } })
  focusedCommit(opened)

  const window = await showReviewWindow(page, opened)
  const position = window.getByTestId("branch-review-position")
  await expect(position).toHaveText("Commit 2 / 3 · File 1 / 1")
  await expect(window.getByTestId("range-review-commit-header")).toContainText("perf db: increase pool size to 50")
  await expect(window.getByTestId("commit-review-code")).toContainText("size: 50")

  await page.keyboard.press("Alt+ArrowDown")
  await expect(window.getByTestId("range-review-commit-header")).toContainText("Fix database connection pool timeout")
  await expect(position).toHaveText("Commit 3 / 3 · File 1 / 1")
  await expect(window.getByTestId("branch-review-footer")).toContainText("EscClose")
})

testWithIssueLinks("should show the files while the patch is read, settle with the diffs, and step to a commit read beside it", async ({ page }) => {
  const opened = await reviewPoolSizeCommit(page)
  // the window's own reads of diffs, and the diffs of its commit held
  const diffReads: unknown[] = []
  let release!: () => void
  const review = await openLinkedSubWindow(page, opened, {
    beforeLoad: async window => {
      window.on("request", request => {
        if (request.url().endsWith("/invoke/get_file_diffs")) {
          diffReads.push(request.postDataJSON())
        }
      })
      release = await holdCommand(window, "get_file_diffs")
    },
  })

  // the tree and the header of the file while the engine reads the diffs; the window waits for the diffs to show
  await expect(review.getByRole("tree", { name: "Changed files" })).toBeVisible()
  await expect(review.getByTestId("review-pending-files").getByTestId("review-file-header")).toHaveCount(1)
  expect(await getSubWindowReadyOpen(review)).toBeNull()

  release()
  await expect(review.getByTestId("commit-review-code")).toContainText("size: 50")
  await expect(review.getByTestId("review-pending-files")).toBeHidden()
  await expect.poll(() => getSubWindowReadyOpen(review)).toBe(1)

  // the commits before and after it were read with it: ⌥↓ shows the next one without a read
  await expect.poll(() => diffReads.length).toBe(3)
  await review.keyboard.press("Alt+ArrowDown")
  await expect(review.getByTestId("range-review-commit-header")).toContainText("Fix database connection pool timeout")
  await expect(review.getByTestId("commit-review-code")).toBeVisible()
  expect(diffReads).toHaveLength(3)
})

testWithIssueLinks("should review a commit of a row in its window with D and with the item of its menu", async ({ page }) => {
  await expandBranch(page, findBranchRow(page, "JIRA-456"))
  const row = getBranchCommitRow(page, "JIRA-456", "perf db: increase pool size to 50")
  const preview = getCommitPreview(page)

  // D over the row: its review in the window, no preview; the branch row under it was not reviewed
  await row.hover()
  await page.keyboard.press("d")
  const byKey = await openedReview(page, 1)
  expect(byKey.title).toMatch(/^Review JIRA-456 · [0-9a-f]{8}$/)
  const commitId = focusedCommit(byKey)
  await expect(preview).toBeHidden()

  // Review Changes, the only review item of the row's menu, first
  await row.click({ button: "right" })
  await expect(page.getByRole("menuitem").first()).toHaveText("Review ChangesD")
  await expect(page.getByRole("menuitem", { name: "Open in Window" })).toHaveCount(0)
  await page.getByRole("menuitem", { name: "Review Changes" }).click()
  expect((await openedReview(page, 2)).data.focus).toMatchObject({ commitId, path: null })
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
  await expect(preview.getByRole("heading", { level: 2 })).toHaveText("JIRA-456 Fix database connection pool timeout")

  // D: the window of the commit shown; the list has the focus again
  await page.keyboard.press("d")
  const shown = focusedCommit(await openedReview(page, 1))
  await expect(preview).toBeHidden()
  await expect(first.locator("xpath=ancestor::table[1]")).toBeFocused()
  // D and Space act on that commit, not on the clicked one
  await page.keyboard.press("d")
  expect(focusedCommit(await openedReview(page, 2))).toBe(shown)
  await page.keyboard.press("Space")
  await expect(preview.getByRole("heading", { level: 2 })).toHaveText("JIRA-456 Fix database connection pool timeout")

  // a click of a row is the anchor again
  await page.keyboard.press("Escape")
  await first.click()
  await page.mouse.move(1, 1)
  await page.keyboard.press("d")
  const clicked = await openedReview(page, 3)
  expect(focusedCommit(clicked)).not.toBe(shown)
  // the review shows the clicked one
  const window = await showReviewWindow(page, clicked)
  await expect(window.getByTestId("range-review-commit-header")).toContainText("perf db: increase pool size to 50")
})

simpleTest("should filter the files of a review by their path, with / from the diffs, Escape and Clear Filter", async ({ page, setupRepo, syncAndWaitForBranches }) => {
  // the `commit_many_files` template: one commit of `tools` with nine files, a source, a test and a README per module
  await setupRepo("commit_many_files")
  await syncAndWaitForBranches()
  await expandBranch(page, findBranchRow(page, "tools"))
  await openCommitPreview(page, getBranchCommitRow(page, "tools", "every tool reads its input as a stream"))
  await page.keyboard.press("d")
  const window = await showReviewWindow(page, await openedReview(page, 1))
  const search = window.getByRole("textbox", { name: "Filter files" })
  const tree = window.getByRole("tree", { name: "Changed files" })
  const files = tree.locator("[role=treeitem]:not([aria-expanded])")
  const code = window.getByTestId("commit-review-code")
  const diffs = code.locator("[aria-label='Changes of the commit']")
  const position = window.getByTestId("branch-review-position")
  const header = window.getByTestId("review-files-header")
  await expect(files).toHaveCount(9)
  await expect(header).toHaveText("Files9")

  // / from the diffs focuses the search; the letters of the review's keys (N, P) are typed, not pressed
  await diffs.focus()
  await page.keyboard.press("/")
  await expect(search).toBeFocused()
  await page.keyboard.type("printer")
  await expect(search).toHaveValue("printer")
  await expect(files).toHaveCount(3)
  await expect(header).toHaveText("Files3 of 9")
  await expect(position).toHaveText("File 1 / 3")
  await expect(code.getByTestId("review-file-header")).toHaveCount(3)

  // every word, in any part of the path
  await page.keyboard.type(" test")
  await expect(files).toHaveCount(1)
  await expect(files).toHaveAttribute("aria-label", "PrinterTest.kt")
  await expect(code.getByTestId("review-file-header")).toHaveCount(1)
  await expect(code).toContainText("class PrinterTest")

  // ↓ goes into the tree
  await page.keyboard.press("ArrowDown")
  await expect(files).toBeFocused()

  // Escape clears the text, Escape in the empty search gives the diffs the focus; the window stays
  await search.focus()
  await page.keyboard.press("Escape")
  await expect(search).toHaveValue("")
  await expect(files).toHaveCount(9)
  await expect(header).toHaveText("Files9")
  await page.keyboard.press("Escape")
  await expect(diffs).toBeFocused()
  await expect(window).toBeVisible()

  // a search that keeps nothing says so, Clear Filter shows all again
  await search.fill("nothing")
  await expect(window.getByTestId("review-files-no-match")).toContainText("No files match “nothing”")
  await expect(tree).toHaveCount(0)
  await window.getByRole("button", { name: "Clear Filter" }).click()
  await expect(search).toHaveValue("")
  await expect(files).toHaveCount(9)
})

simpleTest.describe("Commit review of a re-indented file", () => {
  simpleTest.beforeEach(async ({ setupRepo, syncAndWaitForBranches }) => {
    // the `commit_whitespace` template: Format.kt indents two lines again, adds a blank line apart from them and changes
    // `3` to `4`; Main.kt changes one line
    await setupRepo("commit_whitespace")
    await syncAndWaitForBranches()
  })

  simpleTest("N and P skip a block the whitespace policy ignores", async ({ page }) => {
    await expandBranch(page, findBranchRow(page, "format"))
    await openCommitPreview(page, getBranchCommitRow(page, "format", "indent the block of first and return 4"))
    await page.keyboard.press("d")
    const window = await showReviewWindow(page, await openedReview(page, 1))
    const position = window.getByTestId("branch-review-position")
    const code = window.getByTestId("commit-review-code")
    await expect(position).toHaveText("File 1 / 2")
    await expect(code).toContainText("fun second() = 4")
    /** From the top of Format.kt, N goes through `changes` blocks there, then to Main.kt; P comes back to the last one */
    const stepThrough = async (changes: number) => {
      await window.getByRole("treeitem", { name: "Format.kt" }).click()
      await expect(position).toHaveText("File 1 / 2")
      await code.locator("[aria-label='Changes of the commit']").focus()
      for (let step = 0; step < changes; step++) {
        await page.keyboard.press("n")
        await expect(position).toHaveText("File 1 / 2")
      }
      await page.keyboard.press("n")
      await expect(position).toHaveText("File 2 / 2")
      await page.keyboard.press("p")
      await expect(position).toHaveText("File 1 / 2")
    }
    const choose = async (label: string, additions: string) => {
      await chooseWhitespace(page, window, label)
      await expect(window.getByRole("treeitem", { name: "Format.kt" })).toHaveAttribute("aria-description", additions)
    }

    // Do Not Ignore: the indentation, the blank line and `4`
    await stepThrough(3)
    // Trim Whitespace: the indentation is an ignored block, so N passes it
    await choose("Trim Whitespace", "modified, +2 −1")
    await stepThrough(2)
    // Ignore Whitespace: the blank line is a whitespace-only change, pale, and N still stops at it, as in IntelliJ
    await choose("Ignore Whitespace", "modified, +2 −1")
    await stepThrough(2)
    // Ignore Whitespace and Blank Lines: the blank line is ignored, so N passes it
    await choose("Ignore Whitespace and Blank Lines", "modified, +1 −1")
    await stepThrough(1)
  })
})

simpleTest.describe("A commit with its branch", () => {
  // the `stack_push_alone` template: `perf-part-2`, one commit, stacked on `IJPL-257337`, two commits
  simpleTest.beforeEach(async ({ setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stack_push_alone")
    await syncAndWaitForBranches()
  })

  simpleTest("should review a commit of a stacked branch with its stack and its actions, and move to a commit of the chain", async ({ page }) => {
    await expandBranch(page, findBranchRow(page, "perf-part-2"))
    const own = getBranchCommitRow(page, "perf-part-2", "Cache the parsed icon")
    await own.hover()
    await page.keyboard.press("d")
    const opened = await openedReview(page, 1)
    expect(opened.data).toMatchObject({ chain: ["IJPL-257337", "perf-part-2"], move: false })
    focusedCommit(opened)

    const window = await openLinkedSubWindow(page, opened)
    // the stack of the commit's branch, the branch selected; its push in the title bar
    const segments = window.getByTestId("branch-review-segment")
    await expect(segments).toHaveCount(2)
    await expect(segments.nth(1)).toHaveAttribute("aria-pressed", "true")
    await expect(window.getByTestId("range-review-commit-header")).toContainText("Cache the parsed icon")
    await expect(window.getByTestId("branch-review-actions").getByRole("button", { name: /^Push/ })).toBeVisible()

    // From Root, then D on a commit of the parent in the main window: the window keeps the part, which has it
    await window.getByRole("button", { name: "From Root" }).click()
    await expect(window.getByRole("button", { name: "From Root" })).toHaveAttribute("aria-pressed", "true")
    await expandBranch(page, findBranchRow(page, "IJPL-257337"))
    await getBranchCommitRow(page, "IJPL-257337", "Load icons lazily").hover()
    await page.keyboard.press("d")
    const moved = await openedReview(page, 2)
    expect(moved.data).toMatchObject({ chain: ["IJPL-257337", "perf-part-2"], move: true, startedAt: opened.data.startedAt })
    await reopenLinkedSubWindow(window, moved)
    await expect(window.getByTestId("range-review-commit-header")).toContainText("Load icons lazily")
    await expect(window.getByRole("button", { name: "From Root" })).toHaveAttribute("aria-pressed", "true")

    // the own part of the child: a commit of the parent shows the parent's segment
    await segments.nth(1).click()
    await expect(window.getByRole("button", { name: "From Root" })).toHaveAttribute("aria-pressed", "false")
    await getBranchCommitRow(page, "IJPL-257337", "Parse the icon off the EDT").hover()
    await page.keyboard.press("d")
    await reopenLinkedSubWindow(window, await openedReview(page, 3))
    await expect(window.getByTestId("range-review-commit-header")).toContainText("Parse the icon off the EDT")
    await expect(segments.nth(0)).toHaveAttribute("aria-pressed", "true")
    await expect(segments.nth(1)).toHaveAttribute("aria-pressed", "false")
  })
})

simpleTest("should review an unassigned commit in the review window of the commit", async ({ page, setupRepo, syncAndWaitForBranches }) => {
  await setupRepo("single_unassigned")
  await syncAndWaitForBranches()
  await getUnassignedCommitRow(page, "Fix critical bug").hover()
  await page.keyboard.press("d")
  const opened = await openedCommitReview(page, 1)
  expect(opened.data).toMatchObject({ listLabel: "Unassigned", index: 0 })
})

simpleTest("should review a commit of a branch in conflict and of a blocked one in the review window of the commit", async ({ page, setupRepo, syncAndWaitForBranches }) => {
  // the `stacked_conflict` template: the copy of `auth` conflicts, so `login`, stacked on it, is blocked
  await setupRepo("stacked_conflict")
  await syncAndWaitForBranches()
  await expandBranch(page, findBranchRow(page, "login"))
  await getBranchCommitRow(page, "login", "Add login form").hover()
  await page.keyboard.press("d")
  expect((await openedCommitReview(page, 1)).data).toMatchObject({ listLabel: "login" })

  await expandBranch(page, findBranchRow(page, "auth"))
  await getBranchCommitRow(page, "auth", "Read the auth config").hover()
  await page.keyboard.press("d")
  expect((await openedCommitReview(page, 2)).data).toMatchObject({ listLabel: "auth" })
})
