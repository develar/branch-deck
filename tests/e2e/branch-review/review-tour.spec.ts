import { writeFileSync } from "node:fs"
import path from "node:path"
// @spec spec/branch/review-tour.spec.md
import type { Locator, Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { captureHtmlSnapshot } from "../helpers/aria-snapshot-helpers"
import { mapPointOf } from "../helpers/file-map-helpers"
import { openPlanWindow, reviewSidebar, reviewedPaths, turnOnReviewPlan } from "../helpers/review-plan-helpers"
import { selectedInDiffs } from "../helpers/selection-helpers"
import { runAndWaitForAnswer } from "../helpers/sync-helpers"

/** The card right below the opening of the step of `heading`: the space between them and the file of the card */
function cardBelow(heading: Locator): Promise<{ gap: number; path: string | null }> {
  return heading.evaluate(element => {
    const opening = element.closest("[data-testid=diff-view-opening]")!
    const card = opening.closest("[data-testid=diff-view]")!.querySelector(`[data-testid=diff-view-file][data-section="${opening.getAttribute("data-section")}"]`)!
    return { gap: Math.round(card.getBoundingClientRect().top - opening.getBoundingClientRect().bottom), path: card.getAttribute("data-path") }
  })
}

/** The top of the opening of the step of `heading` below the top of the diffs */
function topOf(heading: Locator): Promise<number> {
  return heading.evaluate(element => {
    const view = element.closest("[data-testid=diff-view]")!
    return Math.round(element.closest("[data-testid=diff-view-opening]")!.getBoundingClientRect().top - view.getBoundingClientRect().top)
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
// `allowed` with its two uses and a test that calls it, a test of the welcome the branch does not touch, a change of the
// paint no test calls, the restart story with a rule of the spec and its test link, and a round that renames the flag of
// the rule

/** The index of the repository, as accio answers `tests --at` the code of the steps of `layout` at its tip */
function writeTestIndex(repositoryPath: string): void {
  const test = (file: string, line: number, readsAs: string) => ({ file, line, declaration: `Test.${readsAs}`, reads_as: readsAs, framework: "junit4", depth: 1 })
  const tests = {
    // `allowed`, by the test the branch adds
    "src/a/Manager.kt:6": { tests: [test("test/a/ManagerTest.kt", 8, "the open may show only the stored layout")] },
    // `focus`, by the test of the welcome the branch does not touch
    "src/b/Welcome.kt:6": { tests: [test("test/b/WelcomeTest.kt", 7, "the welcome focuses its project")] },
    // `show`: no caller is a test
    "src/a/Frame.kt:4": { tests: [] },
    // `paint`: none within the depth, with callers left
    "src/a/Frame.kt:6": { tests: [], cut: "depth" },
  }
  writeFileSync(path.join(repositoryPath, ".git", "branch-deck-accio.json"), JSON.stringify({ tests }))
}

test.describe("Review tour", () => {
  test("should open the review on its overview, then each step's opening above its first card, a step per commit with its story, its rounds and the tests that reach its code", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("review_tour")
    turnOnReviewPlan(repo)
    writeTestIndex(repo.path)
    const window = await openPlanWindow(page, syncAndWaitForBranches, "layout")
    const overview = window.getByTestId("review-overview")
    const bar = window.getByTestId("review-step-bar")

    // the document opens on its overview: the branch, what changes in the words of the spec, proven by the test its
    // link names
    await expect(bar).toHaveText("Overview·4 steps")
    await expect(overview.getByTestId("review-overview-headline")).toHaveText("user-name/virtual/layout")
    const rule = overview.getByTestId("review-brief-contract-entry")
    await expect(rule).toContainText("Spec rule")
    await expect(rule).toContainText("The window keeps its stored layout across a restart, and the project view does not take the place of another tool window.")
    await expect(rule).toContainText("Proven by restart.json")

    // the contents: the steps the commits tell, each its full title, the lead of its commit's body and its facts
    await expect(overview.getByTestId("review-contents-step-title")).toHaveText([
      "The manager drops the hooks nothing sets",
      "The open keeps a stored layout",
      "The view paints with the first frame",
      "The restart story checks the layout",
    ])
    const step = (number: number) => overview.getByTestId("review-contents-step").nth(number - 1)
    await expect(step(2).getByTestId("review-contents-step-lead")).toContainText("Two startup steps showed the project view at each open")
    // the rule a test of the change reaches, a use a test the change did not touch reaches, a use no test reaches
    await expect(step(2).getByTestId("review-contents-step-facts")).toHaveText(
      "Tests of this change reach allowed · 1 test it did not touch reaches focus · No test reaches show · New API: allowed",
    )
    // the paint no test reaches within the depth accio walked; the story proven by its scenario
    await expect(step(3).getByTestId("review-contents-step-unproven")).toHaveText("No test reaches paint within 3 calls")
    await expect(step(4).getByTestId("review-contents-step-facts")).toHaveText("Proven by restart.json")
    await captureHtmlSnapshot(overview, "review-overview")

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
    // each step opens on the canvas right above the card of its first section, "Step 2" above its full title; a header
    // names its file alone, so the name of every file stands at one x (every card of the story on screen: the view
    // renders the cards it shows)
    const viewport = window.viewportSize()!
    await window.setViewportSize({ width: 1280, height: 2400 })
    const dividers = window.getByTestId("review-concern-heading")
    await expect(dividers).toHaveText([
      "Step 1 · The manager drops the hooks nothing sets",
      "Step 2 · The open keeps a stored layout",
      "Step 3 · The view paints with the first frame",
      "Step 4 · The restart story checks the layout",
    ])
    await expect(window.getByTestId("review-file-header").getByTestId("review-concern-heading")).toHaveCount(0)
    // the opening tells the story of the commit that starts the step: its lead, the rest of its body, its round as a
    // disclosure of its own body, the facts of the step
    const opening = window.getByTestId("review-opening").filter({ has: window.getByRole("button", { name: "Step 2 · The open keeps a stored layout", exact: true }) })
    await expect(opening.getByTestId("review-story-lead").first()).toContainText("Two startup steps showed the project view at each open")
    await expect(opening).toContainText("The product decision is to keep the layout of the user.")
    await expect(opening.getByTestId("review-opening-facts")).toHaveText(
      "Tests of this change reach allowed · 1 test it did not touch reaches focus · No test reaches show · New API: allowed",
    )
    const round = opening.getByTestId("review-opening-follow-up")
    await expect(round).toContainText("Follows up:The rule names its flag again")
    await expect(round).not.toContainText("The review asked for the name of the flag")
    await round.getByRole("button", { name: /The rule names its flag again/ }).click()
    await expect(round).toContainText("The review asked for the name of the flag to say what it holds.")
    await captureHtmlSnapshot(opening, "review-opening")
    await expect(window.getByTestId("review-opening").nth(2).getByTestId("review-opening-unproven")).toHaveText("No test reaches paint within 3 calls")
    // the map of the files beside the diffs crosses itself at the start of each step; its hover names the file and its step
    const map = window.getByTestId("review-file-map")
    await expect(map).toHaveAttribute("data-steps", "4")
    // the second section of Manager.kt, the card below the opening of step 2
    const start = await mapPointOf(window.locator("[data-testid=diff-view-file][data-path='src/a/Manager.kt']").nth(1))
    await window.mouse.move(start.x, start.y)
    await expect(map.getByTestId("review-file-map-tip")).toHaveText(/Manager\.ktsrc\/a.*Step 2 · The open keeps a stored layout/)
    // an opening is measured in place before the frame paints: the layout has its height by the next frame
    await expect.poll(() => cardBelow(dividers.nth(1))).toEqual({ gap: 0, path: "src/a/Manager.kt" })
    await expect.poll(() => cardBelow(dividers.nth(3))).toEqual({ gap: 0, path: "spec/window.spec.md" })
    await expect.poll(() => headerNameLefts(window)).toEqual([expect.any(Number)])
    // the rule first in its step, its uses right after it, its test with them
    await expect
      .poll(async () => (await reviewedPaths(window)).slice(0, 7))
      .toEqual(["src/a/Manager.kt", "src/a/Manager.kt", "src/a/Frame.kt", "src/b/Welcome.kt", "test/a/ManagerTest.kt", "src/a/Frame.kt", "spec/window.spec.md"])
    await window.setViewportSize(viewport)

    // a step of the contents shows its opening at the top: the bar names it on one line, the tree selects the step
    await step(3).getByTestId("review-contents-step-title").click()
    await expect(bar).toHaveText("Step 3 / 4·The view paints with the first frame")
    await expect(bar).toHaveAttribute("title", "The view paints with the first frame")
    await expect(tree.getByRole("treeitem", { name: "3 · The view paints with the first frame", exact: true })).toHaveAttribute("aria-selected", "true")
    // J goes from the opening to its card, the tree to its file, then to the opening of the next step
    await window.keyboard.press("j")
    await expect(tree.getByRole("treeitem", { name: "src/a/Frame.kt, also in 3 · The view paints with the first frame", exact: true })).toHaveAttribute("aria-selected", "true")
    await window.keyboard.press("j")
    await expect(bar.getByTestId("review-step-position")).toHaveText("Step 4 / 4")
    await expect(tree.getByRole("treeitem", { name: "4 · The restart story checks the layout", exact: true })).toHaveAttribute("aria-selected", "true")
    // K goes back the same way, and from the opening of the first step to the overview
    await window.keyboard.press("k")
    await expect(tree.getByRole("treeitem", { name: "src/a/Frame.kt, also in 3 · The view paints with the first frame", exact: true })).toHaveAttribute("aria-selected", "true")
    await step(1).getByTestId("review-contents-step-title").click()
    await expect(bar.getByTestId("review-step-position")).toHaveText("Step 1 / 4")
    await window.keyboard.press("k")
    await expect(bar.getByTestId("review-step-position")).toHaveText("Overview")
    await window.keyboard.press("j")
    await expect(bar.getByTestId("review-step-position")).toHaveText("Step 1 / 4")
    // a click on the title of an opening shows the step as its bar does: the opening at the top, the bar and the tree follow
    const second = window.getByRole("button", { name: "Step 2 · The open keeps a stored layout", exact: true })
    await second.click()
    await expect(bar.getByTestId("review-step-position")).toHaveText("Step 2 / 4")
    await expect(tree.getByRole("treeitem", { name: "2 · The open keeps a stored layout", exact: true })).toHaveAttribute("aria-selected", "true")
    await expect.poll(() => topOf(second)).toBe(10)
  })

  test("should link a code name of a body to its declaration in the diffs and a section back to the paragraph that names it", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    turnOnReviewPlan(await setupRepo("review_tour"))
    const window = await openPlanWindow(page, syncAndWaitForBranches, "layout")
    const tree = window.getByRole("tree", { name: "Changed files" })
    const bar = window.getByTestId("review-step-bar")
    const opening = window.getByTestId("review-opening").filter({ has: window.getByRole("button", { name: "Step 2 · The open keeps a stored layout", exact: true }) })

    // `Manager.allowed` in the body of step 2 resolves to the rule the step adds: a link, its hover names it, its file,
    // its step
    const allowed = opening.getByTestId("review-story-lead").getByTestId("review-story-mention").filter({ hasText: "Manager.allowed" })
    await expect(allowed).toHaveAttribute("data-path", "src/a/Manager.kt")
    await allowed.hover()
    await expect(window.getByTestId("review-story-mention-tip")).toHaveText("allowed — new · Manager.kt:6 · step 2")
    // a click shows the declaration: its file selected, its name selected in the diffs
    await allowed.click()
    await expect(tree.getByRole("treeitem", { name: "src/a/Manager.kt", exact: true })).toHaveAttribute("aria-selected", "true")
    await expect.poll(() => selectedInDiffs(window)).toEqual({ text: "allowed", path: "src/a/Manager.kt" })
    // the same name in the contents of the overview links too
    await expect(window.getByTestId("review-contents-step").nth(1).getByTestId("review-story-mention")).toHaveText("Manager.allowed")

    // back: the header of the section the body names marks it, its hover the line that names it, where it is told
    const manager = window.locator('[data-testid="diff-view-file"][data-path="src/a/Manager.kt"]').nth(1)
    const mark = manager.getByTestId("review-story-mark")
    await expect(mark).toHaveCount(1)
    await expect(window.locator('[data-testid="diff-view-file"][data-path="src/a/Manager.kt"]').first().getByTestId("review-story-mark")).toHaveCount(0)
    await mark.hover()
    const tip = window.getByTestId("review-story-mark-tip")
    await expect(tip).toContainText("Step 2 · The open keeps a stored layout")
    await expect(tip).toContainText("One rule decides now: Manager.allowed answers whether the open may show a tool window.")
    // a click shows the opening of its step, the paragraph outlined
    await window.mouse.move(0, 0)
    await mark.click()
    await expect(bar.getByTestId("review-step-position")).toHaveText("Step 2 / 4")
    await expect(opening.locator("[data-story-block][data-highlight]")).toContainText("One rule decides now")
    await expect(window.locator("[data-story-block][data-highlight]")).toHaveCount(1)

    // the Look closely of the author is a callout in the opening of its step and in the contents; its tests are folded
    // always, named on the line of the folded parts
    const restart = window.getByTestId("review-opening").filter({ has: window.getByRole("button", { name: "Step 4 · The restart story checks the layout", exact: true }) })
    await expect(restart.getByTestId("review-story-look-closely")).toContainText("the story restarts the window alone, never the whole IDE.")
    await expect(window.getByTestId("review-contents-step").nth(3).getByTestId("review-contents-step-look-closely")).toContainText("the story restarts the window alone")
    const tests = restart.getByTestId("review-story-fold")
    await expect(tests).toHaveText("Tests")
    await expect(tests).toHaveAttribute("aria-expanded", "false")
    await expect(restart).not.toContainText("restarts the window once")
    // a name in a folded part marks its section too: a click opens the part, its block outlined
    await tree.getByRole("treeitem", { name: "tests/restart.json", exact: true }).click()
    const restartMark = window.locator('[data-testid="diff-view-file"][data-path="tests/restart.json"]').getByTestId("review-story-mark")
    await window.mouse.move(0, 0)
    await restartMark.click()
    await expect(bar.getByTestId("review-step-position")).toHaveText("Step 4 / 4")
    await expect(tests).toHaveAttribute("aria-expanded", "true")
    await expect(restart.locator("[data-story-block][data-highlight]")).toHaveText("restart.json restarts the window once")
    // the disclosure closes the part and opens it again, below the line of the folded parts
    await tests.click()
    await expect(tests).toHaveAttribute("aria-expanded", "false")
    await expect(restart).not.toContainText("restarts the window once")
    await tests.click()
    await expect(restart.getByTestId("review-story-part").filter({ hasText: "restarts the window once" })).toBeVisible()

    // the outline plays once: the commit shown alone, then the whole story again, mount their stories with no block
    // outlined and no fold opened by the click before
    const { commitRow, column } = reviewSidebar(window)
    await commitRow("the restart story checks the layout").click()
    await commitRow("the restart story checks the layout").press("Enter")
    const header = window.getByTestId("range-review-commit-header")
    await expect(header.getByTestId("review-story-fold")).toHaveAttribute("aria-expanded", "false")
    await expect(window.locator("[data-highlight]")).toHaveCount(0)
    await column.filter({ hasText: "All Changes" }).click()
    await expect(header).toHaveCount(0)
    await expect(tests).toHaveAttribute("aria-expanded", "false")
    await expect(window.locator("[data-highlight]")).toHaveCount(0)
  })

  test("should tell a commit shown alone under Since Your Review by its own plan: its story, its steps and the marks back to it", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("review_tour")
    turnOnReviewPlan(repo)
    const window = await openPlanWindow(page, syncAndWaitForBranches, "layout")
    const { commitRow, column } = reviewSidebar(window)
    const bar = window.getByTestId("review-step-bar")
    await runAndWaitForAnswer(window, "mark_branch_reviewed", () => window.getByTestId("branch-review-mark-reviewed").click())
    await expect(window.getByText("Marked layout reviewed")).toBeVisible()

    // a commit on top whose body names the code it changes: a new count of the manager the frame paints by, and the
    // welcome apart; shown alone from Since Your Review
    await repo.commit(
      "(layout) the frame paints by the passes of the manager\n\nThe frame paints as many passes as the manager asks.\n\n`Frame.paint` asks `Manager.passes` for its count.",
      {
        "src/a/Manager.kt":
          "package a\n\nclass Manager {\n  fun open(): Int = 1\n\n  fun allowed(id: String): Boolean {\n    val named = id.isNotEmpty()\n    return named\n  }\n\n  fun passes(): Int = 3\n}\n",
        "src/a/Frame.kt":
          'package a\n\nclass Frame(val manager: Manager) {\n  fun show(): Int = if (manager.allowed("Project")) manager.open() else 0\n\n  fun paint(): Int = manager.passes()\n}\n',
        "src/b/Welcome.kt": "package b\n\nimport a.Manager\n\nclass Welcome(val manager: Manager) {\n  fun focus(): Int = 1\n}\n",
      },
      1704124800,
    )
    await syncAndWaitForBranches()
    await column.filter({ hasText: "Since Your Review" }).click()
    await commitRow("the frame paints by the passes of the manager").click()
    await commitRow("the frame paints by the passes of the manager").press("Enter")
    const header = window.getByTestId("range-review-commit-header")
    await expect(header).toContainText("the frame paints by the passes of the manager")

    // its own story once, in its heading; what is new since the review of the part below
    const overview = window.getByTestId("review-overview")
    await expect(header.getByTestId("review-story")).toHaveText("The frame paints as many passes as the manager asks.")
    await expect(overview.getByTestId("review-overview-untold")).toHaveCount(0)
    await expect(window.getByTestId("review-plan-since")).toHaveText("Since your review: 1 new")
    // the contents list its own steps: a click shows the opening of one
    const titles = overview.getByTestId("review-contents-step-title")
    await expect(titles.first()).toBeEnabled()
    await titles.first().click()
    await expect(bar.getByTestId("review-step-position")).toHaveText(/^Step 1 \/ \d+$/)

    // the section the body names marks it: a click outlines the line that names it where it is told
    const mark = window.locator('[data-testid="diff-view-file"][data-path="src/a/Frame.kt"]').getByTestId("review-story-mark")
    await window.mouse.move(0, 0)
    await mark.first().click()
    await expect(window.locator("[data-story-block][data-highlight]")).toHaveText("Frame.paint asks Manager.passes for its count.")
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

    // a click selects the commit and puts the opening of its step at the top, the commit stays in the story
    await expect(bar.getByTestId("review-step-position")).toHaveText("Overview")
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
    // none of Design is left: J moves nothing, so with All it goes on from Frame.kt to the opening of step 4, then to
    // its first card
    await window.keyboard.press("j")
    await window.getByTestId("branch-review-owner").filter({ hasText: "All" }).click()
    await expect(row(steps[0]!)).not.toHaveAttribute("data-muted")
    await window.getByTestId("commit-review-code").locator("[tabindex='0']").first().focus()
    await window.keyboard.press("j")
    await expect(row(steps[3]!)).toHaveAttribute("aria-selected", "true")
    await window.keyboard.press("j")
    await expect(row("spec/window.spec.md")).toHaveAttribute("aria-selected", "true")
  })
})
