// @spec openspec/specs/commit/commit-review/spec.md
import type { Locator, Page } from "@playwright/test"
import type { SettingsChange } from "../../../src/lib/bindings"
import type { BranchReviewWindowData, CommitsReviewWindowData } from "../../../src/types/sub-window"
import { test as simpleTest, testWithIssueLinks, testWithUnassigned, expect } from "../fixtures/test-fixtures"
import { activityButton, activityPanel } from "../helpers/activity-helpers"
import { findBranchRow, expandBranch, expectNothingToPush, getBranchCommitRow, getPushButton, getUnassignedCommitRow } from "../helpers/branch-helpers"
import { mapPointOf } from "../helpers/file-map-helpers"
import { chooseWhitespace, getCommitPreview, openCommitFilesSection, openCommitPreview } from "../helpers/preview-helpers"
import { openedReview } from "../helpers/review-window-helpers"
import { multiSelectCommit, selectCommit } from "../helpers/selection-helpers"
import { SPLIT_DIFFS } from "../helpers/setup-options"
import {
  getEmittedEvents,
  getOpenedSubWindows,
  getSubWindowReadyOpen,
  getWindowShell,
  openLinkedSubWindow,
  openSubWindowPage,
  reopenLinkedSubWindow,
} from "../helpers/sub-window-helpers"
import type { OpenedSubWindow } from "../helpers/sub-window-helpers"
import { emitTauriEvent, waitForTauriListener } from "../helpers/sync-helpers"
import { holdCommand, waitForStorePersistence, expectClosed } from "../helpers/wait-helpers"

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

/** The left edge of the name in each header of the diffs, in their order */
async function headerNameLefts(code: Locator): Promise<number[]> {
  const lefts: number[] = []
  for (const header of await code.getByTestId("review-file-header").all()) {
    const path = (await header.getAttribute("data-path"))!
    const box = await header.getByText(path.slice(path.lastIndexOf("/") + 1), { exact: true }).boundingBox()
    lefts.push(Math.round(box!.x))
  }
  return lefts
}

/** The text of the elements of `parts` in their order: a hit or a deleted part of a line is a span per token of the syntax */
async function joinedText(parts: Locator): Promise<string> {
  return (await parts.allTextContents()).join("")
}

/**
 * A copy of the selection from the start of the first to the end of the last of the code cells `cells` in `scope`
 * (a part of the diffs): the text the view puts on the clipboard, and the text of each of those cells
 */
async function copyOfCells(scope: Locator, cells: string, count: number): Promise<{ text: string; lines: string[] }> {
  return scope.evaluate(
    (element, [selector, last]) => {
      const cells = [...element.querySelectorAll(selector)].slice(-last)
      const range = document.createRange()
      range.setStart(cells[0]!, 0)
      range.setEnd(cells.at(-1)!, cells.at(-1)!.childNodes.length)
      document.getSelection()!.removeAllRanges()
      document.getSelection()!.addRange(range)
      const data = new DataTransfer()
      element.closest("[data-testid=diff-view]")!.dispatchEvent(new ClipboardEvent("copy", { clipboardData: data, bubbles: true, cancelable: true }))
      return { text: data.getData("text/plain"), lines: cells.map(cell => cell.textContent ?? "") }
    },
    [cells, count] as const,
  )
}

/** ⌘F in the diffs of the review window of the tree commit, once they show: the field of the find and its count */
async function openFind(page: Page, window: Locator): Promise<{ code: Locator; field: Locator; count: Locator }> {
  const code = window.getByTestId("commit-review-code")
  await expect(code).toContainText("SessionRowMapper")
  await code.locator("[aria-label='Changes of the commit']").focus()
  await page.keyboard.press("ControlOrMeta+f")
  const field = window.getByRole("textbox", { name: "Find in the review" })
  await expect(field).toBeFocused()
  return { code, field, count: window.getByTestId("diff-find-count") }
}

function selectedFile(review: Locator): Locator {
  return review.getByRole("tree", { name: "Changed files" }).locator("[role=treeitem][aria-selected=true]")
}

/** RowMapperTest.kt, the second file of the tree commit, shown as J shows it: the footer and the tree follow, its header at the top below the gap */
async function expectSecondFileShown(review: Locator): Promise<void> {
  await expect(review.getByTestId("review-position")).toHaveText("File 2 / 5")
  await expect(selectedFile(review)).toHaveAttribute("aria-label", "RowMapperTest.kt")
  // the card at the top keeps the gap above it that the first card has
  await expect.poll(() => headerPlace(review, "RowMapperTest.kt")).toEqual({ header: 10, rowInGap: false })
}

/** Shows the review window of a branch in the page, as the app opens it */
async function showReviewWindow(page: Page, opened: OpenedSubWindow<BranchReviewWindowData>): Promise<Locator> {
  await openSubWindowPage(page, opened)
  const window = page.getByTestId("review-window")
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

/** The review window of the tree commit in a window low enough for the second file to reach the top */
async function openLowTreeReview(page: Page): Promise<Locator> {
  const viewport = page.viewportSize()!
  await page.setViewportSize({ width: viewport.width, height: 420 })
  return openTreeReview(page)
}

/**
 * Opens the review of the pool size commit in a linked window, once its page listens to the writer of the settings
 * (`settings_changed`): what the main window changes from now on reaches it as a change
 */
async function openSettledReviewWindow(page: Page): Promise<Page> {
  const window = await openLinkedSubWindow(page, await reviewPoolSizeCommit(page))
  await waitForTauriListener(window, "settings_changed")
  await expect(window.getByTestId("review-sidebar-toggle")).toHaveAttribute("aria-pressed", "true")
  return window
}

/** The next patch of `appSettings` the window sends the writer (the request of its `set_setting`) */
async function nextAppSettingsPatch(window: Page): Promise<unknown> {
  const isAppSettings = (body: { patch?: { key?: string } } | null) => body?.patch?.key === "appSettings"
  const request = await window.waitForRequest(request => request.url().endsWith("/invoke/set_setting") && isAppSettings(request.postDataJSON()))
  return (request.postDataJSON() as { patch: unknown }).patch
}

/**
 * The last change of `appSettings` the backend sent for a change of the window `page` (`settings_changed`), once its
 * value has `fields`: the window shows its own change before the writer answers it
 */
async function appSettingsChange(page: Page, fields: Record<string, unknown>): Promise<SettingsChange> {
  const last = async () => (await getEmittedEvents<SettingsChange>(page, "settings_changed")).findLast(it => it.key === "appSettings")
  await expect.poll(async () => (await last())?.value).toMatchObject(fields)
  return (await last())!
}

/** Hides the sidebar of the review window, whose patch of `appSettings` names that field alone */
async function hideSidebarSendingOnlyIt(window: Page): Promise<void> {
  const toggle = window.getByTestId("review-sidebar-toggle")
  const patch = nextAppSettingsPatch(window)
  await toggle.click()
  await expect(toggle).toHaveAttribute("aria-pressed", "false")
  expect(await patch).toEqual({ kind: "fields", key: "appSettings", fields: { reviewSidebar: false } })
}

/** The zoom the page gave its webview, 1 if none */
function webviewZoom(page: Page): Promise<number> {
  return page.evaluate(() => (window as unknown as { __e2eWebviewZoom?: number }).__e2eWebviewZoom ?? 1)
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
    // the highlighting of the backend: what IntelliJ resolves and the syntax shows, `field1` a property
    await expect(code.getByText("field1", { exact: true }).first()).toHaveCSS("color", await tokenColor(window, "--syntax-field"))
    await expect(code.getByText("map", { exact: true }).first()).toHaveCSS("color", await tokenColor(window, "--syntax-function-declaration"))
    await expect(code.getByText("@Test", { exact: true })).toHaveCSS("color", await tokenColor(window, "--syntax-annotation"))
    await expect(code.getByText("fun", { exact: true }).first()).toHaveCSS("color", await tokenColor(window, "--syntax-keyword"))
  })

  simpleTest("should show the tree beside the diffs of all files and follow J, K, N, P and the tree", async ({ page }) => {
    const preview = await openTreePreview(page)
    await page.keyboard.press("d")
    // D in the preview: the review in its window, at the first file; the preview closes
    await expectClosed(preview)
    const opened = await openedReview(page, 1)
    expect(opened.title).toMatch(/^Review session · [0-9a-f]{8}$/)
    expect(opened.data).toMatchObject({ chain: ["session"], focus: { range: "own", path: null } })
    focusedCommit(opened)

    const window = await showReviewWindow(page, opened)
    // the branch of the commit in the title bar, with its actions
    await expect(window.getByTestId("branch-review-segment")).toHaveAttribute("data-name", "session")
    const position = window.getByTestId("review-position")
    const code = window.getByTestId("commit-review-code")
    // the files in the order of the tree, the first one selected
    await expect(position).toHaveText("File 1 / 5")
    await expect(selectedFile(window)).toHaveAttribute("aria-label", "RowMapper.kt")
    await expect(code).toContainText("SessionRowMapper")
    // five files are read in one look: no search of them
    await expect(window.getByRole("textbox", { name: "Filter files" })).toHaveCount(0)
    // the header of a file names itself, then its directory, as every list of changed files does
    await expect(code.getByTestId("review-file-header").first()).toContainText("RowMapper.ktplugins/air/backend/session/catalog/src")
    // the name of each file at one x: nothing in a header stands before it but the chevron and the letter
    await expect(code.getByTestId("review-file-header")).toHaveCount(5)
    await expect.poll(async () => new Set(await headerNameLefts(code)).size).toBe(1)
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
    await expectSecondFileShown(window)
    // its header in its place casts no shadow
    const testHeader = code.locator('[data-testid=review-file-header][data-path$="/RowMapperTest.kt"]')
    await expect(testHeader).toHaveCSS("box-shadow", "none")
    // its header stuck while its rows scroll under it: the same gap, never the rows above it, and a shadow under it
    await window.getByTestId("diff-view").evaluate(view => view.scrollBy(0, 120))
    await expect.poll(() => headerPlace(window, "RowMapperTest.kt")).toEqual({ header: 10, rowInGap: false })
    await expect(testHeader).not.toHaveCSS("box-shadow", "none")
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

  simpleTest("should end a long card with a line naming the next file, which a click shows as J does, and the last card with the end of the review", async ({ page }) => {
    const window = await openLowTreeReview(page)
    const code = window.getByTestId("commit-review-code")
    const position = window.getByTestId("review-position")
    const mapper = code.locator('[data-testid=diff-view-file][data-path$="/RowMapper.kt"]')
    await expect(position).toHaveText("File 1 / 5")
    // its two changes, the 30 lines between them folded: a short card, whose end the header is near, has no line
    await expect(mapper.getByTestId("review-file-header")).toBeVisible()
    await expect(mapper.getByTestId("review-card-end")).toHaveCount(0)

    // all its lines shown, it is a long card: its end names the next file as its header does, its name and its
    // directory, and the key that shows it
    await code.getByRole("button", { name: "Expand all unchanged lines of RowMapper.kt" }).click()
    const next = mapper.getByRole("button", { name: "Next: RowMapperTest.kt plugins/air/backend/session/catalog/testSrc", exact: true })
    await expect(next).toHaveText("Next:RowMapperTest.ktplugins/air/backend/session/catalog/testSrcJ")
    // never in a copy of the code
    await expect(mapper.getByTestId("diff-view-footer")).toHaveCSS("user-select", "none")
    // a click shows the next file as J does: its header at the top, the tree and the footer follow, the focus in the
    // diffs, so J goes on from there
    await next.click()
    await expectSecondFileShown(window)
    await expect(code.locator("[aria-label='Changes of the commit']")).toBeFocused()
    await page.keyboard.press("j")
    await expect(position).toHaveText("File 3 / 5")

    // the last card ends the review
    await window.getByRole("treeitem", { name: "README.md" }).click()
    await expect(position).toHaveText("File 5 / 5")
    await expect(code.locator("[data-testid=diff-view-file][data-path='README.md']").getByTestId("review-card-end")).toHaveText("End of the review")
  })

  simpleTest(
    "should scroll the diffs with a map of the files in place of their scrollbar: a click shows the file under it, the hover names it, the thumb drags and follows",
    async ({ page }) => {
      const window = await openLowTreeReview(page)
      const code = window.getByTestId("commit-review-code")
      const view = code.getByTestId("diff-view")
      const map = code.getByTestId("review-file-map")
      const thumb = map.getByTestId("review-file-map-thumb")
      const position = window.getByTestId("review-position")
      const section = (name: string) => code.locator(`[data-testid=diff-view-file][data-path$="/${name}"]`)
      const scrollTop = () => view.evaluate(element => element.scrollTop)
      await code.getByRole("button", { name: "Expand all unchanged lines of RowMapper.kt" }).click()
      // the map stands for the scrollbar of the diffs, a section for each file
      await expect(view).toHaveCSS("scrollbar-width", "none")
      await expect(map).toHaveAttribute("data-sections", "5")
      await expect(map).toHaveAttribute("data-current", /\/RowMapper\.kt$/)

      // the hover names the file at the place, its directory after it
      const second = await mapPointOf(section("RowMapperTest.kt"))
      await page.mouse.move(second.x, second.y)
      const tip = map.getByTestId("review-file-map-tip")
      await expect(tip).toContainText("RowMapperTest.kt")
      await expect(tip).toContainText("testSrc")
      // beside the pointer, its middle at the height of it
      await expect
        .poll(async () => {
          const box = (await tip.boundingBox())!
          return Math.abs(box.y + box.height / 2 - second.y)
        })
        .toBeLessThan(1)
      // a click shows it as a row of the tree does: its header at the top, the tree and the footer follow; the diffs take
      // the focus, so J goes on from there
      await page.mouse.click(second.x, second.y)
      await expectSecondFileShown(window)
      await expect(map).toHaveAttribute("data-current", /\/RowMapperTest\.kt$/)
      await expect(code.locator("[aria-label='Changes of the commit']")).toBeFocused()
      await page.keyboard.press("j")
      await expect(position).toHaveText("File 3 / 5")

      // the thumb is the window of the view: it follows the wheel over the diffs, at the scale of the map; a row shows the
      // card of its file, the commit at the top of the document above the view
      await window.getByRole("treeitem", { name: "RowMapper.kt" }).click()
      await expect(position).toHaveText("File 1 / 5")
      await expect.poll(scrollTop).toBeGreaterThan(0)
      const start = await scrollTop()
      const scale = await view.evaluate(element => element.parentElement!.querySelector("[data-testid=review-file-map-track]")!.clientHeight / element.scrollHeight)
      const atTop = (await thumb.boundingBox())!.y
      const box = (await view.boundingBox())!
      await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2)
      await page.mouse.wheel(0, 200)
      await expect.poll(scrollTop).toBe(start + 200)
      await expect.poll(async () => (await thumb.boundingBox())!.y - atTop).toBeCloseTo(200 * scale, 0)

      // a drag of the thumb scrolls the diffs by the scale of the map
      const drag = async (by: number) => {
        const handle = (await thumb.boundingBox())!
        await page.mouse.move(handle.x + handle.width / 2, handle.y + 4)
        await page.mouse.down()
        // dragged, it is darker than under the pointer
        await expect(thumb).toHaveAttribute("data-dragging", "")
        await page.mouse.move(handle.x + handle.width / 2, handle.y + 4 + by, { steps: 3 })
        await page.mouse.up()
        await expect(thumb).not.toHaveAttribute("data-dragging")
      }
      await drag(10)
      await expect.poll(scrollTop).toBeCloseTo(start + 200 + 10 / scale, -1)
      // to the end: past the end of the document the last file is at the top, the space below it empty; the tree, the
      // position and the map follow it
      await drag(1000)
      await expect.poll(scrollTop).toBe(await view.evaluate(element => element.scrollHeight - element.clientHeight))
      const fileAtTop = await view.evaluate(element => {
        const at = element.getBoundingClientRect()
        const path = document
          .elementFromPoint(at.left + at.width / 2, at.top + 30)!
          .closest("[data-testid=diff-view-file]")!
          .getAttribute("data-path")!
        return path.slice(path.lastIndexOf("/") + 1)
      })
      expect(fileAtTop).toBe("README.md")
      await expect(selectedFile(window)).toHaveAttribute("aria-label", "README.md")
      await expect(position).toHaveText("File 5 / 5")
      await expect(map).toHaveAttribute("data-current", "README.md")
    },
  )

  simpleTest("should give the sidebar the width of its names, and hide and show it with F and its button, the choice kept", async ({ page }) => {
    await page.setViewportSize({ width: 2000, height: 1000 })
    const window = await openTreeReview(page)
    const sidebar = window.getByTestId("review-sidebar")
    const toggle = window.getByTestId("review-sidebar-toggle")
    // 24rem in a maximized window, not a share of it
    await expect.poll(async () => Math.round((await sidebar.boundingBox())?.width ?? 0)).toBe(384)
    await expect(window.getByTestId("review-window-footer")).toContainText("FSidebar")
    await expect(toggle).toHaveAttribute("aria-pressed", "true")

    await window.locator("[aria-label='Changes of the commit']").focus()
    await page.keyboard.press("f")
    await expect(sidebar).toHaveCount(0)
    await expect(toggle).toHaveAttribute("aria-pressed", "false")
    await waitForStorePersistence(page, "appSettings", settings => (settings as { reviewSidebar?: boolean } | undefined)?.reviewSidebar === false)
    // the choice stays for the next review: no window brings the sidebar back by itself
    await page.reload()
    await expect(page.getByTestId("review-window").getByTestId("review-sidebar-toggle")).toHaveAttribute("aria-pressed", "false")
    await expect(page.getByTestId("review-sidebar")).toHaveCount(0)
    await page.getByTestId("review-sidebar-toggle").click()
    await expect(page.getByTestId("review-sidebar")).toBeVisible()
  })

  simpleTest("should copy the lines of a selection of the diffs from the model in Inline, without the deleted parts and the line numbers", async ({ page }) => {
    const window = await openTreeReview(page)
    const code = window.getByTestId("commit-review-code")
    const mapper = code.locator('[data-testid=diff-view-file][data-path$="/RowMapper.kt"]')
    // the last three rows of RowMapper.kt: `val field30`, the row of the similar pair of `map`, `}`
    const pair = mapper.locator(".dv-row[data-kind=inline]")
    await expect.poll(() => joinedText(pair.locator("[data-inline-deleted]"))).toBe("Row = Row")
    const copied = await copyOfCells(mapper, ".dv-code[data-side]", 3)
    // the row shows the deleted `Row = Row` struck through; the copy is the new line
    expect(copied.text).toBe("  val field30 = 30\n  fun map(state: String): SessionRow = SessionRow(state)\n}")
  })

  simpleTest("should find a text in the diffs from the model, also in the folded lines, with ⌘F, Enter and Escape", async ({ page }) => {
    const window = await openTreeReview(page)
    const { code, field, count } = await openFind(page, window)
    // line 11 of RowMapper.kt lies in a fold: only the find in the folded lines sees it
    await field.fill("field10")
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

  simpleTest("should find a text in the deleted parts of an Inline row, struck through, and in the deleted rows of Unified", async ({ page }) => {
    const window = await openTreeReview(page)
    const { code, field, count } = await openFind(page, window)
    const mapper = code.locator('[data-testid=diff-view-file][data-path$="/RowMapper.kt"]')

    // Inline, the default. `: Row = Row(` stood in the old line of `map` alone: the row shows `: ` and `(` from the new
    // line and strikes `Row = Row` through between them, and the hit marks all three
    await field.fill(": Row = Row(")
    await expect(count).toHaveText("1 result")
    const pair = mapper.locator(".dv-row[data-kind=inline]")
    await field.press("Enter")
    await expect(count).toHaveText("1 of 1")
    await expect.poll(() => joinedText(pair.locator("[data-inline-deleted].dv-hit-current"))).toBe("Row = Row")
    await expect.poll(() => joinedText(pair.locator(".dv-hit-current"))).toBe(": Row = Row(")
    // the text the two lines share is found once, in the new line; the deleted part and the inserted one each on its own
    await field.fill("fun map(state")
    await expect(count).toHaveText("1 result")
    await field.fill("Row(state)")
    await expect(count).toHaveText("2 results")
    // the deleted `Row` before the inserted `SessionRow`, as the row reads
    await field.press("Enter")
    await expect.poll(() => joinedText(pair.locator(".dv-hit-current"))).toBe("Row(state)")
    await expect.poll(() => joinedText(pair.locator("[data-inline-deleted].dv-hit-current"))).toBe("Row")
    await field.press("Enter")
    await expect(count).toHaveText("2 of 2")
    await expect(pair.locator("[data-inline-deleted].dv-hit-current")).toHaveCount(0)

    // Unified: the old line is a deleted row, found there
    await window.getByRole("button", { name: "Unified" }).click()
    await field.fill(": Row = Row(")
    await expect(count).toHaveText("1 result")
    await expect.poll(() => joinedText(mapper.locator(".dv-row[data-kind=deleted] .dv-hit"))).toBe(": Row = Row(")
    await expect(mapper.locator(".dv-row[data-kind=added] .dv-hit")).toHaveCount(0)
  })

  simpleTest("should open find from the file tree and step results there with ⌘F and ⌘G @webkit", async ({ page }) => {
    const window = await openTreeReview(page)
    const item = window.getByRole("tree", { name: "Changed files" }).getByRole("treeitem").first()
    await item.focus()
    await expect(item).toBeFocused()

    await page.keyboard.press("ControlOrMeta+f")
    const field = window.getByRole("textbox", { name: "Find in the review" })
    await expect(field).toBeFocused()
    const count = window.getByTestId("diff-find-count")
    await field.fill("val")
    await expect(count).toHaveText(/^\d+ results?$/)
    const total = Number((await count.textContent())?.split(" ")[0])
    expect(total).toBeGreaterThan(1)

    await field.press("Enter")
    await expect(count).toHaveText(`1 of ${total}`)
    await item.focus()
    await expect(item).toBeFocused()
    await page.keyboard.press("ControlOrMeta+g")
    await expect(count).toHaveText(`2 of ${total}`)
    await page.keyboard.press("ControlOrMeta+Shift+g")
    await expect(count).toHaveText(`1 of ${total}`)
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
    await expectClosed(preview)
    const atFile = await openedReview(page, 1)
    expect(atFile.data.focus.path).toBe("plugins/air/backend/session/catalog/testSrc/RowMapperTest.kt")

    // the footer button: the first file
    await openCommitPreview(page, getBranchCommitRow(page, "session", SUBJECT))
    await preview.getByTestId("commit-review-toggle").click()
    await expectClosed(preview)
    expect((await openedReview(page, 2)).data.focus.path).toBeNull()

    const window = await showReviewWindow(page, atFile)
    await expect(window.getByTestId("review-position")).toHaveText("File 2 / 5")
    await expect(selectedFile(window)).toHaveAttribute("aria-label", "RowMapperTest.kt")
  })
})

simpleTest.describe("Commit review in Split", () => {
  simpleTest.beforeEach(async ({ setupRepo, syncAndWaitForBranches }) => {
    // Split stored: the default is Inline, one column
    await setupRepo("commit_tree", SPLIT_DIFFS)
    await syncAndWaitForBranches()
  })

  simpleTest("should drag the divider of the sides of Split, keep its share, and make the sides equal with a double click", async ({ page }) => {
    await page.setViewportSize({ width: 1600, height: 900 })
    const window = await openTreeReview(page)
    const code = window.getByTestId("diff-view")
    await expect(window.locator(".dv-row[data-kind=changed]").first()).toBeVisible()
    // the painter paints a row anew when its texts arrive or the layout changes, so a handle of a row can be detached
    // the moment after it was found: every look finds the row and measures it in one step of the page, and twice a
    // frame apart, so it also tells whether the rows still move
    const firstRow = () =>
      window.evaluate(async review => {
        const measure = () => {
          const row = review.querySelector(".dv-row[data-kind=changed]")!
          const [left, right] = [...row.children].map(half => half.getBoundingClientRect())
          return { share: left!.width / row.getBoundingClientRect().width, divider: { x: right!.x, y: right!.y + right!.height / 2 } }
        }
        const before = measure()
        await new Promise(resolve => requestAnimationFrame(resolve))
        const after = measure()
        return { ...after, still: before.divider.x === after.divider.x && before.divider.y === after.divider.y }
      })
    const share = async () => (await firstRow()).share
    await expect.poll(share).toBeCloseTo(0.5, 2)
    // the divider is the line between the halves of a row, taken once the rows have stopped moving: the font of the
    // code has loaded, every diff is read, and two frames give one point, else the press misses the 4 px to each side
    // of the line
    const line = async () => {
      await page.evaluate(() => document.fonts.ready.then(() => undefined))
      await expect(code).not.toHaveAttribute("aria-busy", "true")
      let at = { x: 0, y: 0 }
      await expect
        .poll(async () => {
          const row = await firstRow()
          at = row.divider
          return row.still
        })
        .toBe(true)
      return at
    }

    const at = await line()
    await page.mouse.move(at.x, at.y)
    await page.mouse.down()
    // the press took the divider, where a miss would show only as a share that never moved
    await expect(code).toHaveAttribute("data-dragging")
    await page.mouse.move(at.x - 300, at.y, { steps: 5 })
    await page.mouse.up()
    await expect(code).not.toHaveAttribute("data-dragging")
    await expect.poll(share).toBeLessThan(0.35)
    const dragged = await share()
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
    // a press on the new side keeps the selection there; the copy reads the lines of the rows from the texts: the
    // last three rows of the new side of the first file
    await newSide.nth(0).click()
    const copied = await copyOfCells(code.getByTestId("diff-view-file").first(), '.dv-half[data-side="1"] .dv-code[data-side]', 3)
    expect(copied.text).toBe(copied.lines.join("\n"))
    expect(copied.text.split("\n")).toHaveLength(3)
  })

  simpleTest("should find a text on both sides of a change in Split", async ({ page }) => {
    const window = await openTreeReview(page)
    const { code, field, count } = await openFind(page, window)
    // `Row = Row(state)` of the old side of RowMapper.kt, `SessionRow = SessionRow(state)` of the new one
    await field.fill("Row(state)")
    await expect(count).toHaveText("2 results")
    const mapper = code.locator('[data-testid=diff-view-file][data-path$="/RowMapper.kt"]')
    await expect.poll(() => joinedText(mapper.locator('.dv-half[data-side="0"] .dv-hit'))).toBe("Row(state)")
    await expect.poll(() => joinedText(mapper.locator('.dv-half[data-side="1"] .dv-hit'))).toBe("Row(state)")
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

simpleTest("should colour C# as Rider does: a field, a declaration, an attribute, a keyword, an escape", async ({ page, setupRepo, syncAndWaitForBranches }) => {
  // the `csharp_colors` template: one commit of `colors` adds src/Box.cs
  await setupRepo("csharp_colors")
  await syncAndWaitForBranches()
  await expandBranch(page, findBranchRow(page, "colors"))
  await openCommitPreview(page, getBranchCommitRow(page, "colors", "add the box"))
  await page.keyboard.press("d")
  const window = await showReviewWindow(page, await openedReview(page, 1))
  const code = window.getByTestId("commit-review-code")
  await expect(code).toContainText("public void Add()")
  const colorOf = (text: string) => code.getByText(text, { exact: true }).first()
  // the backend's highlighting from Roslyn's tree: a field and its use, a method's declaration, an attribute
  await expect(colorOf("count")).toHaveCSS("color", await tokenColor(window, "--syntax-field"))
  await expect(colorOf("Add")).toHaveCSS("color", await tokenColor(window, "--syntax-function-declaration"))
  await expect(colorOf("Serializable")).toHaveCSS("color", await tokenColor(window, "--syntax-annotation"))
  await expect(colorOf("sealed")).toHaveCSS("color", await tokenColor(window, "--syntax-keyword"))
  await expect(colorOf("\\t")).toHaveCSS("color", await tokenColor(window, "--syntax-escape"))
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

testWithIssueLinks("should keep the Activity panel the main window opened when a review window hides its sidebar right after", async ({ page, testStore }) => {
  await page.setViewportSize({ width: 2000, height: 1000 })
  // a push, so that the main window has activity to show
  const pushed = findBranchRow(page, "ABC-123")
  await getPushButton(pushed).click()
  await expectNothingToPush(pushed)
  const opened = await reviewPoolSizeCommit(page)

  // the main window opens its panel while the page of the review window still loads: a change after its snapshot
  await activityButton(page).click()
  await expect(activityPanel(page)).toBeVisible()
  const panel = await appSettingsChange(page, { activityPanel: true })
  expect(panel.origin).toBe("main")
  expect(panel.version).toBeGreaterThan(opened.storeVersion ?? 0)
  // the load of the page takes it, and the window shows; then it hides its sidebar right after
  const window = await openLinkedSubWindow(page, opened)
  await expect.poll(() => getSubWindowReadyOpen(window)).toBe(1)
  await expect(window.getByTestId("review-sidebar-toggle")).toHaveAttribute("aria-pressed", "true")
  // the window sends only the field it changed, so the writer keeps the panel of the main window
  await hideSidebarSendingOnlyIt(window)
  const sidebar = await appSettingsChange(window, { reviewSidebar: false, activityPanel: true })
  expect(sidebar.origin).toBe("review")
  expect(sidebar.version).toBeGreaterThan(panel.version)
  await expect.poll(() => testStore.get("appSettings")).toMatchObject({ reviewSidebar: false, activityPanel: true })
  await expect(activityPanel(page)).toBeVisible()
})

testWithIssueLinks("should follow Actual Size of the main window in a review window that changed another field in between", async ({ page }) => {
  const window = await openSettledReviewWindow(page)
  const toggle = window.getByTestId("review-sidebar-toggle")

  // the main window zooms in, and the review window hides its sidebar
  await emitTauriEvent(page, "zoom", "in")
  await expect.poll(() => webviewZoom(page)).toBe(1.1)
  const zoomIn = await appSettingsChange(page, { zoom: 1.1 })
  await hideSidebarSendingOnlyIt(window)
  // the writer's change of the zoom reaches the review window, and its own change keeps the zoom, at a later version
  await expect.poll(() => webviewZoom(window)).toBe(1.1)
  const sidebar = await appSettingsChange(window, { zoom: 1.1, reviewSidebar: false })
  expect(sidebar.origin).toBe("review")
  expect(sidebar.version).toBeGreaterThan(zoomIn.version)

  // Actual Size: the zoom goes back to its default, which reaches the review window too, newer than its own change
  await emitTauriEvent(page, "zoom", "reset")
  await expect.poll(() => webviewZoom(page)).toBe(1)
  await expect.poll(() => webviewZoom(window)).toBe(1)
  expect((await appSettingsChange(page, { zoom: 1 })).version).toBeGreaterThan(sidebar.version)
  await expect(toggle).toHaveAttribute("aria-pressed", "false")
})

testWithIssueLinks("should switch the commit of a review with Alt and the arrows", async ({ page }) => {
  const opened = await reviewPoolSizeCommit(page)
  // the branch, at the commit
  expect(opened.data).toMatchObject({ chain: ["JIRA-456"], focus: { range: "own", path: null } })
  focusedCommit(opened)

  const window = await showReviewWindow(page, opened)
  const position = window.getByTestId("review-position")
  await expect(position).toHaveText("Commit 2 / 3 · File 1 / 1")
  await expect(window.getByTestId("range-review-commit-header")).toContainText("perf db: increase pool size to 50")
  await expect(window.getByTestId("commit-review-code")).toContainText("size: 50")

  await page.keyboard.press("Alt+ArrowDown")
  await expect(window.getByTestId("range-review-commit-header")).toContainText("Fix database connection pool timeout")
  await expect(position).toHaveText("Commit 3 / 3 · File 1 / 1")
  await expect(window.getByTestId("review-window-footer")).toContainText("EscClose")
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
  // its shell is said with its title row: the backend counts the 300 ms of its content from it, not from the load
  await expect.poll(() => getWindowShell(review)).toMatchObject({ titleRows: 1 })
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
  await expectClosed(preview)

  // Review Changes, the only review item of the row's menu, first
  await row.click({ button: "right" })
  await expect(page.getByRole("menuitem").first()).toHaveText("Review ChangesD")
  await expect(page.getByRole("menuitem", { name: "Open in Window" })).toHaveCount(0)
  await page.getByRole("menuitem", { name: "Review Changes" }).click()
  expect((await openedReview(page, 2)).data.focus).toMatchObject({ commitId, path: null })
  await expectClosed(page.getByRole("menu"))

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
  await expectClosed(preview)
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
  await expectClosed(preview)
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
  const position = window.getByTestId("review-position")
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
    const position = window.getByTestId("review-position")
    const code = window.getByTestId("commit-review-code")
    await expect(position).toHaveText("File 1 / 2")
    // Inline, the default: the row of `second` is its new line with the deleted `3` before the inserted `4`
    await expect(code.locator(".dv-row[data-kind=inline]", { hasText: "fun second() = " }).locator("[data-inline-deleted]")).toHaveText("3")
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
    // Ignore Whitespace and Blank Lines: the blank line is ignored, so N passes it, and the counts say it
    await choose("Ignore Whitespace and Blank Lines", "modified, +1 −1, 1 line hidden by Whitespace")
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
    expect(opened.data).toMatchObject({ chain: ["IJPL-257337", "perf-part-2"], branchName: "perf-part-2" })
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
    // the open is of the parent's commit; the window decides it is a move: its chain stays
    expect(moved.data).toMatchObject({ branchName: "IJPL-257337", chain: ["IJPL-257337"] })
    await reopenLinkedSubWindow(window, moved)
    await expect(window.getByTestId("range-review-commit-header")).toContainText("Load icons lazily")
    await expect(segments).toHaveCount(2)
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

testWithUnassigned("should review a selection of several unassigned commits together, as a list of its own", async ({ page, setupUnassignedRepo }) => {
  // the `unassigned` template: two commits without a prefix
  await setupUnassignedRepo()
  const first = getUnassignedCommitRow(page, "Fix login validation bug")
  const second = getUnassignedCommitRow(page, "Add user authentication")
  await selectCommit(first)
  await multiSelectCommit(second)
  // D on a row of the selection: the selection, as a list of its own, in the order of the list
  await second.hover()
  await page.keyboard.press("d")
  const opened = await openedReview<CommitsReviewWindowData>(page, 1)
  expect(opened.data).toMatchObject({ kind: "commits", listLabel: "Unassigned", index: 0 })
  expect(opened.data.commits.map(commit => commit.subject)).toEqual(["Fix login validation bug", "Add user authentication"])
  // Review Changes of the menu of the selection, enabled: the same
  await second.click({ button: "right" })
  const item = page.getByRole("menuitem", { name: "Review Changes" })
  await expect(item).toBeEnabled()
  await item.click()
  expect((await openedReview<CommitsReviewWindowData>(page, 2)).data.commits).toHaveLength(2)
})

simpleTest("should review a selection of commits of a branch in the review of the branch, at the first of them", async ({ page, setupRepo, syncAndWaitForBranches }) => {
  // the `stack_push_alone` template: IJPL-257337, two commits, and perf-part-2 stacked on it
  await setupRepo("stack_push_alone")
  await syncAndWaitForBranches()
  await expandBranch(page, findBranchRow(page, "IJPL-257337"))
  const newer = getBranchCommitRow(page, "IJPL-257337", "Parse the icon off the EDT")
  const older = getBranchCommitRow(page, "IJPL-257337", "Load icons lazily")
  await selectCommit(newer)
  await multiSelectCommit(older)
  await older.hover()
  await page.keyboard.press("d")
  // the branch review lists both in its column: it opens at the first of the selection, as the list orders it
  const opened = await openedReview(page, 1)
  expect(opened.data).toMatchObject({ kind: "branch", chain: ["IJPL-257337"], focus: { commitId: expect.stringMatching(/^[0-9a-f]{40}$/) } })
  expect(opened.title).toMatch(/^Review IJPL-257337 · [0-9a-f]{8}$/)
})

simpleTest("should review an unassigned commit in the review window of the commit", async ({ page, setupRepo, syncAndWaitForBranches }) => {
  await setupRepo("single_unassigned")
  await syncAndWaitForBranches()
  await getUnassignedCommitRow(page, "Fix critical bug").hover()
  await page.keyboard.press("d")
  const opened = await openedReview<CommitsReviewWindowData>(page, 1)
  expect(opened.data).toMatchObject({ listLabel: "Unassigned", index: 0 })
})

simpleTest("should review a commit of a branch in conflict and of a blocked one in the review window of the commit", async ({ page, setupRepo, syncAndWaitForBranches }) => {
  // the `stacked_conflict` template: the copy of `auth` conflicts, so `login`, stacked on it, is blocked
  await setupRepo("stacked_conflict")
  await syncAndWaitForBranches()
  await expandBranch(page, findBranchRow(page, "login"))
  await getBranchCommitRow(page, "login", "Add login form").hover()
  await page.keyboard.press("d")
  expect((await openedReview<CommitsReviewWindowData>(page, 1)).data).toMatchObject({ listLabel: "login" })

  await expandBranch(page, findBranchRow(page, "auth"))
  await getBranchCommitRow(page, "auth", "Read the auth config").hover()
  await page.keyboard.press("d")
  expect((await openedReview<CommitsReviewWindowData>(page, 2)).data).toMatchObject({ listLabel: "auth" })
})

simpleTest(
  "should start a lock file and a minified file collapsed, their header saying why, and open one with its chevron",
  async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // the `generated_files` template, without the review plan: the names alone collapse a file
    await setupRepo("generated_files")
    await syncAndWaitForBranches()
    await findBranchRow(page, "search").click({ button: "right" })
    await page.getByRole("menuitem", { name: "Review Changes" }).click()
    const review = await openLinkedSubWindow(page, await openedReview(page, 1))
    const header = (path: string) => review.locator(`[data-testid="diff-view-file"][data-path="${path}"]`).getByTestId("review-file-header")
    const name = (path: string) => path.split("/").at(-1)!
    const collapsed: [string, string][] = [
      ["pnpm-lock.yaml", "Lock file"],
      ["dist/app.min.js", "Minified"],
    ]
    for (const [path, why] of collapsed) {
      await expect(header(path).getByTestId("review-file-folded")).toHaveText(why)
      await expect(header(path).getByRole("button", { name: `Expand ${name(path)}` })).toHaveAttribute("aria-expanded", "false")
    }
    // the real work starts open, and so does a file only its attributes or its header call generated
    for (const path of ["src/App.kt", "config/mcp.json", "AGENTS.md"]) {
      await expect(header(path).getByRole("button", { name: `Collapse ${name(path)}` })).toHaveAttribute("aria-expanded", "true")
      await expect(header(path).getByTestId("review-file-folded")).toHaveCount(0)
    }

    // the chevron opens the lock file
    await header("pnpm-lock.yaml").getByRole("button", { name: "Expand pnpm-lock.yaml" }).click()
    await expect(header("pnpm-lock.yaml").getByRole("button", { name: "Collapse pnpm-lock.yaml" })).toHaveAttribute("aria-expanded", "true")
    await expect(review.getByTestId("diff-view")).toContainText("b@2.0.0")
  },
)
