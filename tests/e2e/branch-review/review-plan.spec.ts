// @spec spec/branch/review-plan.spec.md
import { appendFileSync, writeFileSync } from "node:fs"
import path from "node:path"
import type { Page } from "@playwright/test"
import type { BranchReviewWindowData } from "../../../src/types/sub-window"
import { test, expect } from "../fixtures/test-fixtures"
import { captureHtmlSnapshot } from "../helpers/aria-snapshot-helpers"
import { findBranchRow, getUnassignedCommitRow } from "../helpers/branch-helpers"
import { openCommitPreview } from "../helpers/preview-helpers"
import { clickContextMenuItem, openContextMenu, selectedInDiffs } from "../helpers/selection-helpers"
import { getOpenedSubWindows, openLinkedSubWindow, openSubWindowPage } from "../helpers/sub-window-helpers"
import { runAndWaitForAnswer } from "../helpers/sync-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"
import { closeToast } from "../helpers/toast-helpers"

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

const READER = (body: string) => `package a\n\nimport b.Cache\n\nclass Reader {\n  fun read(cache: Cache) = ${body}\n}\n`

/** The review plan on for the repository: the flag is off when unset, and the global config of the test machine says so too (`TEST_IDENTITY` of the server) */
function turnOnReviewPlan(repo: TestRepositoryBuilder): void {
  // `git config branchdeck.reviewExperimental true` in a terminal: the next sync reads it, and it wins over the global
  appendFileSync(path.join(repo.path, ".git", "config"), "[branchdeck]\n\treviewExperimental = true\n")
}

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
  // the index of the repository, as accio answers `references helper`
  const usage = (file: string, line: number) => ({ file, line, column: 3, snippet: "helper(1)", kind: "call" })
  writeFileSync(
    path.join(repo.path, ".git", "branch-deck-accio.json"),
    JSON.stringify({ references: { helper: [usage("src/Base.kt", 1), usage("src/Uses.kt", 4), usage("src/main/App.kt", 9)] } }),
  )
}

/** The paths of the diffs the review shows of its files, in their order: no use left outside */
async function reviewedPaths(page: Page): Promise<string[]> {
  return page
    .locator('[data-testid="diff-view-file"]:not([data-section^="ghost:"]) [data-testid="review-file-header"]')
    .evaluateAll(headers => headers.map(header => header.getAttribute("data-path")!))
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

/** "Review Changes" of the row of `branch`: the review window it opens */
async function openReviewWindow(page: Page, branch: string) {
  await findBranchRow(page, branch).click({ button: "right" })
  await page.getByRole("menuitem", { name: "Review Changes" }).click()
  await expect.poll(async () => (await getOpenedSubWindows(page)).length).toBe(1)
  return (await getOpenedSubWindows<BranchReviewWindowData>(page))[0]!
}

/** The review window of `branch` (`plan`) after a sync, as a page of its own beside the main window, its diffs shown */
async function openPlanWindow(page: Page, syncAndWaitForBranches: () => Promise<unknown>, branch = "plan"): Promise<Page> {
  await syncAndWaitForBranches()
  const window = await openLinkedSubWindow(page, await openReviewWindow(page, branch))
  await expect(window.getByTestId("review-file-header").first()).toBeVisible()
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

/** Mark Reviewed of the title bar of the window: it keeps the base and the tip of `plan` and says so */
async function markReviewed(window: Page): Promise<void> {
  await runAndWaitForAnswer(window, "mark_branch_reviewed", () => window.getByTestId("branch-review-mark-reviewed").click())
  await expect(window.getByText("Marked plan reviewed")).toBeVisible()
}

test.describe("The review plan in the branch review", () => {
  test("should lead the Review Summary with what can break, show the declaration of an entry, and keep the files in the tree", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    await setupPlan(await setupRepo("symbol_navigation"), { experimental: true })
    const window = await openPlanWindow(page, syncAndWaitForBranches)
    const { store, cache } = planRows(window)

    // the card above the diffs of All Changes: the changed `helper` with its 3 uses outside the change, from accio
    const card = window.getByTestId("review-plan-card")
    await expect(card.getByTestId("review-plan-summary")).toHaveText(/^1 possible break · \d+ new declarations$/)
    const helper = card.getByTestId("review-plan-breakage-entry")
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
    // the new API starts collapsed, per file
    const newApi = card.getByTestId("review-plan-new-api")
    await expect(newApi.getByTestId("review-plan-new-api-file")).toHaveCount(0)
    await captureHtmlSnapshot(card, "review-plan-card")

    // a use left outside opens its file at the head over the review, the name selected; one the head lacks says so
    const fileView = window.getByTestId("declaration-file-view")
    await helper.getByTestId("review-plan-site").filter({ hasText: "src/Base.kt:1" }).click()
    await expect(fileView).toContainText("outside the review")
    await expect.poll(() => selectedInDiffs(window)).toEqual({ text: "helper", path: "src/Base.kt" })
    await helper.getByTestId("review-plan-site").filter({ hasText: "src/main/App.kt:9" }).click()
    const missing = window.locator("[data-slot=toast]", { hasText: "Cannot open src/main/App.kt:9" })
    await expect(missing).toContainText("The head of the review has no such line")
    await closeToast(missing)

    // an entry selects its file and shows its declaration, the name selected, over a file opened outside the review
    await helper.getByTestId("review-plan-breakage-file").click()
    await expect(fileView).toBeHidden()
    await expect(store).toHaveAttribute("aria-selected", "true")
    await expect.poll(() => selectedInDiffs(window)).toEqual({ text: "helper", path: "src/Store.kt" })
    await newApi.getByRole("button", { name: /New API/ }).click()
    await expect
      .poll(() => newApi.getByTestId("review-plan-new-api-file").evaluateAll(files => files.map(file => file.getAttribute("data-path"))))
      .toEqual(["src/a/Reader.kt", "src/b/Cache.kt"])
    await newApi.getByTestId("review-plan-new-api-file").filter({ hasText: "src/b/Cache.kt" }).getByRole("button").click()
    await expect(cache).toHaveAttribute("aria-selected", "true")
    await expect.poll(() => selectedInDiffs(window)).toEqual({ text: "Cache", path: "src/b/Cache.kt" })
    await expect(window.getByTestId("branch-review-position")).toHaveText(/File 2 \/ 3$/)

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

    const card = window.getByTestId("review-plan-card")
    await expect(card.getByTestId("review-plan-summary")).toHaveText(/^1 possible break/)
    const helper = card.getByTestId("review-plan-breakage-entry")
    await expect(helper).toHaveAttribute("data-name", "helper")
    await expect(helper.getByTestId("review-plan-outside")).toHaveText("The uses are unknown")
    await expect(card.getByTestId("review-plan-updated")).toHaveCount(0)
    await expect(card.getByTestId("review-plan-index-note")).toContainText("No usages:")
    await expect(planRows(window).store.getByTestId("file-breaks-badge")).toHaveText("breaks ?")
  })

  test("should badge each file with its links, the uses it breaks and no test, and show a linked file from its popover, a file that uses at its first use", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    await setupPlan(await setupRepo("symbol_navigation"), { experimental: true })
    const window = await openPlanWindow(page, syncAndWaitForBranches)
    const { tree, reader, cache, store } = planRows(window)

    // Reader imports the new Cache: a link the change added; Store's `helper` keeps 3 uses outside the change
    await expect(reader.getByTestId("file-links-badge")).toHaveText("uses 1+1 new")
    await expect(cache.getByTestId("file-links-badge")).toHaveText("used by 1")
    await expect(store.getByTestId("file-breaks-badge")).toHaveText("breaks 3")
    await expect(store.getByTestId("file-links-badge")).toHaveCount(0)
    for (const row of [reader, cache, store]) {
      await expect(row.getByTestId("file-untested-badge")).toHaveText("no test")
    }
    await captureHtmlSnapshot(tree, "review-plan-tree-badges")

    // the popover of Reader lists the link to Cache with its name and state; a click selects Cache
    await reader.getByTestId("file-links-badge").click()
    const popover = window.getByTestId("file-links-badge-popover")
    const linked = popover.getByTestId("file-link")
    await expect(linked).toHaveCount(1)
    await expect(linked).toHaveAttribute("data-path", "src/b/Cache.kt")
    await expect(linked).toHaveAttribute("data-state", "added")
    await expect(linked).toContainText("Cache, get · Reader.kt:3")
    await expect(cache).toHaveAttribute("aria-selected", "false")
    await linked.click()
    await expect(cache).toHaveAttribute("aria-selected", "true")
    await expect(popover).toHaveCount(0)
    await expect(window.getByTestId("branch-review-position")).toHaveText(/File 2 \/ 3$/)

    // the popover of Cache lists Reader, which uses it: a click shows its first use, the name selected
    await cache.getByTestId("file-links-badge").click()
    await popover.getByTestId("file-link").click()
    await expect(reader).toHaveAttribute("aria-selected", "true")
    await expect.poll(() => selectedInDiffs(window)).toEqual({ text: "Cache", path: "src/a/Reader.kt" })
  })

  test("should select a linked file that a filter hides, and drop only the filter that hides it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("symbol_navigation")
    const window = await openPlanWindowWithFilters(page, repo, syncAndWaitForBranches)
    const { reader, cache } = planRows(window)
    const owner = (name: string) => window.getByTestId("branch-review-owner").filter({ hasText: name })
    const search = window.getByRole("textbox", { name: "Filter files" })
    const selectCacheFromReader = async () => {
      await reader.getByTestId("file-links-badge").click()
      await window.getByTestId("file-links-badge-popover").getByTestId("file-link").click()
    }

    const position = window.getByTestId("branch-review-position")
    const ownerAwayAndBack = async () => {
      await owner("Monorepo").click()
      await expect(cache).toHaveCount(0)
      await owner("All").click()
      await expect(owner("All")).toHaveAttribute("aria-pressed", "true")
      await expect(cache).toBeVisible()
    }

    // the owner filter hides Cache: a click on its link shows all files and selects it
    await owner("Monorepo").click()
    await expect(cache).toHaveCount(0)
    await selectCacheFromReader()
    await expect(owner("All")).toHaveAttribute("aria-pressed", "true")
    await expect(cache).toBeVisible()
    await expect(cache).toHaveAttribute("aria-selected", "true")
    await expect(position).toHaveText(/File 7 \/ 8$/)
    // the link selects Cache once: the owner filter away and back starts at the first file
    await ownerAwayAndBack()
    await expect(cache).toHaveAttribute("aria-selected", "false")
    await expect(position).toHaveText(/File 1 \/ 8$/)

    // the owner filter hides Cache and the search shows it: the owner filter goes, the search stays
    // the search first: the owner filter leaves too few files for its field
    await search.fill(".kt")
    await owner("Monorepo").click()
    await expect.poll(() => reviewedPaths(window)).toEqual(["src/a/Reader.kt", "src/Store.kt"])
    await selectCacheFromReader()
    await expect(owner("All")).toHaveAttribute("aria-pressed", "true")
    await expect(search).toHaveValue(".kt")
    await expect.poll(() => reviewedPaths(window)).toEqual(["src/b/Cache.kt", "src/a/Reader.kt", "src/Store.kt"])
    await expect(cache).toBeVisible()
    await expect(cache).toHaveAttribute("aria-selected", "true")
    await expect(position).toHaveText(/File 2 \/ 3$/)

    // a search that hides Cache is cleared, and Cache selected; the search does not select it again
    await search.fill("Reader")
    await expect(cache).toHaveCount(0)
    await selectCacheFromReader()
    await expect(search).toHaveValue("")
    await expect(cache).toBeVisible()
    await expect(cache).toHaveAttribute("aria-selected", "true")
    await expect(position).toHaveText(/File 7 \/ 8$/)
    // the name of the row: its middle is a badge
    await reader.getByText("Reader.kt", { exact: true }).click()
    await expect(reader).toHaveAttribute("aria-selected", "true")
    await search.fill(".kt")
    await expect(reader).toHaveAttribute("aria-selected", "true")
    await search.fill("")
    await expect(reader).toHaveAttribute("aria-selected", "true")
    await ownerAwayAndBack()
    await expect(cache).toHaveAttribute("aria-selected", "false")
  })

  test("should select the file of a card entry while a filter hides every file, and drop only the filter that hides it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("symbol_navigation")
    const window = await openPlanWindowWithFilters(page, repo, syncAndWaitForBranches)
    const { cache, store } = planRows(window)
    const owner = (name: string) => window.getByTestId("branch-review-owner").filter({ hasText: name })
    const search = window.getByRole("textbox", { name: "Filter files" })
    const position = window.getByTestId("branch-review-position")
    const noMatch = window.getByTestId("review-files-no-match")
    const card = window.getByTestId("review-plan-card")
    const header = (filePath: string) => window.locator(`[data-testid="review-file-header"][data-path="${filePath}"]`)

    // a search that matches no file: the card stays, its entry clears the search and selects Store
    await search.fill("nothing matches this")
    await expect(noMatch).toBeVisible()
    await card.getByTestId("review-plan-breakage-file").click()
    await expect(search).toHaveValue("")
    await expect(noMatch).toHaveCount(0)
    await expect(store).toHaveAttribute("aria-selected", "true")
    await expect(position).toHaveText(/File 8 \/ 8$/)
    await expect(header("src/Store.kt")).toBeInViewport()
    await expect.poll(() => selectedInDiffs(window)).toEqual({ text: "helper", path: "src/Store.kt" })

    // the owner filter and a search that hides its only file: the entry of Cache drops the search, the owner stays
    await search.fill("nothing matches this")
    await owner("Cache Team").click()
    await expect(noMatch).toBeVisible()
    const newApi = card.getByTestId("review-plan-new-api")
    await newApi.getByRole("button", { name: /New API/ }).click()
    await newApi.getByTestId("review-plan-new-api-file").filter({ hasText: "src/b/Cache.kt" }).getByRole("button").click()
    // the search is empty, and one file is too few for its field
    await expect(search).toHaveCount(0)
    await expect(owner("Cache Team")).toHaveAttribute("aria-pressed", "true")
    await expect(cache).toHaveAttribute("aria-selected", "true")
    await expect(position).toHaveText(/File 1 \/ 1$/)
    await expect.poll(() => selectedInDiffs(window)).toEqual({ text: "Cache", path: "src/b/Cache.kt" })

    // the search keeps only Store and the owner filter hides it, so no file shows: the owner filter goes, the search stays
    await owner("All").click()
    await search.fill("Store")
    await owner("Cache Team").click()
    await expect(noMatch).toBeVisible()
    await card.getByTestId("review-plan-breakage-file").click()
    await expect(owner("All")).toHaveAttribute("aria-pressed", "true")
    await expect(search).toHaveValue("Store")
    await expect.poll(() => reviewedPaths(window)).toEqual(["src/Store.kt"])
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
      JSON.stringify({ references: { helper: [{ file: "src/Base.kt", line: 1, column: 31, snippet: "helper(value)", kind: "call" }] } }),
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
    await expect(module.getByTestId("directory-dependency-badge")).toHaveText("new module")
    await module.getByTestId("directory-dependency-badge").hover()
    await expect(window.getByTestId("directory-dependency-changes")).toHaveText("New module b")
    await captureHtmlSnapshot(tree, "review-plan-tree-mechanical")
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
    await expect(reader.getByTestId("file-same-edit-badge")).toHaveText("same edit ×3")
    await reader.getByTestId("file-same-edit-badge").click()
    const popover = window.getByTestId("file-same-edit-badge-popover")
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

    // the card: no possible break, `helper` folded with every use updated; the new module and the link into it; the
    // same edit, collapsed
    const card = window.getByTestId("review-plan-card")
    await expect(card.getByTestId("review-plan-summary")).toHaveText(
      /^No possible break · 1 dependency change · 1 new link between modules · 1 repeated edit · \d+ new declarations$/,
    )
    await expect(card.getByTestId("review-plan-breakage-entry")).toHaveCount(0)
    await expect(card.getByTestId("review-plan-updated")).toContainText("1 API change, every use updated in this change")
    const dependencies = card.getByTestId("review-plan-dependencies")
    await expect(dependencies.getByTestId("review-plan-dependency-module")).toHaveAttribute("data-manifest", "src/b/b.iml")
    await expect(dependencies.getByTestId("review-plan-dependency")).toHaveText("New module b")
    const moduleLink = dependencies.getByTestId("review-plan-module-link")
    await expect(moduleLink).toHaveAttribute("data-from", "src/a/Reader.kt")
    await expect(moduleLink).toContainText("Reader.kt → Cache.kt")
    const repeated = card.getByTestId("review-plan-repeated")
    await expect(repeated.getByTestId("review-plan-repeated-edit")).toHaveCount(0)
    await repeated.getByRole("button", { name: /Repeated Edits/ }).click()
    const edit = repeated.getByTestId("review-plan-repeated-edit")
    await expect(edit).toContainText("Same edit in 3 files:")
    await expect(edit.locator("code")).toHaveText("+ // read by the review plan of the branch")
    await expect(edit.getByTestId("review-plan-repeated-file")).toHaveText(["Reader.kt", "Cache.kt", "Store.kt"])
    await captureHtmlSnapshot(card, "review-plan-card-dependencies")

    // a click selects the file of an entry: the link selects Reader, the module its build file in the Mechanical group
    await moduleLink.getByRole("button").click()
    await expect(reader).toHaveAttribute("aria-selected", "true")
    await dependencies.getByTestId("review-plan-file").first().click()
    await expect(tree.getByRole("treeitem", { name: "src/b/b.iml" })).toHaveAttribute("aria-selected", "true")
  })

  test("should fold the generated files into their group last, each header saying why, and keep the renderer's echo out of the repeated edits", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    // the source src/guide.md and its two renderings get the same edit; a JSON file .gitattributes marks, a lock file
    // and a minified file change too
    turnOnReviewPlan(await setupRepo("generated_files"))
    const window = await openPlanWindow(page, syncAndWaitForBranches, "search")
    const tree = window.getByRole("tree", { name: "Changed files" })
    // the real work, then the generated files in the order of the tree
    await expect.poll(() => reviewedPaths(window)).toEqual(["src/App.kt", "src/guide.md", "config/mcp.json", "dist/app.min.js", "docs/AGENTS.md", "AGENTS.md", "pnpm-lock.yaml"])
    // the echo of the guide is no edit a person repeated: no group, no badge
    await expect(tree.getByTestId("file-same-edit-badge")).toHaveCount(0)
    await expect(window.getByTestId("review-plan-repeated-file")).toHaveCount(0)
    // the first file of the tree without the plan, a generated one, gave way to the first of the real work
    await expect(tree.getByRole("treeitem", { name: "App.kt" })).toHaveAttribute("aria-selected", "true")
    const group = tree.getByRole("treeitem", { name: "Mechanical (5)" })
    await expect(group).toHaveAttribute("aria-expanded", "false")
    await group.click()
    await expect(tree.getByRole("treeitem", { name: "Generated (5)" })).toBeVisible()
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
    await expect(window.getByRole("tree", { name: "Changed files" }).getByTestId("file-same-edit-badge").first()).toHaveText("same edit ×4")
    // the story: the edit once, under its heading on Alpha; then the change of Delta's own
    await expect.poll(() => reviewedPaths(window)).toEqual(["src/a/Alpha.kt", "src/a/Beta.kt", "src/a/Delta.kt", "src/a/Gamma.kt", "src/a/Delta.kt"])
    await expect(header("Alpha", true).getByTestId("review-section-heading")).toHaveText("Same edit in 4 files")
    await expect(header("Alpha", true).getByRole("button", { name: "Collapse Alpha.kt" })).toHaveAttribute("aria-expanded", "true")
    for (const name of ["Beta", "Gamma", "Delta"]) {
      await expect(header(name, true).getByTestId("review-file-folded")).toHaveText("Same edit as Alpha.kt")
      await expect(header(name, true).getByRole("button", { name: `Expand ${name}.kt` })).toHaveAttribute("aria-expanded", "false")
    }

    // Delta in the tree shows its own change, open; its edit is one row there, which opens Delta in the group
    await window.getByRole("treeitem", { name: "Delta.kt" }).click()
    await expect(header("Delta", false).getByRole("button", { name: "Collapse Delta.kt" })).toHaveAttribute("aria-expanded", "true")
    await expect(header("Delta", false).getByTestId("review-file-folded")).toHaveCount(0)
    await section("Delta", false).getByTestId("diff-unit-elsewhere").click()
    await expect(header("Delta", true).getByRole("button", { name: "Collapse Delta.kt" })).toHaveAttribute("aria-expanded", "true")

    // K and J step through the sections, the tree following
    const tree = window.getByRole("tree", { name: "Changed files" })
    await window.getByTestId("commit-review-code").locator("[aria-label='Changes of the commit']").focus()
    await window.keyboard.press("k")
    await expect(tree.getByRole("treeitem", { name: "Beta.kt" })).toHaveAttribute("aria-selected", "true")
    await window.keyboard.press("j")
    await expect(tree.getByRole("treeitem", { name: "Delta.kt" })).toHaveAttribute("aria-selected", "true")

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
    const position = window.getByTestId("branch-review-position")
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
    await expect(window.getByTestId("review-plan-summary")).toHaveText(/possible break/)
    // the name of the file, beside its badges, which open their popovers
    await window.getByRole("treeitem", { name: "Store.kt" }).getByText("Store.kt", { exact: true }).click()
    await expect.poll(() => ghosts.evaluateAll(sections => sections.map(section => section.getAttribute("data-path")))).toEqual(["src/Base.kt"])
    const base = ghosts.first()
    await expect(base.getByTestId("review-section-heading")).toHaveText("Left outside")
    await expect(base.getByTestId("diff-unit-heading")).toHaveText("helper — left outside: src/Base.kt:1")
    await expect(base).toContainText("helper(value)")
    // the tree keeps the selected file of the review, a use outside has no row there
    await expect(window.getByRole("treeitem", { name: "Store.kt" })).toHaveAttribute("aria-selected", "true")
  })

  test("should show the plan of a commit chosen in the column: its card and the story of its diffs", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("symbol_navigation")
    await setupPlan(repo, { experimental: true })
    const window = await openPlanWindow(page, syncAndWaitForBranches)
    await window.getByTestId("range-review-commit").filter({ hasText: "scale the helper" }).click()
    await expect.poll(() => reviewedPaths(window)).toEqual(["src/Store.kt"])
    // the plan of that commit alone: the change of `helper` under its heading, its uses left outside on the card
    const store = window.locator('[data-testid="diff-view-file"][data-section="file:src/Store.kt"]')
    await expect(store.getByTestId("diff-unit-heading")).toHaveText("helper — form changed: parameters")
    const card = window.getByTestId("review-plan-card")
    await expect(card.getByTestId("review-plan-breakage-entry")).toHaveCount(1)
    await expect(card.getByTestId("review-plan-outside")).toHaveText("3 uses left outside")
    await expect(card.getByTestId("review-plan-summary")).toHaveText("1 possible break")
    // All Changes again: the plan of the branch, with the declarations the other commit adds
    await window.getByTestId("range-review-commit").filter({ hasText: "All Changes" }).click()
    await expect(card.getByTestId("review-plan-summary")).toHaveText("1 possible break · 2 new declarations")
  })

  test("should show the plan of a commit in the review window of a commit: its card and the story of its diffs", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("symbol_navigation")
    await setupPlan(repo, { experimental: true })
    // a commit of no branch: D opens the review window of the commit
    await repo.commit("scale the helper again", { "src/Store.kt": storeText("fun helper(value: Int, scale: Int, offset: Int) = value * scale + offset") }, 1704124800)
    await syncAndWaitForBranches()
    await openCommitPreview(page, getUnassignedCommitRow(page, "scale the helper again"))
    await page.keyboard.press("d")
    await expect.poll(async () => (await getOpenedSubWindows(page)).map(opened => opened.windowId)).toEqual(["commit-review"])
    const window = await openLinkedSubWindow(page, (await getOpenedSubWindows(page))[0]!)
    const store = window.locator('[data-testid="diff-view-file"][data-section="file:src/Store.kt"]')
    await expect(store.getByTestId("diff-unit-heading")).toHaveText("helper — form changed: parameters")
    await expect(window.getByTestId("review-plan-card").getByTestId("review-plan-outside")).toHaveText("3 uses left outside")
  })

  test("should show only the commits added or rewritten since Mark Reviewed, as one range on top or apart", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("symbol_navigation")
    await setupPlan(repo, { experimental: true })
    const window = await openPlanWindow(page, syncAndWaitForBranches)
    // never marked: no Since Your Review
    await expect.poll(() => commitColumn(window)).toEqual(["All Changes2 commits", "add the cache", "scale the helper"])

    await markReviewed(window)

    // a commit on top, then a sync: Since Your Review is its range, the files of that commit alone
    await repo.commit("(plan) read the cache twice", { "src/a/Reader.kt": READER('cache.get("x") + cache.get("y")') }, 1704124800)
    await syncAndWaitForBranches()
    await expect.poll(() => commitColumn(window)).toEqual(["All Changes3 commits", "Since Your Review1 commit", "read the cache twiceNew", "add the cache", "scale the helper"])
    await window.getByTestId("range-review-commit").filter({ hasText: "Since Your Review" }).click()
    await expect.poll(() => reviewedPaths(window)).toEqual(["src/a/Reader.kt"])
    await expect(window.getByTestId("review-plan-since")).toHaveText("Since your review: 1 new")
    await expect(window.getByTestId("branch-review-position")).toHaveText(/^Since Your Review · File 1 \/ 1$/)

    // the commit of nav joins plan below the reviewed ones: the new commits are not one range of the branch, so Since
    // Your Review is the diff from the reviewed change, both new commits together
    await repo.reword(await repo.revision("HEAD~3"), "(plan) use the store and the queue")
    await syncAndWaitForBranches()
    await expect(findBranchRow(page, "nav")).toHaveCount(0)
    await expect.poll(() => commitColumn(window)).toEqual(["All Changes4 commits", "Since Your Review2 commits", "read the cache twiceNew", "use the store and the queueNew"])
    await expect.poll(() => reviewedPaths(window)).toEqual(["src/a/Reader.kt", "src/Queue.kt", "src/Store.kt", "src/Uses.kt"])
    await expect(window.getByTestId("review-plan-since")).toHaveText("Since your review: 2 new")
    await window.getByTestId("range-review-commit").filter({ hasText: "use the store and the queue" }).click()
    await expect.poll(() => reviewedPaths(window)).toEqual(["src/Queue.kt", "src/Store.kt", "src/Uses.kt"])

    // All Changes lists every commit again; marked again, nothing is new: no Since Your Review
    await window.getByTestId("range-review-commit").filter({ hasText: "All Changes" }).click()
    await expect.poll(() => commitColumn(window)).toHaveLength(6)
    await runAndWaitForAnswer(window, "mark_branch_reviewed", () => window.getByTestId("branch-review-mark-reviewed").click())
    await expect.poll(() => commitColumn(window)).toEqual(["All Changes4 commits", "read the cache twice", "add the cache", "scale the helper", "use the store and the queue"])
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
    await expect.poll(() => commitColumn(window)).toEqual(["All Changes2 commits", "Since Your Review1 commit", "add the cache", "scale the helperRewritten"])
    // the card of All Changes says what the rewrite changed against the reviewed version, per path
    const since = window.getByTestId("review-plan-since")
    await expect(since).toContainText("Since your review: 1 rewritten")
    await expect(since).toContainText("Rewritten scale the helper: src/Store.kt +1 −1")

    // Since Your Review: one diff from the reviewed version, the amend alone: the reviewed line against the new one,
    // never the line before the branch, under the card with that difference
    await window.getByTestId("range-review-commit").filter({ hasText: "Since Your Review" }).click()
    await expect(window.getByTestId("branch-review-position")).toHaveText(/^Since Your Review · File 1 \/ 1$/)
    await expect.poll(() => commitColumn(window)).toEqual(["All Changes2 commits", "Since Your Review1 commit", "scale the helperRewritten"])
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
    // and nothing of the plan: no Mark Reviewed, no card, no badges of the files, no Since Your Review, no read of a plan
    await expect(page.getByRole("tree", { name: "Changed files" }).getByRole("treeitem", { name: "Store.kt" })).toBeVisible()
    await expect(page.getByTestId("branch-review-mark-reviewed")).toHaveCount(0)
    await expect(page.getByTestId("review-plan-card")).toHaveCount(0)
    await expect(page.getByTestId("file-badges")).toHaveCount(0)
    await expect.poll(() => commitColumn(page)).toEqual(["All Changes2 commits", "add the cache", "scale the helper"])
    expect(plans).toBe(0)
  })
})
