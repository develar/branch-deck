// @spec spec/branch/review-tour.spec.md
import type { Locator, Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { captureHtmlSnapshot } from "../helpers/aria-snapshot-helpers"
import { mapPointOf } from "../helpers/file-map-helpers"
import { openPlanWindow, reviewSidebar, reviewedPaths, turnOnReviewPlan } from "../helpers/review-plan-helpers"

/** The card right below the divider of `heading`: the space between them and the file of the card */
function cardBelow(heading: Locator): Promise<{ gap: number; path: string | null }> {
  return heading.evaluate(element => {
    const divider = element.closest("[data-testid=diff-view-divider]")!
    const card = divider.parentElement!.querySelector(`[data-testid=diff-view-file][data-section="${divider.getAttribute("data-section")}"]`)!
    return { gap: Math.round(card.getBoundingClientRect().top - divider.getBoundingClientRect().bottom), path: card.getAttribute("data-path") }
  })
}

/** The top of the divider of `heading` below the top of the diffs */
function topOf(heading: Locator): Promise<number> {
  return heading.evaluate(element => {
    const view = element.closest("[data-testid=diff-view]")!
    return Math.round(element.closest("[data-testid=diff-view-divider]")!.getBoundingClientRect().top - view.getBoundingClientRect().top)
  })
}

/** The distinct left edges of the names in the headers of the diffs */
function headerNameLefts(window: Page): Promise<number[]> {
  return window.getByTestId("review-file-header").evaluateAll(headers => {
    const lefts = headers.map(header => {
      const path = header.getAttribute("data-path")!
      const name = [...header.querySelectorAll("span")].find(span => span.childElementCount === 0 && span.textContent === path.slice(path.lastIndexOf("/") + 1))!
      return Math.round(name.getBoundingClientRect().left)
    })
    return [...new Set(lefts)]
  })
}

// the `review_tour` template: the branch `layout` of five commits with bodies, a refactor of the manager, the rule
// `allowed` with its two uses and a test that names it, a change of the paint no test names, the restart story with a
// rule of the spec and its test link, and a round that renames the flag of the rule

test.describe("Review tour", () => {
  test("should open the review on its brief, a step per commit with its why, its rounds and the tests that name its code", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    turnOnReviewPlan(await setupRepo("review_tour"))
    const window = await openPlanWindow(page, syncAndWaitForBranches, "layout")
    const brief = window.getByTestId("review-brief")

    // what changes, in the words of the spec, proven by the test its link names
    const rule = brief.getByTestId("review-brief-contract-entry")
    await expect(rule).toContainText("Spec rule")
    await expect(rule).toContainText("The window keeps its stored layout across a restart, and the project view does not take the place of another tool window.")
    await expect(rule).toContainText("Proven by restart.json")

    // the steps the commits tell, the round of the rule in its step; the why of each its commit's first paragraph
    await expect(brief.getByTestId("review-brief-step-title")).toHaveText([
      "The manager drops the hooks nothing sets",
      "The open keeps a stored layout",
      "The view paints with the first frame",
      "The restart story checks the layout",
    ])
    const step = (number: number) => brief.getByTestId("review-brief-step").nth(number - 1)
    await expect(step(2)).toContainText("Two startup steps showed the project view at each open")
    // its uses changed bodies no test names, beside the rule a test names
    await expect(step(2).getByTestId("review-brief-step-fact")).toHaveText([
      "Follows up: The rule names its flag again",
      "Tests that name it: allowed",
      "Not named by a test: show, focus",
    ])
    // the decision of the author is the rest of the body, folded
    await step(2).getByRole("button", { name: "More" }).click()
    await expect(step(2)).toContainText("The product decision is to keep the layout of the user.")
    // the paint no test of the change names; the story proven by its scenario
    await expect(step(3).getByTestId("review-brief-step-unproven")).toHaveText("No test in this change names paint")
    await expect(step(4).getByTestId("review-brief-step-fact")).toHaveText(["Proven by restart.json"])
    await captureHtmlSnapshot(brief, "review-tour-brief")

    // the tree lists the steps, each with its commits, then its files, a file of two steps with its row in the step of its
    // most lines and a row "also in" the other; the diffs tell the story by them
    const tree = window.getByRole("tree", { name: "Changed files" })
    const rows = () => tree.getByRole("treeitem").evaluateAll(items => items.map(item => item.getAttribute("aria-label")))
    await expect
      .poll(rows)
      .toEqual([
        "1 · The manager drops the hooks nothing sets",
        "the manager drops the hooks nothing sets",
        "src/a/Manager.kt, also in 1 · The manager drops the hooks nothing sets",
        "2 · The open keeps a stored layout",
        "the open keeps a stored layout",
        "the rule names its flag again",
        "src/a/Manager.kt",
        "src/a/Frame.kt",
        "src/b/Welcome.kt",
        "test/a/ManagerTest.kt",
        "3 · The view paints with the first frame",
        "the view paints with the first frame",
        "src/a/Frame.kt, also in 3 · The view paints with the first frame",
        "4 · The restart story checks the layout",
        "the restart story checks the layout",
        "spec/window.spec.md",
        "tests/restart.json",
      ])
    // each step opens with a divider on the canvas right above the card of its first section; a header names its file
    // alone, so the name of every file stands at one x (every card of the story on screen: the view renders the cards it
    // shows)
    const viewport = window.viewportSize()!
    await window.setViewportSize({ width: 1280, height: 1200 })
    const dividers = window.getByTestId("review-concern-heading")
    await expect(dividers).toHaveText([
      "Step 1 · The manager drops the hooks nothing sets",
      "Step 2 · The open keeps a stored layout",
      "Step 3 · The view paints with the first frame",
      "Step 4 · The restart story checks the layout",
    ])
    await expect(window.getByTestId("review-file-header").getByTestId("review-concern-heading")).toHaveCount(0)
    // the map of the files beside the diffs crosses itself at the start of each step; its hover names the file and its step
    const map = window.getByTestId("review-file-map")
    await expect(map).toHaveAttribute("data-steps", "4")
    // the second section of Manager.kt, the card below the divider of step 2
    const start = await mapPointOf(window.locator("[data-testid=diff-view-file][data-path='src/a/Manager.kt']").nth(1))
    await window.mouse.move(start.x, start.y)
    await expect(map.getByTestId("review-file-map-tip")).toHaveText(/Manager\.ktsrc\/a.*Step 2 · The open keeps a stored layout/)
    expect(await cardBelow(dividers.nth(1))).toEqual({ gap: 0, path: "src/a/Manager.kt" })
    expect(await cardBelow(dividers.nth(3))).toEqual({ gap: 0, path: "spec/window.spec.md" })
    await expect.poll(() => headerNameLefts(window)).toEqual([expect.any(Number)])
    // the rule first in its step, its uses right after it, its test with them
    await expect
      .poll(async () => (await reviewedPaths(window)).slice(0, 7))
      .toEqual(["src/a/Manager.kt", "src/a/Manager.kt", "src/a/Frame.kt", "src/b/Welcome.kt", "test/a/ManagerTest.kt", "src/a/Frame.kt", "spec/window.spec.md"])
    await window.setViewportSize(viewport)

    // a step of the brief opens its first section; the bar above the diffs names the step at the top as they scroll
    const bar = window.getByTestId("review-step-bar")
    await expect(bar.getByTestId("review-step-position")).toHaveText("Step 1 / 4")
    await step(3).getByTestId("review-brief-step-title").click()
    await expect(bar.getByTestId("review-step-position")).toHaveText("Step 3 / 4")
    await expect(bar.getByTestId("review-step-title")).toHaveText("The view paints with the first frame")
    await expect(bar).toContainText("No test in this change names paint")
    await expect(tree.getByRole("treeitem", { name: "src/a/Frame.kt, also in 3 · The view paints with the first frame", exact: true })).toHaveAttribute("aria-selected", "true")
    // J goes on into the next step, the bar follows
    await window.keyboard.press("j")
    await expect(bar.getByTestId("review-step-position")).toHaveText("Step 4 / 4")
    // a click on the divider of a step shows the step as its bar does: the divider at the top, the bar and the tree follow
    const second = window.getByRole("button", { name: "Step 2 · The open keeps a stored layout", exact: true })
    await second.click()
    await expect(bar.getByTestId("review-step-position")).toHaveText("Step 2 / 4")
    await expect(tree.getByRole("treeitem", { name: "src/a/Manager.kt", exact: true })).toHaveAttribute("aria-selected", "true")
    await expect.poll(() => topOf(second)).toBe(10)
  })

  test("should list the commits in their steps as one list, show the step of a commit on a click, and the commit alone on Enter, Show Commit Alone and Option+Down", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    turnOnReviewPlan(await setupRepo("review_tour"))
    const window = await openPlanWindow(page, syncAndWaitForBranches, "layout")
    const { commitRow, column } = reviewSidebar(window)
    const header = window.getByTestId("range-review-commit-header")
    const bar = window.getByTestId("review-step-bar")

    // the column keeps the scopes; the commits are in the steps of the tree, the round of a step says so
    await expect(commitRow("the rule names its flag again")).toBeVisible()
    await expect(column).toHaveText([/^All Changes5 commits$/])
    await expect(commitRow("the rule names its flag again").getByTestId("review-commit-role")).toHaveText("follows up")
    await expect(commitRow("the open keeps a stored layout").getByTestId("review-commit-role")).toHaveCount(0)
    await expect(commitRow("the open keeps a stored layout")).toHaveAttribute("aria-description", /^commit [0-9a-f]{7,}$/)

    // a click selects the commit and puts the first section of its step at the top, the commit stays in the story
    await expect(bar.getByTestId("review-step-position")).toHaveText("Step 1 / 4")
    await commitRow("the view paints with the first frame").click()
    await expect(commitRow("the view paints with the first frame")).toHaveAttribute("aria-selected", "true")
    await expect(bar.getByTestId("review-step-position")).toHaveText("Step 3 / 4")
    await expect(header).toHaveCount(0)

    // Enter shows the commit alone: its header, the column lists every commit again; All Changes goes back to the story
    await commitRow("the view paints with the first frame").press("Enter")
    await expect(header).toContainText("the view paints with the first frame")
    await expect(column).toHaveCount(6)
    await expect(commitRow("the view paints with the first frame")).toHaveCount(0)
    await column.filter({ hasText: "All Changes" }).click()
    await expect(header).toHaveCount(0)
    await expect(column).toHaveCount(1)

    // Show Commit Alone of its right-click menu does the same
    await commitRow("the manager drops the hooks nothing sets").click({ button: "right" })
    await window.getByRole("menuitem", { name: "Show Commit Alone" }).click()
    await expect(header).toContainText("the manager drops the hooks nothing sets")
    await column.filter({ hasText: "All Changes" }).click()
    await expect(header).toHaveCount(0)

    // Option+Down steps from All Changes to the newest commit alone, as the column did
    await window.getByTestId("commit-review-code").locator("[tabindex='0']").first().focus()
    await window.keyboard.press("Alt+ArrowDown")
    await expect(header).toContainText("the rule names its flag again")
  })

  test("should mark the steps and the files of an owner chip in the whole story, and move N and J through its sections only", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("review_tour")
    turnOnReviewPlan(repo)
    // Design owns src/b and src/a/Frame.kt: the files of steps 2 and 3, none of steps 1 and 4
    await repo.commitOnRemote("master", {
      subject: "Add the owners",
      author: "Jane <jane@example.com>",
      timestamp: 1704110400,
      files: { OWNERSHIP: "owner 'Monorepo'\n", "src/b/OWNERSHIP": "owner 'Design'\n", "src/a/OWNERSHIP": "/Frame.kt\nowner 'Design'\n" },
    })
    await repo.fetchInTerminal()
    const window = await openPlanWindow(page, syncAndWaitForBranches, "layout")
    const tree = window.getByRole("tree", { name: "Changed files" })
    const row = (name: string) => tree.getByRole("treeitem", { name, exact: true })
    const steps = [
      "1 · The manager drops the hooks nothing sets",
      "2 · The open keeps a stored layout",
      "3 · The view paints with the first frame",
      "4 · The restart story checks the layout",
    ]
    const frameAlso = row("src/a/Frame.kt, also in 3 · The view paints with the first frame")
    // the steps, their files and their five commits
    await expect(tree.getByRole("treeitem")).toHaveCount(17)

    await window.getByTestId("branch-review-owner").filter({ hasText: "Design" }).click()
    await expect(window.getByTestId("branch-review-owner").filter({ hasText: "Design" })).toHaveAttribute("aria-pressed", "true")
    // a chip hides nothing: every step and file stays, the steps with its files count them, the others are muted, their
    // commits with them
    await expect(tree.getByRole("treeitem")).toHaveCount(17)
    await expect(row(steps[1]!).getByTestId("review-owner-mark")).toHaveText("Design2")
    await expect(row(steps[1]!)).toHaveAttribute("aria-description", /, 2 files of Design$/)
    await expect(row(steps[2]!).getByTestId("review-owner-mark")).toHaveText("Design1")
    await expect(row(steps[1]!)).not.toHaveAttribute("data-muted")
    for (const muted of [steps[0]!, steps[3]!, "src/a/Manager.kt", "test/a/ManagerTest.kt", "the manager drops the hooks nothing sets"]) {
      await expect(row(muted)).toHaveAttribute("data-muted", "")
    }
    await expect(row("the open keeps a stored layout")).not.toHaveAttribute("data-muted")
    await expect(row(steps[0]!).getByTestId("review-owner-mark")).toHaveCount(0)
    await expect(row("src/b/Welcome.kt")).not.toHaveAttribute("data-muted")
    await expect(window.locator('[data-testid="review-file-header"][data-path="src/a/Manager.kt"]').first()).toHaveAttribute("data-muted", "")
    await expect(window.locator('[data-testid="review-file-header"][data-path="src/a/Frame.kt"]').first()).not.toHaveAttribute("data-muted")

    // N from the first section passes the blocks of Manager.kt to the first of Frame.kt
    await window.getByTestId("commit-review-code").locator("[tabindex='0']").first().focus()
    await window.keyboard.press("n")
    await expect(row("src/a/Frame.kt")).toHaveAttribute("aria-selected", "true")
    // J goes on through the sections of Design: Welcome.kt, then past ManagerTest.kt to the part of Frame.kt in step 3
    await window.keyboard.press("j")
    await expect(row("src/b/Welcome.kt")).toHaveAttribute("aria-selected", "true")
    await window.keyboard.press("j")
    await expect(frameAlso).toHaveAttribute("aria-selected", "true")
    await expect(window.getByTestId("review-step-bar").getByTestId("review-step-position")).toHaveText("Step 3 / 4")
    // none of Design is left: J moves nothing, so with All it goes on from Frame.kt to the first section of step 4
    await window.keyboard.press("j")
    await window.getByTestId("branch-review-owner").filter({ hasText: "All" }).click()
    await expect(row(steps[0]!)).not.toHaveAttribute("data-muted")
    await window.getByTestId("commit-review-code").locator("[tabindex='0']").first().focus()
    await window.keyboard.press("j")
    await expect(row("spec/window.spec.md")).toHaveAttribute("aria-selected", "true")
  })
})
