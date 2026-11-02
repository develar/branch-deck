// @spec spec/branch/review-plan.spec.md
import { writeFileSync } from "node:fs"
import path from "node:path"
import type { Locator, Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { captureHtmlSnapshot } from "../helpers/aria-snapshot-helpers"
import { findBranchRow, getUnassignedCommitRow } from "../helpers/branch-helpers"
import { pressCopyKey, readClipboard } from "../helpers/clipboard-helpers"
import { openCommitPreview } from "../helpers/preview-helpers"
import { openPlanWindow, reviewedPaths, turnOnReviewPlan } from "../helpers/review-plan-helpers"
import { openReviewWindow } from "../helpers/review-window-helpers"
import { clickContextMenuItem, openContextMenu, selectedInDiffs } from "../helpers/selection-helpers"
import { getOpenedSubWindows, openLinkedSubWindow, openSubWindowPage } from "../helpers/sub-window-helpers"
import { runAndWaitForAnswer } from "../helpers/sync-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"
import { windowCalls } from "../helpers/title-bar-helpers"
import { closeToast } from "../helpers/toast-helpers"
import { holdCommandCalls } from "../helpers/wait-helpers"

// the `symbol_navigation` template (src/Store.kt with the top-level `helper`, the commit of `nav`), then the branch
// `plan` of a terminal: `helper` takes a scale (a changed declaration that src/Base.kt and others use), then
// src/b/Cache.kt and src/a/Reader.kt, which imports it: Reader uses Cache, a link the change added

/** src/Store.kt as the commit of `nav` leaves it, with `helper` declared as given */
function storeText(helper: string): string {
  let text = "class Store {\n  val version = 2\n"
  for (let index = 0; index < 30; index++) {
    text += `  val field${index} = ${index}\n`
  }
  return `${text}  fun add(value: Int) {}\n}\n\n${helper}\n`
}

/** The place of `helper` in `storeText`, as the index of a fixture keys its answer */
const HELPER = "src/Store.kt:36"

const READER = (body: string) => `package a\n\nimport b.Cache\n\nclass Reader {\n  fun read(cache: Cache) = ${body}\n}\n`

/** The branch `plan` of two commits on top of the template, the index of accio with three usages of `helper` */
async function setupPlan(repo: TestRepositoryBuilder, { experimental }: { experimental: boolean }): Promise<void> {
  if (experimental) {
    turnOnReviewPlan(repo)
  }
  await repo.commit("(plan) scale the helper", { "src/Store.kt": storeText("fun helper(value: Int, scale: Int) = value * scale") }, 1704121200)
  await repo.commit(
    "(plan) add the cache",
    { "src/b/Cache.kt": "package b\n\nclass Cache {\n  fun get(key: String): Int = key.length\n}\n", "src/a/Reader.kt": READER('cache.get("x")') },
    1704123000,
  )
  // the index of the repository, as accio answers `references --at` the line of `helper`
  const usage = (file: string, line: number) => ({ file, line, column: 3, snippet: "helper(1)", kind: "call", resolved: "typed" })
  writeFileSync(
    path.join(repo.path, ".git", "branch-deck-accio.json"),
    JSON.stringify({
      references: {
        // and a call only the name ties, in a script
        [HELPER]: [usage("src/Base.kt", 1), usage("src/Uses.kt", 4), usage("src/main/App.kt", 9), { ...usage("scripts/helper.py", 2), resolved: "name" }],
      },
    }),
  )
}

/** The rows of the commit column: a scope by its label, a commit by its subject with what it is since the review */
async function commitColumn(page: Page): Promise<string[]> {
  return page.getByTestId("range-review-commit").evaluateAll(rows =>
    rows.map(row =>
      [...row.children]
        .filter(child => child.getAttribute("data-testid") !== "range-review-commit-hash")
        .map(child => child.textContent)
        .join(""),
    ),
  )
}

/** The commit rows of the tree of a scope with the plan: each by its subject with what it is since the review, in their order */
async function treeCommits(page: Page): Promise<string[]> {
  return page
    .getByRole("tree", { name: "Changed files" })
    .getByTestId("review-commit-subject")
    .evaluateAll(subjects => subjects.map(subject => `${subject.textContent}${subject.parentElement?.querySelector('[data-testid="review-commit-since"]')?.textContent ?? ""}`))
}

/** The commit row of `subject` in the tree of a scope with the plan */
function commitRow(page: Page, subject: string): Locator {
  return page.getByRole("tree", { name: "Changed files" }).getByRole("treeitem", { name: subject, exact: true })
}

/**
 * The review window of `plan` once its plan is read: its commits tell no story, so the tree lists them under Commits,
 * after the files, as the range lists them, and the column keeps the scope
 */
async function openPlanCommits(page: Page, repo: TestRepositoryBuilder, syncAndWaitForBranches: () => Promise<unknown>): Promise<Page> {
  await setupPlan(repo, { experimental: true })
  const window = await openPlanWindow(page, syncAndWaitForBranches)
  await expect.poll(() => treeCommits(window)).toEqual(["add the cache", "scale the helper"])
  await expect.poll(() => commitColumn(window)).toEqual(["All Changes2 commits"])
  return window
}

/**
 * The review window of `plan` with five notes more, so it has the search of the files, and the owners of the baseline,
 * src/b its own, so it has the owner chips
 */
async function openPlanWindowWithFilters(page: Page, repo: TestRepositoryBuilder, syncAndWaitForBranches: () => Promise<unknown>): Promise<Page> {
  await setupPlan(repo, { experimental: true })
  const notes = Object.fromEntries([1, 2, 3, 4, 5].map(number => [`docs/note${number}.txt`, `note ${number}\n`]))
  await repo.commit("(plan) add the notes", notes, 1704124800)
  await repo.commitOnRemote("master", {
    subject: "Add the owners",
    author: "Jane <jane@example.com>",
    timestamp: 1704110400,
    files: { OWNERSHIP: "owner 'Monorepo'\n", "src/b/OWNERSHIP": "owner 'Cache Team'\n" },
  })
  await repo.fetchInTerminal()
  return openPlanWindow(page, syncAndWaitForBranches)
}

/** The tree of the files of the review, and the rows of the three files of `plan` */
function planRows(window: Page) {
  const tree = window.getByRole("tree", { name: "Changed files" })
  const row = (name: string) => tree.getByRole("treeitem", { name })
  return { tree, reader: row("Reader.kt"), cache: row("Cache.kt"), store: row("Store.kt") }
}

/** The card of the one mark of a row: the pointer leaves the card open before, then moves onto the mark of `row` */
async function openFactsCard(window: Page, row: Locator): Promise<Locator> {
  const card = window.getByTestId("file-facts-card")
  await window.mouse.move(0, 0)
  await expect(card).toHaveCount(0)
  await row.getByTestId("file-row-mark").hover()
  await expect(card).toBeVisible()
  return card
}

/** Mark Reviewed of the title bar of the window: it keeps the base and the tip of `plan` and says so */
async function markReviewed(window: Page): Promise<void> {
  await runAndWaitForAnswer(window, "mark_branch_reviewed", () => window.getByTestId("branch-review-mark-reviewed").click())
  await expect(window.getByText("Marked plan reviewed")).toBeVisible()
}

test.describe("The review plan in the branch review", () => {
  test("should open the review on its overview with what can break, show the declaration of an entry, and keep the files in the tree", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    await setupPlan(await setupRepo("symbol_navigation"), { experimental: true })
    const window = await openPlanWindow(page, syncAndWaitForBranches)
    const { store, cache } = planRows(window)

    // the overview at the top of the document of All Changes: the changed `helper` with its 3 uses outside the change,
    // from accio
    const overview = window.getByTestId("review-overview")
    await expect(overview.getByTestId("review-plan-breakage")).toContainText("Possible Breakage1")
    const helper = overview.getByTestId("review-plan-breakage-entry")
    await expect(helper).toHaveCount(1)
    await expect(helper).toHaveAttribute("data-name", "helper")
    await expect(helper).toContainText("Changed")
    await expect(helper).toContainText("src/Store.kt:")
    await expect(helper.getByTestId("review-plan-outside")).toHaveText("3 uses left outside")
    // the forms before and after on their own lines, then the uses left outside
    await expect(helper.getByTestId("review-plan-before")).toContainText("value:Int")
    await expect(helper.getByTestId("review-plan-before")).not.toContainText("scale")
    await expect(helper.getByTestId("review-plan-after")).toContainText("scale:Int")
    for (const site of ["src/Base.kt:1", "src/Uses.kt:4", "src/main/App.kt:9"]) {
      await expect(helper.getByTestId("review-plan-sites")).toContainText(site)
    }
    // a call only the name ties is no use, and no proof of none
    await expect(helper.getByTestId("review-plan-sites")).toContainText("1 use by name only, unknown")
    await expect(helper.getByTestId("review-plan-sites")).not.toContainText("scripts/helper.py")

    // a use left outside opens its file at the head over the review, the name selected, the overview under it until Back;
    // one the head lacks says so
    const fileView = window.getByTestId("declaration-file-view")
    await helper.getByTestId("review-plan-site").filter({ hasText: "src/Base.kt:1" }).click()
    await expect(fileView).toContainText("outside the review")
    await expect.poll(() => selectedInDiffs(window)).toEqual({ text: "helper", path: "src/Base.kt" })
    await fileView.getByRole("button", { name: "Back" }).click()
    await expect(fileView).toBeHidden()
    await helper.getByTestId("review-plan-site").filter({ hasText: "src/main/App.kt:9" }).click()
    const missing = window.locator("[data-slot=toast]", { hasText: "Cannot open src/main/App.kt:9" })
    await expect(missing).toContainText("The head of the review has no such line")
    await closeToast(missing)

    // an entry selects its file and shows its declaration, the name selected
    await helper.getByTestId("review-plan-breakage-file").click()
    await expect(store).toHaveAttribute("aria-selected", "true")
    await expect.poll(() => selectedInDiffs(window)).toEqual({ text: "helper", path: "src/Store.kt" })
    // the new API is a fact of the step that adds it, each name at its declaration
    const newApi = overview.getByTestId("review-step-new-api")
    await expect
      .poll(() => newApi.evaluateAll(names => names.map(name => `${name.textContent}:${name.getAttribute("data-path")}`)))
      .toEqual(["Reader:src/a/Reader.kt", "Cache:src/b/Cache.kt"])
    await newApi.filter({ hasText: "Cache" }).click()
    await expect(cache).toHaveAttribute("aria-selected", "true")
    await expect.poll(() => selectedInDiffs(window)).toEqual({ text: "Cache", path: "src/b/Cache.kt" })
    await expect(window.getByTestId("review-position")).toHaveText(/File 2 \/ 3$/)

    // the diffs are the story: Cache, which Reader reads, before Reader; the tree keeps its order, no order to choose
    await expect.poll(() => reviewedPaths(window)).toEqual(["src/b/Cache.kt", "src/a/Reader.kt", "src/Store.kt"])
    await expect(window.getByRole("group", { name: "Order" })).toHaveCount(0)
  })

  test("should keep an API whose uses are unknown under Possible breakage, and say why", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("symbol_navigation")
    await setupPlan(repo, { experimental: true })
    // every query of the index fails: the uses are unknown, never a safe 0
    writeFileSync(path.join(repo.path, ".git", "branch-deck-accio.json"), JSON.stringify({ error: "no index of the repository" }))
    const window = await openPlanWindow(page, syncAndWaitForBranches)

    const overview = window.getByTestId("review-overview")
    await expect(overview.getByTestId("review-plan-breakage")).toContainText("Possible Breakage1")
    const helper = overview.getByTestId("review-plan-breakage-entry")
    await expect(helper).toHaveAttribute("data-name", "helper")
    await expect(helper.getByTestId("review-plan-outside")).toHaveText("The uses are unknown")
    await expect(overview.getByTestId("review-plan-unbroken")).toHaveCount(0)
    // why, folded under it
    const note = overview.getByTestId("review-plan-index-note")
    const reason = note.getByText(/^Uses unknown: /)
    await expect(reason).toBeHidden()
    await note.getByRole("button", { name: "Why the uses are unknown" }).click()
    await expect(reason).toBeVisible()
    // a lower bound at 0: a quiet mark, never a safe one
    const { store } = planRows(window)
    await expect(store.getByTestId("file-row-mark")).toHaveAttribute("data-tone", "muted")
    await expect(store).toHaveAttribute("aria-description", /, breaks \?$/)
    const storeCard = await openFactsCard(window, store)
    await expect(storeCard.getByTestId("file-facts-breaks")).toHaveText("breaks ? · The uses left outside the change are unknown, or their count is incomplete")
  })

  test(
    "should mark each file row once, warning when it breaks uses outside, and show a linked file from the card of its mark, a file that uses at its first use",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await setupPlan(await setupRepo("symbol_navigation"), { experimental: true })
      const window = await openPlanWindow(page, syncAndWaitForBranches)
      const { tree, reader, cache, store } = planRows(window)

      // Reader imports the new Cache: a link the change added; Store's `helper` keeps 3 uses outside the change: one mark
      // a row, the warning on Store, every fact in the description of the row
      await expect(tree.getByTestId("file-row-mark")).toHaveCount(3)
      await expect(reader.getByTestId("file-row-mark")).toHaveAttribute("data-tone", "muted")
      await expect(cache.getByTestId("file-row-mark")).toHaveAttribute("data-tone", "muted")
      await expect(store.getByTestId("file-row-mark")).toHaveAttribute("data-tone", "warning")
      await expect(reader).toHaveAttribute("aria-description", /, uses 1, \+1 new$/)
      await expect(cache).toHaveAttribute("aria-description", /, used by 1$/)
      await expect(store).toHaveAttribute("aria-description", /, breaks 3$/)
      await captureHtmlSnapshot(tree, "review-plan-tree-badges")

      // the card of Store says what it breaks and why, and nothing else
      const storeCard = await openFactsCard(window, store)
      await expect(storeCard.getByRole("heading")).toHaveText(["Breaks"])
      await expect(storeCard.getByTestId("file-facts-breaks")).toHaveText("breaks 3 · 3 uses left outside the change")

      // the card of Reader lists the link to Cache with its name and state; a click selects Cache and closes the card
      const popover = await openFactsCard(window, reader)
      await expect(popover.getByRole("heading")).toHaveText(["Uses 1 · +1 new"])
      const linked = popover.getByTestId("file-link")
      await expect(linked).toHaveCount(1)
      await expect(linked).toHaveAttribute("data-path", "src/b/Cache.kt")
      await expect(linked).toHaveAttribute("data-state", "added")
      await expect(linked).toContainText("Cache, get · Reader.kt:3")
      await expect(cache).toHaveAttribute("aria-selected", "false")
      await linked.click()
      await expect(cache).toHaveAttribute("aria-selected", "true")
      await expect(popover).toHaveCount(0)
      await expect(window.getByTestId("review-position")).toHaveText(/File 2 \/ 3$/)

      // the card of Cache lists Reader, which uses it: a click shows its first use, the name selected
      const cacheCard = await openFactsCard(window, cache)
      await expect(cacheCard.getByRole("heading")).toHaveText(["Used by 1"])
      await cacheCard.getByTestId("file-link").click()
      await expect(reader).toHaveAttribute("aria-selected", "true")
      await expect.poll(() => selectedInDiffs(window)).toEqual({ text: "Cache", path: "src/a/Reader.kt" })
    },
  )

  test(
    "should open the card of a row's mark by I on the focused row, the focus on its first file, Enter selecting it and Escape giving the focus back to the row",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await setupPlan(await setupRepo("symbol_navigation"), { experimental: true })
      const window = await openPlanWindow(page, syncAndWaitForBranches)
      const { reader, cache, store } = planRows(window)
      const card = window.getByTestId("file-facts-card")
      const footer = window.getByTestId("review-window-footer")
      const link = card.getByTestId("file-link")
      // the plan with the usages, after the one without them
      await expect(store).toHaveAttribute("aria-description", /, breaks 3$/)

      // the focused row: the hints name I
      await reader.getByText("Reader.kt", { exact: true }).click()
      await expect(reader).toBeFocused()
      await expect(footer).toContainText("Details")

      // I opens the card of Reader, the focus on its link; Escape closes it alone, the focus back on the row
      await window.keyboard.press("i")
      await expect(card).toBeVisible()
      await expect(link).toHaveAttribute("data-path", "src/b/Cache.kt")
      await expect(link).toBeFocused()
      await expect(footer).not.toContainText("Details")
      await window.keyboard.press("Escape")
      await expect(card).toHaveCount(0)
      await expect(reader).toBeFocused()
      await expect(footer).toContainText("Details")

      // Enter on the link selects Cache and closes the card, the focus back on the row it was opened from
      await window.keyboard.press("i")
      await expect(link).toBeFocused()
      await expect(cache).toHaveAttribute("aria-selected", "false")
      await window.keyboard.press("Enter")
      await expect(cache).toHaveAttribute("aria-selected", "true")
      await expect(card).toHaveCount(0)
      await expect(reader).toBeFocused()
      await expect(window.getByTestId("review-position")).toHaveText(/File 2 \/ 3$/)

      // the card of Store lists no file: the card takes the focus, and its Escape is the card's, not the window's
      await store.getByText("Store.kt", { exact: true }).click()
      await expect(store).toBeFocused()
      await window.keyboard.press("i")
      await expect(card.getByTestId("file-facts-breaks")).toHaveText("breaks 3 · 3 uses left outside the change")
      await expect(card).toBeFocused()
      await window.keyboard.press("Escape")
      await expect(card).toHaveCount(0)
      await expect(store).toBeFocused()
      expect(await windowCalls(window)).not.toContain("close")
    },
  )

  test(
    "should give the focus back to the row when a card opened by I goes by the hover of another mark or the pointer leaving, and close only the card on Escape",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await setupPlan(await setupRepo("symbol_navigation"), { experimental: true })
      const window = await openPlanWindow(page, syncAndWaitForBranches)
      const { reader, store } = planRows(window)
      const card = window.getByTestId("file-facts-card")
      const link = card.getByTestId("file-link")
      const breaks = card.getByTestId("file-facts-breaks")
      // the plan with the usages, after the one without them
      await expect(store).toHaveAttribute("aria-description", /, breaks 3$/)

      // I on Store, the last row: its card lists no file and takes the focus
      await window.mouse.move(0, 0)
      await store.getByText("Store.kt", { exact: true }).click()
      await window.keyboard.press("i")
      await expect(breaks).toHaveText("breaks 3 · 3 uses left outside the change")
      await expect(card).toBeFocused()

      // the hover of Reader's mark, above the card, replaces it: the focus does not fall to the page, it is back on Store
      await reader.getByTestId("file-row-mark").hover()
      await expect(link).toHaveAttribute("data-path", "src/b/Cache.kt")
      await expect(store).toBeFocused()
      // Escape closes the hovered card alone, the window stays
      await window.keyboard.press("Escape")
      await expect(card).toHaveCount(0)
      await expect(store).toBeFocused()
      expect(await windowCalls(window)).not.toContain("close")

      // the pointer passes over the mark of the card I opened and leaves it: the card closes, the focus back on Store
      await window.mouse.move(0, 0)
      await window.keyboard.press("i")
      await expect(card).toBeFocused()
      await store.getByTestId("file-row-mark").hover()
      await window.mouse.move(0, 0)
      await expect(card).toHaveCount(0)
      await expect(store).toBeFocused()
      expect(await windowCalls(window)).not.toContain("close")
    },
  )

  test("should select a linked file under an owner chip, which hides none, and clear only a search that hides it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("symbol_navigation")
    const window = await openPlanWindowWithFilters(page, repo, syncAndWaitForBranches)
    const { reader, cache } = planRows(window)
    const owner = (name: string) => window.getByTestId("branch-review-owner").filter({ hasText: name })
    const search = window.getByRole("textbox", { name: "Filter files" })
    const selectCacheFromReader = async () => {
      await (await openFactsCard(window, reader)).getByTestId("file-link").click()
    }
    const position = window.getByTestId("review-position")

    // the chip of Monorepo keeps Cache, of Cache Team, muted: a click on its link selects it, the chip stays
    await owner("Monorepo").click()
    await expect(cache).toHaveAttribute("data-muted", "")
    await expect(reader).not.toHaveAttribute("data-muted")
    await selectCacheFromReader()
    await expect(owner("Monorepo")).toHaveAttribute("aria-pressed", "true")
    await expect(cache).toHaveAttribute("aria-selected", "true")
    await expect(position).toHaveText(/File 7 \/ 8 · Monorepo \d+$/)
    await owner("All").click()
    await expect(cache).not.toHaveAttribute("data-muted")

    // a search that hides Cache is cleared, and Cache selected; the search does not select it again
    await search.fill("Reader")
    await expect(cache).toHaveCount(0)
    await selectCacheFromReader()
    await expect(search).toHaveValue("")
    await expect(cache).toBeVisible()
    await expect(cache).toHaveAttribute("aria-selected", "true")
    await expect(position).toHaveText(/File 7 \/ 8$/)
    // the name of the row: its end is the mark
    await reader.getByText("Reader.kt", { exact: true }).click()
    await expect(reader).toHaveAttribute("aria-selected", "true")
    await search.fill(".kt")
    await expect(reader).toHaveAttribute("aria-selected", "true")
    await search.fill("")
    await expect(reader).toHaveAttribute("aria-selected", "true")
    // a chip away and back keeps the selection: it starts nothing over
    await owner("Monorepo").click()
    await expect(cache).toHaveAttribute("data-muted", "")
    await owner("All").click()
    await expect(cache).not.toHaveAttribute("data-muted")
    await expect(reader).toHaveAttribute("aria-selected", "true")
    await expect(cache).toHaveAttribute("aria-selected", "false")
  })

  test("should give the commits back to the column while the search keeps none of their rows, and copy a commit row by the copy key and its menu", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("symbol_navigation")
    const window = await openPlanWindowWithFilters(page, repo, syncAndWaitForBranches)
    const search = window.getByRole("textbox", { name: "Filter files" })
    const column = window.getByTestId("range-review-commits").getByTestId("range-review-commit")
    // the three commits tell no story: the tree lists them under Commits, the column keeps the scope
    await expect.poll(async () => (await treeCommits(window)).toSorted()).toEqual(["add the cache", "add the notes", "scale the helper"])
    await expect(column).toHaveCount(1)

    // the copy key on a commit row copies its hash, Copy Commit Message of its menu its message
    await commitRow(window, "add the notes").click()
    await pressCopyKey(window)
    await expect.poll(() => readClipboard(window)).toBe(await repo.revision("user-name/virtual/plan"))
    await openContextMenu(window, commitRow(window, "add the notes"))
    await clickContextMenuItem(window, "Copy Commit Message")
    await expect.poll(() => readClipboard(window)).toMatch(/add the notes/)

    // a search that finds files of no step keeps no commit row: the column lists the commits again
    await search.fill("Reader")
    await expect(planRows(window).reader).toBeVisible()
    await expect(window.getByRole("tree", { name: "Changed files" }).getByTestId("review-commit-subject")).toHaveCount(0)
    await expect(column).toHaveCount(4)
    // a search that keeps nothing: no tree, the commits in the column, a click on one shows it alone
    await search.fill("nothing matches this")
    await expect(window.getByTestId("review-files-no-match")).toBeVisible()
    await expect(column).toHaveCount(4)
    await column.filter({ hasText: "add the cache" }).click()
    await expect(window.getByTestId("range-review-commit-header")).toContainText("add the cache")
    // back in All Changes with no search, the tree has the commits again
    await search.fill("")
    await column.filter({ hasText: "All Changes" }).click()
    await expect(commitRow(window, "add the cache")).toBeVisible()
    await expect(column).toHaveCount(1)
  })

  test("should select the file of an entry of the overview while a search hides every file, under an owner chip that hides none", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("symbol_navigation")
    const window = await openPlanWindowWithFilters(page, repo, syncAndWaitForBranches)
    const { cache, store } = planRows(window)
    const owner = (name: string) => window.getByTestId("branch-review-owner").filter({ hasText: name })
    const search = window.getByRole("textbox", { name: "Filter files" })
    const position = window.getByTestId("review-position")
    const noMatch = window.getByTestId("review-files-no-match")
    const card = window.getByTestId("review-overview")
    const header = (filePath: string) => window.locator(`[data-testid="review-file-header"][data-path="${filePath}"]`)

    // a search that matches no file: the overview stays, its entry clears the search and selects Store
    await search.fill("nothing matches this")
    await expect(noMatch).toBeVisible()
    await card.getByTestId("review-plan-breakage-file").click()
    await expect(search).toHaveValue("")
    await expect(noMatch).toHaveCount(0)
    await expect(store).toHaveAttribute("aria-selected", "true")
    await expect(position).toHaveText(/File 8 \/ 8$/)
    await expect(header("src/Store.kt")).toBeInViewport()
    await expect.poll(() => selectedInDiffs(window)).toEqual({ text: "helper", path: "src/Store.kt" })

    // an owner chip and a search that hides every file: the entry of Cache clears the search, the chip stays
    await search.fill("nothing matches this")
    await owner("Cache Team").click()
    await expect(noMatch).toBeVisible()
    await card.getByTestId("review-step-new-api").filter({ hasText: "Cache" }).click()
    await expect(search).toHaveValue("")
    await expect(owner("Cache Team")).toHaveAttribute("aria-pressed", "true")
    await expect(cache).toHaveAttribute("aria-selected", "true")
    await expect(position).toHaveText(/File 7 \/ 8 · Cache Team \d+ \/ \d+$/)
    await expect.poll(() => selectedInDiffs(window)).toEqual({ text: "Cache", path: "src/b/Cache.kt" })

    // the chip of Cache Team hides nothing: the search keeps Store, muted, and the entry selects it; chip and search stay
    await owner("All").click()
    await search.fill("Store")
    await owner("Cache Team").click()
    await expect(header("src/Store.kt")).toHaveAttribute("data-muted", "")
    await expect(noMatch).toHaveCount(0)
    await expect.poll(() => reviewedPaths(window)).toEqual(["src/Store.kt"])
    await card.getByTestId("review-plan-breakage-file").click()
    await expect(owner("Cache Team")).toHaveAttribute("aria-pressed", "true")
    await expect(search).toHaveValue("Store")
    await expect(store).toHaveAttribute("aria-selected", "true")
    await expect(header("src/Store.kt")).toBeInViewport()
  })

  test("should fold the mechanical changes into a collapsed group at the end, with the same edit and the dependency changes of a module", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("symbol_navigation")
    await setupPlan(repo, { experimental: true })
    // one more commit: Base.kt reindented, a new module file beside Cache, and the same line at the end of three files
    const SAME = "\n// read by the review plan of the branch\n"
    await repo.commit(
      "(plan) fold the build model",
      {
        "src/Base.kt": "fun outsider(value: Int) =  helper(value)\n",
        "src/b/b.iml": `<?xml version="1.0" encoding="UTF-8"?>\n<module type="JAVA_MODULE" version="4">\n  <component name="NewModuleRootManager">\n    <orderEntry type="module" module-name="intellij.platform.core" />\n  </component>\n</module>\n`,
        "src/a/Reader.kt": READER('cache.get("x")') + SAME,
        "src/b/Cache.kt": "package b\n\nclass Cache {\n  fun get(key: String): Int = key.length\n}\n" + SAME,
        "src/Store.kt": storeText("fun helper(value: Int, scale: Int) = value * scale") + SAME,
      },
      1704124800,
    )
    // the only use of `helper` is in Base.kt, which this change updates
    writeFileSync(
      path.join(repo.path, ".git", "branch-deck-accio.json"),
      JSON.stringify({ references: { [HELPER]: [{ file: "src/Base.kt", line: 1, column: 31, snippet: "helper(value)", kind: "call", resolved: "typed" }] } }),
    )
    const window = await openPlanWindow(page, syncAndWaitForBranches)
    const tree = window.getByRole("tree", { name: "Changed files" })

    // the real work in the order of the story, the build file and the reindented file last, in a group that starts collapsed
    await expect.poll(() => reviewedPaths(window)).toEqual(["src/b/Cache.kt", "src/a/Reader.kt", "src/Store.kt", "src/b/b.iml", "src/Base.kt"])
    const group = tree.getByRole("treeitem", { name: "Mechanical (2)" })
    await expect(group).toHaveAttribute("aria-expanded", "false")
    await expect(tree.getByRole("treeitem", { name: "src/Base.kt" })).toHaveCount(0)
    // the new module shows on the directory of its module, its build file folded
    const module = tree.getByRole("treeitem", { name: "b", exact: true })
    await expect(module.getByTestId("file-row-mark")).toHaveAttribute("data-tone", "muted")
    await expect(module).toHaveAttribute("aria-description", /, new module$/)
    await captureHtmlSnapshot(tree, "review-plan-tree-mechanical")
    const moduleCard = await openFactsCard(window, module)
    await expect(moduleCard.getByRole("heading")).toHaveText(["Dependencies · new module"])
    await expect(moduleCard.getByTestId("file-facts-dependency")).toHaveText(["New module b"])
    // their diffs start collapsed, the header saying why
    for (const [file, name, why] of [
      ["src/b/b.iml", "b.iml", "Build file"],
      ["src/Base.kt", "Base.kt", "Whitespace only"],
    ] as const) {
      const header = window.locator(`[data-testid="review-file-header"][data-path="${file}"]`)
      await expect(header.getByTestId("review-file-folded")).toHaveText(why)
      await expect(header.getByRole("button", { name: `Expand ${name}` })).toHaveAttribute("aria-expanded", "false")
    }

    // the same edit of three files: a click on another file of the group selects it
    const reader = tree.getByRole("treeitem", { name: "Reader.kt" })
    await expect(reader).toHaveAttribute("aria-description", /, same edit ×3$/)
    const popover = await openFactsCard(window, reader)
    await expect(popover.getByRole("heading", { name: "Same edit ×3" })).toBeVisible()
    await expect(popover.getByTestId("file-same-edit")).toHaveText("+ // read by the review plan of the branch")
    await expect(popover.getByTestId("file-same-edit-file")).toHaveText([/^Cache\.kt/, /^Store\.kt/])
    await popover.getByTestId("file-same-edit-file").filter({ hasText: "Store.kt" }).click()
    const store = tree.getByRole("treeitem", { name: "Store.kt" })
    await expect(store).toHaveAttribute("aria-selected", "true")

    // J and K reach the mechanical files last: J from Store opens the group at its first file
    await store.focus()
    await window.keyboard.press("j")
    await expect(group).toHaveAttribute("aria-expanded", "true")
    await expect(tree.getByRole("treeitem", { name: "src/b/b.iml" })).toHaveAttribute("aria-selected", "true")
    await expect(tree.getByRole("treeitem", { name: "Build files (1)" })).toBeVisible()
    await expect(tree.getByRole("treeitem", { name: "Whitespace only (1)" })).toBeVisible()

    // the overview: no possible break, `helper` folded as no use left outside breaks; the link into the new module a fact
    // of its step, at its first use; the dependency changes and the same edit are on their rows only
    const overview = window.getByTestId("review-overview")
    await expect(overview.getByTestId("review-plan-breakage-entry")).toHaveCount(0)
    await expect(overview.getByTestId("review-plan-unbroken")).toContainText("1 API change, no use left outside breaks")
    const moduleLink = overview.getByTestId("review-step-module-link")
    await expect(moduleLink).toHaveAttribute("data-path", "src/a/Reader.kt")
    await expect(moduleLink).toHaveText("Reader.kt → Cache.kt")
    await expect(overview).not.toContainText("Same edit")
    await expect(overview).not.toContainText("New module b")

    // a click selects the file of the link at its first use
    await moduleLink.click()
    await expect(reader).toHaveAttribute("aria-selected", "true")
  })

  test("should fold the generated files, each header saying why, a rendering at the end of the concern of its source, and keep the renderer's echo out of the repeated edits", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    // the source src/guide.md and its two renderings get the same edit; a JSON file .gitattributes marks, a lock file
    // and a minified file change too
    turnOnReviewPlan(await setupRepo("generated_files"))
    const window = await openPlanWindow(page, syncAndWaitForBranches, "search")
    const tree = window.getByRole("tree", { name: "Changed files" })
    // the guide a concern, its renderings at its end as their header names it; App.kt, which no evidence joins; then the
    // other generated files in the order of the tree
    await expect.poll(() => reviewedPaths(window)).toEqual(["src/guide.md", "docs/AGENTS.md", "AGENTS.md", "src/App.kt", "config/mcp.json", "dist/app.min.js", "pnpm-lock.yaml"])
    // the echo of the guide is no edit a person repeated: no group, no badge
    await expect(tree.getByRole("treeitem", { name: "src/guide.md" })).not.toHaveAttribute("aria-description", /same edit/)
    // the first file of the tree without the plan, a generated one, gave way to the first of the real work
    await expect(tree.getByRole("treeitem", { name: "src/guide.md" })).toHaveAttribute("aria-selected", "true")
    const concern = tree.getByRole("treeitem", { name: "guide.md", exact: true })
    await expect(concern).toHaveAttribute("aria-expanded", "true")
    const renderings = tree.getByRole("treeitem", { name: "Generated (2)" })
    await expect(renderings).toHaveAttribute("aria-expanded", "false")
    await renderings.click()
    await expect(tree.getByRole("treeitem", { name: "docs/AGENTS.md" })).toBeVisible()
    const group = tree.getByRole("treeitem", { name: "Mechanical (3)" })
    await expect(group).toHaveAttribute("aria-expanded", "false")
    await group.click()
    await expect(tree.getByRole("treeitem", { name: "Generated (3)" })).toBeVisible()
    for (const [file, why] of [
      ["AGENTS.md", "Generated by tools/render.mjs; edit src/guide.md"],
      ["docs/AGENTS.md", "Generated by tools/render.mjs; edit src/guide.md"],
      ["config/mcp.json", "Generated (.gitattributes)"],
      ["dist/app.min.js", "Minified"],
      ["pnpm-lock.yaml", "Lock file"],
    ] as const) {
      const header = window.locator(`[data-testid="review-file-header"][data-path="${file}"]`)
      await expect(header.getByTestId("review-file-folded")).toHaveText(why)
      await expect(header.getByRole("button", { name: `Expand ${file.split("/").at(-1)}` })).toHaveAttribute("aria-expanded", "false")
    }
    // the real work stays open
    await expect(window.locator('[data-testid="review-file-header"][data-path="src/guide.md"]').getByRole("button", { name: "Collapse guide.md" })).toHaveAttribute(
      "aria-expanded",
      "true",
    )

    // the chevron opens a generated file
    const agents = window.locator('[data-testid="review-file-header"][data-path="AGENTS.md"]')
    await agents.getByRole("button", { name: "Expand AGENTS.md" }).click()
    await expect(agents.getByRole("button", { name: "Collapse AGENTS.md" })).toHaveAttribute("aria-expanded", "true")
  })

  test("should list the concerns of the range in the tree, each with its files, a file of two concerns also in the second, and tell the story by them", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    // one commit of two tasks: ijproxy dropped from three configs in three directories and from the guide, grep
    // replaced by accio in the guide, its list of tools and AGENTS.md, rendered from the guide; App.kt joins neither
    turnOnReviewPlan(await setupRepo("review_concerns"))
    const window = await openPlanWindow(page, syncAndWaitForBranches, "search")
    // the brief above the diffs and every card of the story on screen: the view renders the cards it shows
    await window.setViewportSize({ width: 1280, height: 1200 })
    const tree = window.getByRole("tree", { name: "Changed files" })
    const row = (name: string) => tree.getByRole("treeitem", { name, exact: true })
    const rows = () => tree.getByRole("treeitem").evaluateAll(items => items.map(item => item.getAttribute("aria-label")))
    const position = window.getByTestId("review-position")

    // a concern per task, its files by their path; the guide's own row in the concern of most of its lines, a row
    // "also in" the other; the generated AGENTS.md folded into the concern of its source; then the rest and Mechanical
    await expect
      .poll(rows)
      .toEqual([
        "ijproxy",
        ".codex/config.toml",
        "docs/guide.md, also in ijproxy",
        "tools/codex/config.toml",
        ".mcp.json",
        "grep → accio",
        "docs/guide.md",
        "docs/tools.md",
        "Generated (1)",
        "Other changes",
        "src",
        "App.kt",
        "Mechanical (1)",
      ])
    await expect(row("ijproxy")).toHaveAttribute("aria-description", "3 files, +0 −10")
    await captureHtmlSnapshot(tree, "review-plan-concerns-tree")
    await row("Generated (1)").click()
    await expect(row("AGENTS.md")).toBeVisible()

    // the story in the order of the concerns, each under its name, the guide in each with its own block, the generated
    // file collapsed at the end of its concern
    await expect
      .poll(() => reviewedPaths(window))
      .toEqual([".codex/config.toml", "docs/guide.md", "tools/codex/config.toml", ".mcp.json", "docs/guide.md", "docs/tools.md", "AGENTS.md", "src/App.kt", "pnpm-lock.yaml"])
    await expect(window.getByTestId("review-concern-heading")).toHaveText(["Step 1 · ijproxy", "Step 2 · grep → accio", "Step 3 · Other changes"])
    const agents = window.locator('[data-testid="review-file-header"][data-path="AGENTS.md"]')
    await expect(agents.getByTestId("review-file-folded")).toHaveText("Generated from docs/guide.md by tools/render.mjs")
    await expect(agents.getByRole("button", { name: "Expand AGENTS.md" })).toHaveAttribute("aria-expanded", "false")
    const guideOfProxy = window.locator('[data-testid="diff-view-file"][data-section="file:docs/guide.md"]')
    await expect(guideOfProxy).toContainText("- `ijproxy`: the IDE through `ij-mcp-proxy`")
    await expect(guideOfProxy).not.toContainText("Search the source with `grep`.")

    // the row "also in" selects the guide, not counted as a file of its own, and shows its part of ijproxy: J goes on in
    // ijproxy from there, into the opening of the next concern at its end, then its first card, and the tree follows; K
    // goes back
    await row("docs/guide.md, also in ijproxy").click()
    await expect(row("docs/guide.md, also in ijproxy")).toHaveAttribute("aria-selected", "true")
    await expect(row("docs/guide.md")).toHaveAttribute("aria-selected", "false")
    await expect(position).toHaveText(/File 4 \/ 8$/)
    await window.keyboard.press("j")
    await expect(row("tools/codex/config.toml")).toHaveAttribute("aria-selected", "true")
    await window.keyboard.press("j")
    await expect(row(".mcp.json")).toHaveAttribute("aria-selected", "true")
    await window.keyboard.press("j")
    await expect(row("grep → accio")).toHaveAttribute("aria-selected", "true")
    await window.keyboard.press("j")
    await expect(row("docs/guide.md")).toHaveAttribute("aria-selected", "true")
    await expect(position).toHaveText(/File 4 \/ 8$/)
    await window.keyboard.press("k")
    await expect(row("grep → accio")).toHaveAttribute("aria-selected", "true")
    await window.keyboard.press("k")
    await expect(row(".mcp.json")).toHaveAttribute("aria-selected", "true")

    // the guide's own row shows its part of grep → accio: J goes on to the list of tools
    await row("docs/guide.md").click()
    await window.keyboard.press("j")
    await expect(row("docs/tools.md")).toHaveAttribute("aria-selected", "true")

    // the filter keeps the concerns of the files it finds
    await window.getByRole("textbox", { name: "Filter files" }).fill("tools")
    await expect.poll(rows).toEqual(["ijproxy", "tools/codex/config.toml", "grep → accio", "docs/tools.md"])
  })

  test("should show a repeated edit once under its group, the other files of the edit collapsed, and a file's own change in its section with a row to the edit", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    // the same edit in Alpha, Beta, Gamma and Delta, Delta with a change of its own
    turnOnReviewPlan(await setupRepo("repeated_edit"))
    const window = await openPlanWindow(page, syncAndWaitForBranches, "rename")
    const section = (name: string, group: boolean) =>
      window.locator(`[data-testid="diff-view-file"][data-path="src/a/${name}.kt"]${group ? '[data-section^="group:"]' : `[data-section="file:src/a/${name}.kt"]`}`)
    const header = (name: string, group: boolean) => section(name, group).getByTestId("review-file-header")
    // the divider of a group, or the opening of the step the group starts
    const divider = (name: string) =>
      window.locator(`:is([data-testid="diff-view-divider"], [data-testid="diff-view-opening"])[data-section^="group:"][data-section$=":src/a/${name}.kt"]`)
    await expect(
      window
        .getByRole("tree", { name: "Changed files" })
        .getByRole("treeitem", { name: /Alpha\.kt$/ })
        .first(),
    ).toHaveAttribute("aria-description", /same edit ×4/)
    // the story: the edit once, under its heading on Alpha; then the change of Delta's own
    await expect.poll(() => reviewedPaths(window)).toEqual(["src/a/Alpha.kt", "src/a/Beta.kt", "src/a/Delta.kt", "src/a/Gamma.kt", "src/a/Delta.kt"])
    // the heading of the group on the canvas above the card of its first file, never in a header
    await expect(divider("Alpha").getByTestId("review-section-heading")).toHaveText("Same edit in 4 files")
    await expect(window.getByTestId("review-file-header").getByTestId("review-section-heading")).toHaveCount(0)
    await expect(divider("Beta")).toHaveCount(0)
    // an opening is measured in place before the frame paints: the layout has its height by the next frame
    await expect
      .poll(async () => {
        const [above, card] = await Promise.all([divider("Alpha").boundingBox(), section("Alpha", true).boundingBox()])
        return Math.round(card!.y - (above!.y + above!.height))
      })
      .toBe(0)
    await expect(header("Alpha", true).getByRole("button", { name: "Collapse Alpha.kt" })).toHaveAttribute("aria-expanded", "true")
    for (const name of ["Beta", "Gamma", "Delta"]) {
      await expect(header(name, true).getByTestId("review-file-folded")).toHaveText("Same edit as Alpha.kt")
      await expect(header(name, true).getByRole("button", { name: `Expand ${name}.kt` })).toHaveAttribute("aria-expanded", "false")
    }

    // the edit is one concern, Delta's own change in what no evidence joined: its row there shows that change, open; its
    // edit is one row there, which opens Delta in the group
    await window.getByRole("treeitem", { name: "Delta.kt, also in Other changes", exact: true }).click()
    await expect(header("Delta", false).getByRole("button", { name: "Collapse Delta.kt" })).toHaveAttribute("aria-expanded", "true")
    await expect(header("Delta", false).getByTestId("review-file-folded")).toHaveCount(0)
    await section("Delta", false).getByTestId("diff-unit-elsewhere").click()
    await expect(header("Delta", true).getByRole("button", { name: "Collapse Delta.kt" })).toHaveAttribute("aria-expanded", "true")

    // K and J step through the sections, the tree following
    const tree = window.getByRole("tree", { name: "Changed files" })
    await window.getByTestId("commit-review-code").locator("[aria-label='Changes of the commit']").focus()
    await window.keyboard.press("k")
    await expect(tree.getByRole("treeitem", { name: "src/a/Beta.kt", exact: true })).toHaveAttribute("aria-selected", "true")
    await window.keyboard.press("j")
    await expect(tree.getByRole("treeitem", { name: "src/a/Delta.kt", exact: true })).toHaveAttribute("aria-selected", "true")

    // the chevron opens a folded file
    await header("Beta", true).getByRole("button", { name: "Expand Beta.kt" }).click()
    await expect(header("Beta", true).getByRole("button", { name: "Collapse Beta.kt" })).toHaveAttribute("aria-expanded", "true")
    await expect(window.getByTestId("diff-view")).toContainText("class Beta")
  })

  test("should show a block of the imports alone in the group of the imports, in its file's section one row that opens it, and pass it with N", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("symbol_navigation")
    await setupPlan(repo, { experimental: true })
    // Base.kt gets an import above its function and a function below it: two blocks, the first of the imports alone
    await repo.commit("(plan) add other", { "src/Base.kt": "import kotlin.math.max\n\nfun outsider(value: Int) = helper(value)\n\nfun other() = max(1, 2)\n" }, 1704124800)
    const window = await openPlanWindow(page, syncAndWaitForBranches)
    const own = window.locator('[data-testid="diff-view-file"][data-section="file:src/Base.kt"]')
    const imports = window.locator('[data-testid="diff-view-file"][data-section="group:mechanical:importsOnly:src/Base.kt"]')
    await expect.poll(() => reviewedPaths(window)).toEqual(["src/b/Cache.kt", "src/a/Reader.kt", "src/Base.kt", "src/Store.kt", "src/Base.kt"])

    // Base.kt in the tree: its new function under its heading, the import one row that names the group
    const position = window.getByTestId("review-position")
    await window.getByRole("treeitem", { name: "Base.kt" }).click()
    await expect(position).toHaveText(/File 3 \/ 4$/)
    await expect(own.getByTestId("diff-unit-heading")).toHaveText("other — new")
    await expect(own).toContainText("fun other() = max(1, 2)")
    await expect(own).not.toContainText("import kotlin.math.max")
    await expect(own.getByTestId("diff-unit-elsewhere")).toHaveText("Shown in Imports only (1)")

    // from the top of Base.kt, N goes to the function, then to Store.kt: the import is no change of this section
    await window.getByTestId("commit-review-code").locator("[aria-label='Changes of the commit']").focus()
    await window.keyboard.press("n")
    await expect(position).toHaveText(/File 3 \/ 4$/)
    await window.keyboard.press("n")
    await expect(position).toHaveText(/File 4 \/ 4$/)

    // the row opens the group of the imports, collapsed until then: the import is pale there, as whitespace alone
    await expect(imports.getByTestId("review-file-folded")).toHaveText("Imports only")
    await own.getByTestId("diff-unit-elsewhere").click()
    await expect(imports).toContainText("import kotlin.math.max")
    await expect(imports.locator('[data-tint="whitespace"]')).toHaveCount(2)
  })

  test("should show after a changed API the uses it left outside, each its file at the head around the use, none that the head lacks", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("symbol_navigation")
    await setupPlan(repo, { experimental: true })
    const window = await openPlanWindow(page, syncAndWaitForBranches)
    // after Store.kt, where `helper` takes a scale: its use in Base.kt, which the change left as it was; the index names
    // two more in files the head lacks, which show nothing
    const ghosts = window.locator('[data-testid="diff-view-file"][data-section^="ghost:"]')
    // once the plan came: the story is laid out
    await expect(window.getByTestId("review-plan-breakage")).toContainText("Possible Breakage")
    // the name of the file, beside its badges, which open their popovers
    await window.getByRole("treeitem", { name: "Store.kt" }).getByText("Store.kt", { exact: true }).click()
    await expect.poll(() => ghosts.evaluateAll(sections => sections.map(section => section.getAttribute("data-path")))).toEqual(["src/Base.kt"])
    const base = ghosts.first()
    await expect(window.locator('[data-testid="diff-view-divider"][data-section^="ghost:"]').getByTestId("review-section-heading")).toHaveText("Left outside")
    await expect(base.getByTestId("diff-unit-heading")).toHaveText("helper — left outside: src/Base.kt:1")
    await expect(base).toContainText("helper(value)")
    // the tree keeps the selected file of the review, a use outside has no row there
    await expect(window.getByRole("treeitem", { name: "Store.kt" })).toHaveAttribute("aria-selected", "true")
  })

  test("should show the plan of a commit shown alone from its row in the tree: its overview and the story of its diffs @webkit", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const window = await openPlanCommits(page, await setupRepo("symbol_navigation"), syncAndWaitForBranches)
    await commitRow(window, "scale the helper").click()
    await commitRow(window, "scale the helper").press("Enter")
    await expect.poll(() => reviewedPaths(window)).toEqual(["src/Store.kt"])
    await expect(window.getByTestId("range-review-commit-header")).toContainText("scale the helper")
    // the plan of that commit alone: the change of `helper` under its heading, its uses left outside in the overview
    // below the commit
    const store = window.locator('[data-testid="diff-view-file"][data-section="file:src/Store.kt"]')
    await expect(store.getByTestId("diff-unit-heading")).toHaveText("helper — form changed: parameters")
    const overview = window.getByTestId("review-overview")
    await expect(overview.getByTestId("range-review-commit-header")).toContainText("scale the helper")
    await expect(overview.getByTestId("review-plan-breakage-entry")).toHaveCount(1)
    await expect(overview.getByTestId("review-plan-outside")).toHaveText("3 uses left outside")
    await expect(overview.getByTestId("review-step-new-api")).toHaveCount(0)
    // All Changes again: the plan of the branch, with the declarations the other commit adds
    await window.getByTestId("range-review-commit").filter({ hasText: "All Changes" }).click()
    await expect(overview.getByTestId("review-step-new-api")).toHaveText(["Reader", "Cache"])
  })

  test("should show the plan of a commit in the review window of a commit: its overview and the story of its diffs", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("symbol_navigation")
    await setupPlan(repo, { experimental: true })
    // a commit of no branch: D opens the review window of the commit
    await repo.commit("scale the helper again", { "src/Store.kt": storeText("fun helper(value: Int, scale: Int, offset: Int) = value * scale + offset") }, 1704124800)
    await syncAndWaitForBranches()
    await openCommitPreview(page, getUnassignedCommitRow(page, "scale the helper again"))
    await page.keyboard.press("d")
    await expect.poll(async () => (await getOpenedSubWindows(page)).map(opened => opened.windowId)).toEqual(["review"])
    const window = await openLinkedSubWindow(page, (await getOpenedSubWindows(page))[0]!)
    const store = window.locator('[data-testid="diff-view-file"][data-section="file:src/Store.kt"]')
    await expect(store.getByTestId("diff-unit-heading")).toHaveText("helper — form changed: parameters")
    await expect(window.getByTestId("review-overview").getByTestId("review-plan-outside")).toHaveText("3 uses left outside")
  })

  test("should keep the review on screen after a sync that moved the head until the files, the diffs and the plan of the new head are read, then show them at once", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("symbol_navigation")
    const window = await openPlanCommits(page, repo, syncAndWaitForBranches)
    const position = window.getByTestId("review-position")
    await window.getByRole("treeitem", { name: "Reader.kt" }).click()
    await expect(window.getByRole("treeitem", { name: "Reader.kt" })).toHaveAttribute("aria-selected", "true")
    const shownPosition = await position.textContent()
    const shownPaths = await reviewedPaths(window)

    // a commit on top, then a sync: the window reads the new head, its plan held
    const plans = await holdCommandCalls(window, "get_review_plan")
    await repo.commit("(plan) read the cache twice", { "src/a/Reader.kt": READER('cache.get("x") + cache.get("y")') }, 1704124800)
    await syncAndWaitForBranches()
    await expect.poll(() => plans.calls.length).toBe(1)
    // the review as it was: the story of the tree with its commits, the column, the diffs and the file on screen
    expect(await treeCommits(window)).toEqual(["add the cache", "scale the helper"])
    expect(await commitColumn(window)).toEqual(["All Changes2 commits"])
    expect(await reviewedPaths(window)).toEqual(shownPaths)
    await expect(position).toHaveText(shownPosition!)
    await expect(window.getByTestId("branch-review-body")).toHaveAttribute("aria-busy", "true")

    // the plan of the new head comes: the new review shows in one change, at the same file
    plans.release()
    await expect.poll(() => treeCommits(window)).toEqual(["read the cache twice", "add the cache", "scale the helper"])
    expect(await commitColumn(window)).toEqual(["All Changes3 commits"])
    await expect(window.getByTestId("branch-review-body")).not.toHaveAttribute("aria-busy")
    await expect(window.getByRole("treeitem", { name: "Reader.kt" })).toHaveAttribute("aria-selected", "true")
  })

  test("should show only the commits added or rewritten since Mark Reviewed, as one range on top or apart", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("symbol_navigation")
    // never marked: no Since Your Review; the column keeps the scope, the tree lists the commits
    const window = await openPlanCommits(page, repo, syncAndWaitForBranches)

    // Mark Reviewed reads the plan again: its steps and their commits stay in the tree while it is read
    const plans = await holdCommandCalls(window, "get_review_plan")
    await markReviewed(window)
    await expect.poll(() => plans.calls.length).toBe(1)
    expect(await treeCommits(window)).toEqual(["add the cache", "scale the helper"])
    plans.release()

    // a commit on top, then a sync: Since Your Review is its range, the files of that commit alone
    await repo.commit("(plan) read the cache twice", { "src/a/Reader.kt": READER('cache.get("x") + cache.get("y")') }, 1704124800)
    await syncAndWaitForBranches()
    await expect.poll(() => treeCommits(window)).toEqual(["read the cache twiceNew", "add the cache", "scale the helper"])
    await expect.poll(() => commitColumn(window)).toEqual(["All Changes3 commits", "Since Your Review1 commit"])
    await window.getByTestId("range-review-commit").filter({ hasText: "Since Your Review" }).click()
    await expect.poll(() => treeCommits(window)).toEqual(["read the cache twiceNew"])
    await expect.poll(() => reviewedPaths(window)).toEqual(["src/a/Reader.kt"])
    await expect(window.getByTestId("review-plan-since")).toHaveText("Since your review: 1 new")
    await expect(window.getByTestId("review-position")).toHaveText(/^Since Your Review · File 1 \/ 1$/)

    // the commit of nav joins plan below the reviewed ones: the new commits are not one range of the branch, so Since
    // Your Review is the diff from the reviewed change, both new commits together
    await repo.reword(await repo.revision("HEAD~3"), "(plan) use the store and the queue")
    await syncAndWaitForBranches()
    await expect(findBranchRow(page, "nav")).toHaveCount(0)
    await expect.poll(() => treeCommits(window)).toEqual(["read the cache twiceNew", "use the store and the queueNew"])
    await expect.poll(() => commitColumn(window)).toEqual(["All Changes4 commits", "Since Your Review2 commits"])
    await expect.poll(() => reviewedPaths(window)).toEqual(["src/a/Reader.kt", "src/Queue.kt", "src/Store.kt", "src/Uses.kt"])
    await expect(window.getByTestId("review-plan-since")).toHaveText("Since your review: 2 new")
    await commitRow(window, "use the store and the queue").click()
    await commitRow(window, "use the store and the queue").press("Enter")
    await expect.poll(() => reviewedPaths(window)).toEqual(["src/Queue.kt", "src/Store.kt", "src/Uses.kt"])
    // a commit alone: the column lists every commit
    await expect.poll(() => commitColumn(window)).toEqual(["All Changes4 commits", "Since Your Review2 commits", "read the cache twiceNew", "use the store and the queueNew"])

    // All Changes lists every commit again; marked again, nothing is new: no Since Your Review
    await window.getByTestId("range-review-commit").filter({ hasText: "All Changes" }).click()
    await expect.poll(() => treeCommits(window)).toHaveLength(4)
    await runAndWaitForAnswer(window, "mark_branch_reviewed", () => window.getByTestId("branch-review-mark-reviewed").click())
    await expect.poll(() => commitColumn(window)).toEqual(["All Changes4 commits"])
    await expect.poll(() => treeCommits(window)).toEqual(["read the cache twice", "add the cache", "scale the helper", "use the store and the queue"])
  })

  test("should show a reviewed commit amended since as Rewritten, and Since Your Review as the diff from its reviewed version", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("symbol_navigation")
    await setupPlan(repo, { experimental: true })
    const window = await openPlanWindow(page, syncAndWaitForBranches)
    await markReviewed(window)

    // an agent amends the reviewed `scale the helper`: a fixup, folded by Squash Fixups of the main window
    await repo.commit("fixup! (plan) scale the helper", { "src/Store.kt": storeText("fun helper(value: Int, scale: Int) = value * scale * 2") }, 1704124800)
    await syncAndWaitForBranches()
    await openContextMenu(page, findBranchRow(page, "plan"))
    await clickContextMenuItem(page, "Squash 1 Fixup…")
    await page.getByTestId("squash-fixups-confirm").click()
    await expect(page.getByText("Squashed 1 fixup")).toBeVisible()

    // the amended commit is Rewritten, the one above it, replayed with the same changes, is not
    await expect.poll(() => treeCommits(window)).toEqual(["add the cache", "scale the helperRewritten"])
    await expect.poll(() => commitColumn(window)).toEqual(["All Changes2 commits", "Since Your Review1 commit"])
    // the card of All Changes says what the rewrite changed against the reviewed version, per path
    const since = window.getByTestId("review-plan-since")
    await expect(since).toContainText("Since your review: 1 rewritten")
    await expect(since).toContainText("Rewritten scale the helper: src/Store.kt +1 −1")

    // Since Your Review: one diff from the reviewed version, the amend alone: the reviewed line against the new one,
    // never the line before the branch, under the card with that difference
    await window.getByTestId("range-review-commit").filter({ hasText: "Since Your Review" }).click()
    await expect(window.getByTestId("review-position")).toHaveText(/^Since Your Review · File 1 \/ 1$/)
    await expect.poll(() => treeCommits(window)).toEqual(["scale the helperRewritten"])
    await expect.poll(() => reviewedPaths(window)).toEqual(["src/Store.kt"])
    const code = window.getByTestId("diff-view")
    await expect(code).toContainText("value * scale * 2")
    await expect(code).not.toContainText("helper(value: Int)")
    await expect(since).toContainText("Rewritten scale the helper: src/Store.kt +1 −1")
    await captureHtmlSnapshot(window.getByTestId("review-plan-since"), "review-plan-since-rewritten")
  })

  test("should look as it does without the review plan while the flag is off", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupPlan(await setupRepo("symbol_navigation"), { experimental: false })
    await syncAndWaitForBranches()
    let plans = 0
    page.on("request", request => {
      if (request.url().endsWith("/invoke/get_review_plan")) {
        plans++
      }
    })
    await openSubWindowPage(page, await openReviewWindow(page, "plan"))
    // the review is there: its actions, its files in the tree, the header of the files
    await expect(page.getByTestId("review-file-header").first()).toBeVisible()
    await expect(page.getByTestId("branch-review-actions")).toBeVisible()
    await expect(page.getByTestId("branch-review-files")).toHaveText("Files3")
    await expect.poll(() => reviewedPaths(page)).toEqual(["src/a/Reader.kt", "src/b/Cache.kt", "src/Store.kt"])
    // and nothing of the plan: no Mark Reviewed, no overview, no badges of the files, no Since Your Review, no read of a plan
    await expect(page.getByRole("tree", { name: "Changed files" }).getByRole("treeitem", { name: "Store.kt" })).toBeVisible()
    await expect(page.getByTestId("branch-review-mark-reviewed")).toHaveCount(0)
    await expect(page.getByTestId("review-overview")).toHaveCount(0)
    await expect(page.getByTestId("file-row-mark")).toHaveCount(0)
    await expect.poll(() => commitColumn(page)).toEqual(["All Changes2 commits", "add the cache", "scale the helper"])
    expect(plans).toBe(0)
  })
})
