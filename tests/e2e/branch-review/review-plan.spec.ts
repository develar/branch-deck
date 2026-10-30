// @spec spec/branch/review-plan.spec.md
import { appendFileSync, writeFileSync } from "node:fs"
import path from "node:path"
import type { Page } from "@playwright/test"
import type { BranchReviewWindowData } from "../../../src/types/sub-window"
import { test, expect } from "../fixtures/test-fixtures"
import { captureHtmlSnapshot } from "../helpers/aria-snapshot-helpers"
import { findBranchRow } from "../helpers/branch-helpers"
import { clickContextMenuItem, openContextMenu } from "../helpers/selection-helpers"
import { getOpenedSubWindows, openLinkedSubWindow, openSubWindowPage } from "../helpers/sub-window-helpers"
import { runAndWaitForAnswer } from "../helpers/sync-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"

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

/** The branch `plan` of two commits on top of the template, the index of accio with three usages of `helper` */
async function setupPlan(repo: TestRepositoryBuilder, { experimental }: { experimental: boolean }): Promise<void> {
  // the flag is on when unset, but the global config of the test machine turns it off (`TEST_IDENTITY` of the server)
  if (experimental) {
    // `git config branchdeck.reviewExperimental true` in a terminal: the next sync reads it, and it wins over the global
    appendFileSync(path.join(repo.path, ".git", "config"), "[branchdeck]\n\treviewExperimental = true\n")
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

/** The paths of the diffs the review shows, in their order */
async function reviewedPaths(page: Page): Promise<string[]> {
  return page.getByTestId("review-file-header").evaluateAll(headers => headers.map(header => header.getAttribute("data-path")!))
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

/** The review window of `plan` after a sync, as a page of its own beside the main window, its diffs shown */
async function openPlanWindow(page: Page, syncAndWaitForBranches: () => Promise<unknown>): Promise<Page> {
  await syncAndWaitForBranches()
  const window = await openLinkedSubWindow(page, await openReviewWindow(page, "plan"))
  await expect(window.getByTestId("review-file-header").first()).toBeVisible()
  return window
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
  test("should show the API & Risk card with the fan-in, and keep the files in the tree", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupPlan(await setupRepo("symbol_navigation"), { experimental: true })
    await syncAndWaitForBranches()
    await openSubWindowPage(page, await openReviewWindow(page, "plan"))
    await expect(page.getByTestId("review-file-header").first()).toBeVisible()

    // the card above the diffs of All Changes: the changed `helper` first, with its usages at the head from accio
    const card = page.getByTestId("review-plan-card")
    await expect(card.getByTestId("review-plan-summary")).toHaveText(/^\d+ API changes · 3 risky files$/)
    const helper = card.getByTestId("review-plan-api-entry").first()
    await expect(helper).toHaveAttribute("data-name", "helper")
    await expect(helper).toContainText("Changed")
    await expect(helper).toContainText("src/Store.kt:")
    await expect(helper.getByTestId("review-plan-fan-in")).toHaveText("3 usages")
    // the risky files: the used API without its test first
    await expect(card.getByTestId("review-plan-risk-entry").first()).toHaveAttribute("data-path", "src/Store.kt")
    await expect(card.getByTestId("review-plan-risk-entry").first()).toContainText("An API used in 3 places · No test changed")
    await captureHtmlSnapshot(card, "review-plan-card")

    // the files stay in the tree: the directories first, no order to choose
    await expect.poll(() => reviewedPaths(page)).toEqual(["src/a/Reader.kt", "src/b/Cache.kt", "src/Store.kt"])
    await expect(page.getByRole("group", { name: "Order" })).toHaveCount(0)
  })

  test("should badge each file with its links, the uses it breaks and no test, and select a linked file from its popover", async ({ page, setupRepo, syncAndWaitForBranches }) => {
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
  })

  test("should select a linked file that a filter hides, and drop only the filter that hides it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("symbol_navigation")
    await setupPlan(repo, { experimental: true })
    // five notes more, so the window has the search of the files; the owners of the baseline, src/b its own
    const notes = Object.fromEntries([1, 2, 3, 4, 5].map(number => [`docs/note${number}.txt`, `note ${number}\n`]))
    await repo.commit("(plan) add the notes", notes, 1704124800)
    await repo.commitOnRemote("master", {
      subject: "Add the owners",
      author: "Jane <jane@example.com>",
      timestamp: 1704110400,
      files: { OWNERSHIP: "owner 'Monorepo'\n", "src/b/OWNERSHIP": "owner 'Cache Team'\n" },
    })
    await repo.fetchInTerminal()
    const window = await openPlanWindow(page, syncAndWaitForBranches)
    const { reader, cache } = planRows(window)
    const owner = (name: string) => window.getByTestId("branch-review-owner").filter({ hasText: name })
    const search = window.getByRole("textbox", { name: "Filter files" })
    const selectCacheFromReader = async () => {
      await reader.getByTestId("file-links-badge").click()
      await window.getByTestId("file-links-badge-popover").getByTestId("file-link").click()
    }

    // the owner filter hides Cache: a click on its link shows all files and selects it
    await owner("Monorepo").click()
    await expect(cache).toHaveCount(0)
    await selectCacheFromReader()
    await expect(owner("All")).toHaveAttribute("aria-pressed", "true")
    await expect(cache).toBeVisible()
    await expect(cache).toHaveAttribute("aria-selected", "true")
    await expect(window.getByTestId("branch-review-position")).toHaveText(/File 7 \/ 8$/)

    // a search that shows Cache stays
    await search.fill(".kt")
    await expect.poll(() => reviewedPaths(window)).toEqual(["src/a/Reader.kt", "src/b/Cache.kt", "src/Store.kt"])
    // the name of the row: its middle is a badge
    await reader.getByText("Reader.kt", { exact: true }).click()
    await expect(reader).toHaveAttribute("aria-selected", "true")
    await selectCacheFromReader()
    await expect(cache).toHaveAttribute("aria-selected", "true")
    await expect(search).toHaveValue(".kt")

    // a search that hides Cache is cleared, and Cache selected
    await search.fill("Reader")
    await expect(cache).toHaveCount(0)
    await selectCacheFromReader()
    await expect(search).toHaveValue("")
    await expect(cache).toBeVisible()
    await expect(cache).toHaveAttribute("aria-selected", "true")
    await expect(window.getByTestId("branch-review-position")).toHaveText(/File 7 \/ 8$/)
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
    const window = await openPlanWindow(page, syncAndWaitForBranches)
    const tree = window.getByRole("tree", { name: "Changed files" })

    // the real work in the tree, the build file and the reindented file last, in a group that starts collapsed
    await expect.poll(() => reviewedPaths(window)).toEqual(["src/a/Reader.kt", "src/b/Cache.kt", "src/Store.kt", "src/b/b.iml", "src/Base.kt"])
    const group = tree.getByRole("treeitem", { name: "Mechanical (2)" })
    await expect(group).toHaveAttribute("aria-expanded", "false")
    await expect(tree.getByRole("treeitem", { name: "src/Base.kt" })).toHaveCount(0)
    // the new module shows on the directory of its module, its build file folded
    const module = tree.getByRole("treeitem", { name: "b", exact: true })
    await expect(module.getByTestId("directory-dependency-badge")).toHaveText("new module")
    await module.getByTestId("directory-dependency-badge").hover()
    await expect(window.getByTestId("directory-dependency-changes")).toHaveText("New module b")
    await captureHtmlSnapshot(tree, "review-plan-tree-mechanical")

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

    // the commit of nav joins plan below the reviewed ones: the new commits are not one range, the column lists only them
    await repo.reword(await repo.revision("HEAD~3"), "(plan) use the store and the queue")
    await syncAndWaitForBranches()
    await expect(findBranchRow(page, "nav")).toHaveCount(0)
    await expect.poll(() => commitColumn(window)).toEqual(["All Changes4 commits", "Since Your Review2 commits", "read the cache twiceNew", "use the store and the queueNew"])
    await expect.poll(() => reviewedPaths(window)).toEqual(["src/a/Reader.kt"])
    await window.getByTestId("range-review-commit").filter({ hasText: "use the store and the queue" }).click()
    await expect.poll(() => reviewedPaths(window)).toEqual(["src/Queue.kt", "src/Store.kt", "src/Uses.kt"])

    // All Changes lists every commit again; marked again, nothing is new: no Since Your Review
    await window.getByTestId("range-review-commit").filter({ hasText: "All Changes" }).click()
    await expect.poll(() => commitColumn(window)).toHaveLength(6)
    await runAndWaitForAnswer(window, "mark_branch_reviewed", () => window.getByTestId("branch-review-mark-reviewed").click())
    await expect.poll(() => commitColumn(window)).toEqual(["All Changes4 commits", "read the cache twice", "add the cache", "scale the helper", "use the store and the queue"])
  })

  test("should show a reviewed commit amended since as Rewritten, with what it changes against its reviewed version", async ({ page, setupRepo, syncAndWaitForBranches }) => {
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

    // Since Your Review: no range, the rewritten commit alone, and above its heading the card with that difference
    await window.getByTestId("range-review-commit").filter({ hasText: "Since Your Review" }).click()
    await expect.poll(() => commitColumn(window)).toEqual(["All Changes2 commits", "Since Your Review1 commit", "scale the helperRewritten"])
    await expect(window.getByTestId("range-review-commit-header")).toContainText("scale the helper")
    await expect(window.getByTestId("branch-review-position")).not.toHaveText(/^Since Your Review/)
    await expect.poll(() => reviewedPaths(window)).toEqual(["src/Store.kt"])
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
