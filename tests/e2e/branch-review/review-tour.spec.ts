import { writeFileSync } from "node:fs"
import path from "node:path"
// @spec spec/branch/review-tour.spec.md
import type { Locator, Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { captureHtmlSnapshot } from "../helpers/aria-snapshot-helpers"
import { pressCopyKey, readClipboard } from "../helpers/clipboard-helpers"
import { mapPointOf } from "../helpers/file-map-helpers"
import { THEME_ITEM, commitTwoTasks, expectHeadingCard, openPlanWindow, reviewSidebar, reviewedPaths, turnOnReviewPlan } from "../helpers/review-plan-helpers"
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

/** The top of the opening of the step of `heading` below the top of the diffs: of a layer of several, its own */
function topOf(heading: Locator): Promise<number> {
  return heading.evaluate(element => {
    const view = element.closest("[data-testid=diff-view]")!
    // the card of the opening: its element of the layer of openings
    return Math.round(element.closest("[data-opening-step]")!.getBoundingClientRect().top - view.getBoundingClientRect().top)
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

type ReviewFindMarker = { kind: "text" | "diff"; layer: string | null; text: string }

async function openReviewFind(window: Page): Promise<Locator> {
  await window.getByTestId("diff-view").focus()
  await window.keyboard.press("ControlOrMeta+f")
  const field = window.getByRole("textbox", { name: "Find in the review" })
  await expect(field).toBeFocused()
  return field
}

function currentReviewFindMarker(window: Page): Promise<ReviewFindMarker | null> {
  return window.evaluate(() => {
    const highlight = typeof CSS === "undefined" || CSS.highlights === undefined ? undefined : CSS.highlights.get("bd-find-current")
    const range = highlight?.values().next().value
    if (range !== undefined) {
      const layer = range.startContainer.parentElement?.closest<HTMLElement>("[data-testid=diff-view-prologue], [data-testid=diff-view-opening], [data-testid=diff-view-epilogue]")
      const contents = document.createRange()
      contents.setStart(range.startContainer, range.startOffset)
      contents.setEnd(range.endContainer, range.endOffset)
      return { kind: "text", layer: layer?.dataset.testid ?? null, text: contents.cloneContents().textContent ?? "" }
    }
    const hit = document.querySelector<HTMLElement>(".dv-hit-current")
    return hit === null ? null : { kind: "diff", layer: hit.closest<HTMLElement>("[data-testid=diff-view-file]")?.dataset.path ?? null, text: hit.textContent ?? "" }
  })
}

// the `review_tour` template: the branch `layout` of five commits with bodies, a refactor of the manager, the rule
// `allowed` with its two uses and a test that calls it, a test of the welcome the branch does not touch, a change of the
// paint no test of the branch reaches, the restart story with a rule of the spec and its test link, and a round that renames the flag of
// the rule

/**
 * The index of the repository, as accio answers `tests --at … --tests <file>…` the code of the steps of `layout` at its
 * tip with the test files of the branch given
 */
function writeTestIndex(repositoryPath: string): void {
  const test = (file: string, line: number, readsAs: string) => ({ file, line, declaration: `Test.${readsAs}`, reads_as: readsAs, framework: "junit4", depth: 1, class: "direct" })
  const tests = {
    // `allowed`, by the test the branch adds, which calls it
    "src/a/Manager.kt:6": { tests: [test("test/a/ManagerTest.kt", 8, "the open may show only the stored layout")], complete: true },
    // `show`: no test of the branch reaches it, every way walked
    "src/a/Frame.kt:4": { tests: [], complete: true },
    // `focus` and `paint`: accio did not walk every way, unknown
    "src/b/Welcome.kt:6": { tests: [], complete: false },
    "src/a/Frame.kt:6": { tests: [], complete: false },
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
    // the branch as the title bar names it
    await expect(overview.getByTestId("review-overview-headline")).toHaveText("layout")
    const rule = overview.getByTestId("review-brief-contract-entry")
    await expect(rule).toContainText("Spec rule")
    await expect(rule).toContainText("The window keeps its stored layout across a restart, and the project view does not take the place of another tool window.")
    await expect(rule).toContainText("Proven by restart.json")

    // the contents: the steps the commits tell, each its full title, the lead of its commit's body and its facts
    await expect(overview.getByTestId("review-contents-step-title")).toHaveText([
      "the manager drops the hooks nothing sets",
      "the open keeps a stored layout",
      "the view paints with the first frame",
      "the restart story checks the layout",
    ])
    const step = (number: number) => overview.getByTestId("review-contents-step").nth(number - 1)
    await expect(step(2).getByTestId("review-contents-step-lead")).toContainText("Two startup steps showed the project view at each open")
    // the contents name the file of the tests of the change that reach the rule; the opening tells the rest
    await expect(step(2).getByTestId("review-contents-step-facts")).toHaveText("Tested by ManagerTest.kt · New API: allowed")
    // the paint, whose ways accio did not walk all: unknown, no warning; the story proven by its scenario
    await expect(step(3).getByTestId("review-contents-step-title")).toHaveText("the view paints with the first frame")
    await expect(step(3).getByTestId("review-contents-step-unproven")).toHaveCount(0)
    await expect(step(4).getByTestId("review-contents-step-facts")).toHaveText("Proven by restart.json")
    await captureHtmlSnapshot(overview, "review-overview")

    // the tree lists the steps, each the commit that starts it, its round under it, then its files, a file of two steps
    // with its row in the step of its most lines and a row "also in" the other; the diffs tell the story by them. Step 1
    // shows no card: the card of step 2 shows the hooks it removed, its file a row "also in" step 1
    const tree = window.getByRole("tree", { name: "Changed files" })
    const rows = () => tree.getByRole("treeitem").evaluateAll(items => items.map(item => item.getAttribute("aria-label")))
    await expect
      .poll(rows)
      .toEqual([
        "1 · the manager drops the hooks nothing sets",
        "src/a/Manager.kt, also in 1 · the manager drops the hooks nothing sets",
        "2 · the open keeps a stored layout",
        "the rule names its flag again",
        "src/a/Manager.kt",
        "src/a/Frame.kt",
        "src/b/Welcome.kt",
        "test/a/ManagerTest.kt",
        "3 · the view paints with the first frame",
        "src/a/Frame.kt, also in 3 · the view paints with the first frame",
        "4 · the restart story checks the layout",
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
      "Step 1 · the manager drops the hooks nothing sets",
      "Step 2 · the open keeps a stored layout",
      "Step 3 · the view paints with the first frame",
      "Step 4 · the restart story checks the layout",
    ])
    await expect(window.getByTestId("review-file-header").getByTestId("review-concern-heading")).toHaveCount(0)
    // the opening tells the story of the commit that starts the step: its lead, the rest of its body, its round as a
    // disclosure of its own body, the facts of the step
    const opening = window.getByTestId("review-opening").filter({ has: window.getByRole("button", { name: "Step 2 · the open keeps a stored layout", exact: true }) })
    await expect(opening.getByTestId("commit-body-lead").first()).toContainText("Two startup steps showed the project view at each open")
    await expect(opening).toContainText("The product decision is to keep the layout of the user.")
    // its facts a line each: the file of the tests of the change, its test and its way in the hover, the code no test of
    // the change reaches
    await expect(opening.getByTestId("review-opening-facts").getByTestId("review-step-fact")).toHaveText([
      "Tested by ManagerTest.kt",
      "No test of the change reaches show",
      "New API: allowed",
    ])
    await expect(opening.getByTestId("review-step-tested").filter({ hasText: "ManagerTest.kt" })).toHaveAttribute(
      "title",
      "the open may show only the stored layout: calls allowed",
    )
    const round = opening.getByTestId("review-opening-follow-up")
    await expect(round).toContainText("Follows upthe rule names its flag again")
    const foldedStory = round.getByText("The review asked for the name of the flag to say what it holds.", { exact: true })
    await expect(foldedStory).toBeAttached()
    await expect(foldedStory).toBeHidden()
    await round.getByRole("button", { name: /the rule names its flag again/ }).click()
    await expect(round).toContainText("The review asked for the name of the flag to say what it holds.")
    await captureHtmlSnapshot(opening, "review-opening")
    // the opening of the paint, unknown: no fact of its tests
    const paint = window.getByTestId("review-opening").filter({ has: window.getByRole("button", { name: "Step 3 · the view paints with the first frame", exact: true }) })
    await expect(paint).toBeVisible()
    await expect(paint.getByTestId("review-opening-facts")).toHaveCount(0)
    // the map of the files beside the diffs crosses itself at the start of each step with a card; its hover names the
    // file and its step
    const map = window.getByTestId("review-file-map")
    await expect(map).toHaveAttribute("data-steps", "3")
    // the first section of Manager.kt, the card below the openings of steps 1 and 2
    const start = await mapPointOf(window.locator("[data-testid=diff-view-file][data-path='src/a/Manager.kt']").first())
    await window.mouse.move(start.x, start.y)
    await expect(map.getByTestId("review-file-map-tip")).toHaveText(/Manager\.ktsrc\/a.*Step 2 · the open keeps a stored layout/)
    // an opening is measured in place before the frame paints: the layout has its height by the next frame
    await expect.poll(() => cardBelow(dividers.nth(1))).toEqual({ gap: 0, path: "src/a/Manager.kt" })
    await expect.poll(() => cardBelow(dividers.nth(3))).toEqual({ gap: 0, path: "spec/window.spec.md" })
    await expect.poll(() => headerNameLefts(window)).toEqual([expect.any(Number)])
    // the rule first in its step, its uses right after it, its test with them
    await expect
      .poll(async () => (await reviewedPaths(window)).slice(0, 6))
      .toEqual(["src/a/Manager.kt", "src/a/Frame.kt", "src/b/Welcome.kt", "test/a/ManagerTest.kt", "src/a/Frame.kt", "spec/window.spec.md"])
    await window.setViewportSize(viewport)

    // a step of the contents shows its opening at the top: the bar names it on one line, the tree selects the step
    await step(3).getByTestId("review-contents-step-title").click()
    await expect(bar).toHaveText("Step 3 / 4·the view paints with the first frame")
    await expect(bar).toHaveAttribute("title", "the view paints with the first frame")
    await expect(tree.getByRole("treeitem", { name: "3 · the view paints with the first frame", exact: true })).toHaveAttribute("aria-selected", "true")
    // J goes from the opening to its card, the tree to its file, then to the opening of the next step
    await window.keyboard.press("j")
    await expect(tree.getByRole("treeitem", { name: "src/a/Frame.kt, also in 3 · the view paints with the first frame", exact: true })).toHaveAttribute("aria-selected", "true")
    await window.keyboard.press("j")
    await expect(bar.getByTestId("review-step-position")).toHaveText("Step 4 / 4")
    await expect(tree.getByRole("treeitem", { name: "4 · the restart story checks the layout", exact: true })).toHaveAttribute("aria-selected", "true")
    // K goes back the same way, and from the opening of the first step to the overview
    await window.keyboard.press("k")
    await expect(tree.getByRole("treeitem", { name: "src/a/Frame.kt, also in 3 · the view paints with the first frame", exact: true })).toHaveAttribute("aria-selected", "true")
    await step(1).getByTestId("review-contents-step-title").click()
    await expect(bar.getByTestId("review-step-position")).toHaveText("Step 1 / 4")
    await window.keyboard.press("k")
    await expect(bar.getByTestId("review-step-position")).toHaveText("Overview")
    await window.keyboard.press("j")
    await expect(bar.getByTestId("review-step-position")).toHaveText("Step 1 / 4")
    // a click on the title of an opening shows the step as its bar does: the opening at the top, the bar and the tree follow
    const second = window.getByRole("button", { name: "Step 2 · the open keeps a stored layout", exact: true })
    await second.click()
    await expect(bar.getByTestId("review-step-position")).toHaveText("Step 2 / 4")
    await expect(tree.getByRole("treeitem", { name: "2 · the open keeps a stored layout", exact: true })).toHaveAttribute("aria-selected", "true")
    await expect.poll(() => topOf(second)).toBe(10)
    // a file of its tests shows its test in the diffs, the file selected in the tree
    await opening.getByTestId("review-step-tested").filter({ hasText: "ManagerTest.kt" }).click()
    await expect(window.getByRole("treeitem", { name: "ManagerTest.kt" })).toHaveAttribute("aria-selected", "true")
  })

  test("should find overview and opening text before the matching diff rows, and open a folded follow-up", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("review_tour")
    turnOnReviewPlan(repo)
    writeTestIndex(repo.path)
    const window = await openPlanWindow(page, syncAndWaitForBranches, "layout")
    const field = await openReviewFind(window)
    const count = window.getByTestId("diff-find-count")

    await field.fill("layout")
    await expect(count).toHaveText(/^\d+ results?$/)
    await expect.poll(() => window.evaluate(() => CSS.highlights.get("bd-find")?.size ?? 0)).toBeGreaterThan(0)
    const textCount = await window.evaluate(() => CSS.highlights.get("bd-find")?.size ?? 0)
    const total = Number((await count.textContent())?.split(" ")[0])
    expect(total).toBeGreaterThan(textCount)

    const visited: ReviewFindMarker[] = []
    for (let index = 0; index < total; index++) {
      await field.press("Enter")
      await expect(count).toHaveText(`${index + 1} of ${total}`)
      await expect.poll(() => currentReviewFindMarker(window)).not.toBeNull()
      const marker = await currentReviewFindMarker(window)
      if (marker !== null) {
        visited.push(marker)
      }
    }
    const prologue = visited.findIndex(hit => hit.kind === "text" && hit.layer === "diff-view-prologue")
    const opening = visited.findIndex(hit => hit.kind === "text" && hit.layer === "diff-view-opening")
    const diff = visited.findIndex(hit => hit.kind === "diff")
    expect(prologue).toBeGreaterThanOrEqual(0)
    expect(opening).toBeGreaterThan(prologue)
    expect(diff).toBeGreaterThan(opening)

    const round = window.getByTestId("review-opening-follow-up")
    const foldedStory = round.getByText("The review asked for the name of the flag to say what it holds.", { exact: true })
    await expect(foldedStory).toBeHidden()
    await field.fill("The review asked for the name of the flag to say what it holds.")
    await expect(count).toHaveText("1 result")
    await field.press("Enter")
    await expect(count).toHaveText("1 of 1")
    await expect(foldedStory).toBeVisible()
    await expect.poll(() => currentReviewFindMarker(window)).toMatchObject({ kind: "text", layer: "diff-view-opening" })
    await field.press("Escape")
    await expect(window.getByTestId("diff-find-bar")).toHaveCount(0)
    await expect.poll(() => window.evaluate(() => CSS.highlights.has("bd-find"))).toBe(false)
    await expect.poll(() => window.evaluate(() => CSS.highlights.has("bd-find-current"))).toBe(false)
  })

  test("should search story text in the epilogue and scroll to the hit", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("review_tour")
    turnOnReviewPlan(repo)
    const welcomeTest = (body: string) => `package b\n\nimport a.Manager\nimport org.junit.Test\n\nclass WelcomeTest {\n${body}}\n`
    const focuses = "  @Test fun `the welcome focuses its project`() = check(Welcome(Manager()).focus() >= 0)\n"
    const keeps = "\n  @Test fun `the welcome keeps the stored layout`() {\n    val welcome = Welcome(Manager())\n    val focused = welcome.focus()\n    check(focused >= 0)\n  }\n"
    await repo.commit(
      "(tail) the welcome test checks a stored layout\n\nThe welcome test checks that the welcome keeps the stored layout of its project.",
      { "test/b/WelcomeTest.kt": welcomeTest(focuses + keeps) },
      1704121200,
    )
    await repo.commit(
      "(tail) the welcome test drops the focus check\n\nThe check of the focus repeats the check of the layout, so it goes.",
      { "test/b/WelcomeTest.kt": welcomeTest(keeps) },
      1704121800,
    )
    const window = await openPlanWindow(page, syncAndWaitForBranches, "tail")
    const epilogue = window.getByTestId("diff-view-epilogue")
    const epilogueText = "The check of the focus repeats the check of the layout, so it goes."
    const field = await openReviewFind(window)
    const count = window.getByTestId("diff-find-count")

    await field.fill(epilogueText)
    await expect(count).toHaveText("2 results")
    await field.press("Enter")
    await expect(count).toHaveText("1 of 2")
    await expect.poll(() => currentReviewFindMarker(window)).toMatchObject({ kind: "text", layer: "diff-view-prologue" })
    await field.press("Enter")
    await expect(count).toHaveText("2 of 2")
    await expect(epilogue.getByText(epilogueText, { exact: true })).toBeInViewport()
    await expect.poll(() => currentReviewFindMarker(window)).toMatchObject({ kind: "text", layer: "diff-view-epilogue", text: epilogueText })
  })

  test("should open a hit in a closed story body and keep it current", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("review_tour")
    turnOnReviewPlan(repo)
    const window = await openPlanWindow(page, syncAndWaitForBranches, "layout")
    const restart = window.getByTestId("review-opening").filter({ has: window.getByRole("button", { name: "Step 4 · the restart story checks the layout", exact: true }) })
    const foldedTest = restart.getByText("restart.json restarts the window once", { exact: true })
    await expect(foldedTest).toBeAttached()
    await expect(foldedTest).toBeHidden()
    const field = await openReviewFind(window)
    const count = window.getByTestId("diff-find-count")

    await field.fill("restart.json restarts the window once")
    await expect(count).toHaveText("1 result")
    await field.press("Enter")
    await expect(count).toHaveText("1 of 1")
    await expect(foldedTest).toBeVisible()
    await expect(restart.getByTestId("commit-body-unfold")).toHaveAttribute("aria-expanded", "true")
    await expect.poll(() => currentReviewFindMarker(window)).toMatchObject({ kind: "text", layer: "diff-view-opening", text: "restart.json restarts the window once" })
    await expect(foldedTest).toBeInViewport()
  })

  test("should open a step whose code another step's card shows above the opening of that step, with no card, its unit named in that card, J and K through both openings", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("review_tour")
    turnOnReviewPlan(repo)
    const window = await openPlanWindow(page, syncAndWaitForBranches, "layout")
    const { tree } = reviewSidebar(window)
    const position = window.getByTestId("review-step-position")
    const openingOf = (name: string) => window.getByTestId("review-opening").filter({ has: window.getByRole("button", { name, exact: true }) })
    const first = openingOf("Step 1 · the manager drops the hooks nothing sets")
    const second = openingOf("Step 2 · the open keeps a stored layout")

    // step 1 removed the hooks, whose block the rule of step 2 holds: its opening stands right above the opening of
    // step 2, one layer above the card of Manager.kt, and its facts say where its change is shown
    const layer = window.getByTestId("diff-view-opening").filter({ has: first })
    await expect(layer.getByTestId("review-opening")).toHaveCount(2)
    await expect(layer.getByTestId("review-opening-title")).toHaveText(["the manager drops the hooks nothing sets", "the open keeps a stored layout"])
    await expect(layer).toHaveAttribute("data-section", "file:src/a/Manager.kt")
    await expect(first.getByTestId("review-opening-facts")).toHaveText("Removes hooks, shown in step 2")
    const manager = window.locator('[data-testid="diff-view-file"][data-path="src/a/Manager.kt"]')
    await expect(manager).toHaveCount(1)
    // its tree lists Manager.kt "also in" it, its own row in step 2
    const also = tree.getByRole("treeitem", { name: "src/a/Manager.kt, also in 1 · the manager drops the hooks nothing sets", exact: true })
    await expect(also.getByTestId("review-also-home")).toHaveText("in step 2")

    // the row of step 1 shows its opening at the top: the bar and the tree follow it
    await tree.getByRole("treeitem", { name: "1 · the manager drops the hooks nothing sets", exact: true }).click()
    await expect(position).toHaveText("Step 1 / 4")
    await expect.poll(() => topOf(first.getByTestId("review-concern-heading"))).toBe(10)
    // J goes from it to the opening of step 2, then to its card; K back the same way, then to the overview
    await window.keyboard.press("j")
    await expect(position).toHaveText("Step 2 / 4")
    await expect(tree.getByRole("treeitem", { name: "2 · the open keeps a stored layout", exact: true })).toHaveAttribute("aria-selected", "true")
    await window.keyboard.press("j")
    await expect(tree.getByRole("treeitem", { name: "src/a/Manager.kt", exact: true })).toHaveAttribute("aria-selected", "true")
    // the card of step 2 names the removal after the heading of its block, marked with the step it is of
    await expect(manager.getByTestId("diff-unit-heading").first()).toHaveText("allowed — new · hooks — removed in step 1")
    await window.keyboard.press("k")
    await expect(position).toHaveText("Step 2 / 4")
    await window.keyboard.press("k")
    await expect(position).toHaveText("Step 1 / 4")
    await expect(tree.getByRole("treeitem", { name: "1 · the manager drops the hooks nothing sets", exact: true })).toHaveAttribute("aria-selected", "true")
    await window.keyboard.press("k")
    await expect(position).toHaveText("Overview")
    // the link of the facts shows the step that shows the change
    await first.getByTestId("review-step-shown-elsewhere").click()
    await expect(position).toHaveText("Step 2 / 4")
    await expect.poll(() => topOf(second.getByTestId("review-concern-heading"))).toBe(10)
  })

  test("should open a last step whose code an earlier step's card shows below the last card, from its row, the overview, the end of that card and J, K going back to the card", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("review_tour")
    turnOnReviewPlan(repo)
    // the second commit removes the focus test alone, whose line the block of the test the first adds holds
    const test = (body: string) => `package b\n\nimport a.Manager\nimport org.junit.Test\n\nclass WelcomeTest {\n${body}}\n`
    const focuses = "  @Test fun `the welcome focuses its project`() = check(Welcome(Manager()).focus() >= 0)\n"
    const keeps = "\n  @Test fun `the welcome keeps the stored layout`() {\n    val welcome = Welcome(Manager())\n    val focused = welcome.focus()\n    check(focused >= 0)\n  }\n"
    await repo.commit(
      "(tail) the welcome test checks a stored layout\n\nThe welcome test checks that the welcome keeps the stored layout of its project.",
      { "test/b/WelcomeTest.kt": test(focuses + keeps) },
      1704121200,
    )
    await repo.commit(
      "(tail) the welcome test drops the focus check\n\nThe check of the focus repeats the check of the layout, so it goes.",
      { "test/b/WelcomeTest.kt": test(keeps) },
      1704121800,
    )
    const window = await openPlanWindow(page, syncAndWaitForBranches, "tail")
    const { tree } = reviewSidebar(window)
    const position = window.getByTestId("review-step-position")
    const stepRow = tree.getByRole("treeitem", { name: "2 · the welcome test drops the focus check", exact: true })
    const fileRow = tree.getByRole("treeitem", { name: "test/b/WelcomeTest.kt", exact: true })

    // the last step shows no card: its opening is the epilogue of the diffs, below the last card, and its facts say
    // where its change is shown; its tree lists the file "also in" it, its own row in step 1
    const last = window.getByTestId("diff-view-epilogue").getByTestId("review-opening")
    await expect(last.getByTestId("review-opening-title")).toHaveText("the welcome test drops the focus check")
    await expect(last.getByTestId("review-step-shown-elsewhere")).toHaveText("step 1")
    await expect(window.locator('[data-testid="diff-view-file"]')).toHaveCount(1)
    await expect(window.getByTestId("diff-unit-heading").first()).toContainText("in step 2")
    const also = tree.getByRole("treeitem", { name: "test/b/WelcomeTest.kt, also in 2 · the welcome test drops the focus check", exact: true })
    await expect(also.getByTestId("review-also-home")).toHaveText("in step 1")

    // its row shows its opening: the bar and the tree follow it
    await stepRow.click()
    await expect(position).toHaveText("Step 2 / 2")
    await expect(last).toBeInViewport()
    // K goes back to the card of step 1, J to the opening again, which J goes on from to nothing
    await window.keyboard.press("k")
    await expect(fileRow).toHaveAttribute("aria-selected", "true")
    await expect(position).toHaveText("Step 1 / 2")
    await window.keyboard.press("j")
    await expect(position).toHaveText("Step 2 / 2")
    await expect(stepRow).toHaveAttribute("aria-selected", "true")
    await window.keyboard.press("j")
    await expect(position).toHaveText("Step 2 / 2")
    await expect(stepRow).toHaveAttribute("aria-selected", "true")
    // the step of the overview shows it too
    await window.keyboard.press("k")
    await window.keyboard.press("k")
    await window.keyboard.press("k")
    await expect(position).toHaveText("Overview")
    await window.getByTestId("review-contents-step").nth(1).getByTestId("review-contents-step-title").click()
    await expect(position).toHaveText("Step 2 / 2")
    await expect(stepRow).toHaveAttribute("aria-selected", "true")
    await expect(last).toBeInViewport()
    // the line at the end of the last card names it, and a click shows it as J does
    await fileRow.click()
    await expect(position).toHaveText("Step 1 / 2")
    const end = window.getByTestId("review-card-end")
    await expect(end).toHaveAccessibleName("Next: Step 2 · the welcome test drops the focus check")
    await end.click()
    await expect(position).toHaveText("Step 2 / 2")
    await expect(stepRow).toHaveAttribute("aria-selected", "true")
  })

  test("should give the heading of a range of one commit the mark of its card, which the hover and I open, Escape giving the focus back", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("review_tour")
    turnOnReviewPlan(repo)
    await commitTwoTasks(repo)
    const window = await openPlanWindow(page, syncAndWaitForBranches, "story")
    // the tree lists the tasks of the commit, no row of it; its heading tells what is told before the tasks
    await expect(window.getByRole("tree", { name: "Changed files" }).getByTestId("commit-row-mark")).toHaveCount(0)
    const header = window.getByTestId("range-review-commit-header")
    await expect(header).toContainText("The cold start paints a splash at once")
    await expect(header).not.toContainText(THEME_ITEM.replaceAll("`", ""))
    // the card of the mark of its heading shows the whole message
    await expectHeadingCard(window, header, THEME_ITEM.replaceAll("`", ""))
  })

  test("should tell the whole body of a range of one commit of one task in its heading, whose mark opens its card, name a removed declaration after the heading of its block, and give a commit without a body no mark and no tab stop", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("review_tour")
    turnOnReviewPlan(repo)
    // one commit of one task: the test it adds holds the line of the test it removes, in one block
    await repo.commit(
      "(welcome) the welcome test checks a stored layout\n\nThe welcome test checks that the welcome keeps the stored layout of its project.\n\nThe check of the layout covers the focus too, so the check of the focus goes: `WelcomeTest.kt` keeps one test.\n",
      {
        "test/b/WelcomeTest.kt":
          "package b\n\nimport a.Manager\nimport org.junit.Test\n\nclass WelcomeTest {\n\n  @Test fun `the welcome keeps the stored layout`() {\n    val welcome = Welcome(Manager())\n    val focused = welcome.focus()\n    check(focused >= 0)\n  }\n}\n",
      },
      1704121200,
    )
    await repo.commit("(bare) the screen hides the splash twice", { "src/e/Twice.kt": "package e\n\nfun twice(): Int = 2\n" }, 1704124800)
    const window = await openPlanWindow(page, syncAndWaitForBranches, "welcome")
    // no step is told: the heading tells the whole body, and the card of the file names the removed test after the
    // heading of the block that holds its line, with no step
    const header = window.getByTestId("range-review-commit-header")
    await expect(header).toContainText("The welcome test checks that the welcome keeps the stored layout of its project.")
    await expect(header).toContainText("The check of the layout covers the focus too, so the check of the focus goes: WelcomeTest.kt keeps one test.")
    await expect(window.getByTestId("review-step-bar")).toHaveCount(0)
    // the line of the body that names the file marks its card
    await expect(window.locator('[data-testid="diff-view-file"][data-path="test/b/WelcomeTest.kt"]').getByTestId("review-story-mark")).toHaveCount(1)
    const heading = window.getByTestId("diff-unit-heading").first()
    await expect(heading).toContainText(/the welcome keeps the stored layout.* — new · .*the welcome focuses its project.* — removed$/)
    await expect(window.getByTestId("diff-unit-heading")).toHaveCount(1)
    await expectHeadingCard(window, header, "The check of the layout covers the focus too")
    await window.close()

    // a commit without a body: its heading has no mark and is no tab stop
    const bare = await openPlanWindow(page, syncAndWaitForBranches, "bare")
    const bareHeader = bare.getByTestId("range-review-commit-header")
    await expect(bareHeader).toContainText("the screen hides the splash twice")
    await expect(bareHeader.getByTestId("commit-row-mark")).toHaveCount(0)
    await expect(bareHeader).not.toHaveAttribute("tabindex")
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
    const opening = window.getByTestId("review-opening").filter({ has: window.getByRole("button", { name: "Step 2 · the open keeps a stored layout", exact: true }) })

    // `Manager.allowed` in the body of step 2 resolves to the rule the step adds: a link, its hover names it, its file,
    // its step
    const allowed = opening.getByTestId("commit-body-lead").getByTestId("review-story-mention").filter({ hasText: "Manager.allowed" })
    await expect(allowed).toHaveAttribute("data-path", "src/a/Manager.kt")
    await allowed.hover()
    // its card says what it names, never the name again: the status and the place, and no step, the reader is in it
    await expect(window.getByTestId("review-story-mention-tip")).toHaveText("NewManager.kt:6")
    // its name is the name it shows, its description what the card says
    await expect(allowed).toHaveAccessibleName("Manager.allowed")
    await expect(allowed).toHaveAccessibleDescription("New · Manager.kt:6")
    // a click shows the declaration: its file selected, its name selected in the diffs
    await allowed.click()
    await expect(tree.getByRole("treeitem", { name: "src/a/Manager.kt", exact: true })).toHaveAttribute("aria-selected", "true")
    await expect.poll(() => selectedInDiffs(window)).toEqual({ text: "allowed", path: "src/a/Manager.kt" })
    // the same name in the contents of the overview links too
    await expect(window.getByTestId("review-contents-step").nth(1).getByTestId("review-story-mention")).toHaveText("Manager.allowed")

    // back: the header of the section the body names marks it, its hover the line that names it, where it is told
    const manager = window.locator('[data-testid="diff-view-file"][data-path="src/a/Manager.kt"]').first()
    const mark = manager.getByTestId("review-story-mark")
    await expect(mark).toHaveCount(1)
    await mark.hover()
    // a card of the story: where it is told once, then the line that names the section, its code names links, inside
    // the window
    const tip = window.getByTestId("review-story-mark-tip")
    await expect(tip.getByText("Step 2 · the open keeps a stored layout", { exact: true })).toHaveCount(1)
    await expect(tip).toContainText("One rule decides now: Manager.allowed answers whether the open may show a tool window.")
    await expect(tip.getByTestId("review-story-mention").filter({ hasText: "Manager.allowed" })).toBeVisible()
    await expect.poll(async () => ((await tip.boundingBox())?.x ?? 0) + ((await tip.boundingBox())?.width ?? Infinity) <= window.viewportSize()!.width - 8).toBe(true)
    // the focus from the keyboard opens it too, above the header of its section, or under it at the top of the diffs,
    // never over it; it is read, not entered (Enter shows the story, whose names are links), and Escape closes it
    await window.mouse.move(0, 0)
    await expect(tip).toHaveCount(0)
    await mark.focus()
    await window.keyboard.press("Shift+Tab")
    await window.keyboard.press("Tab")
    await expect(mark).toBeFocused()
    await expect(tip).toBeVisible()
    const headerBox = (await manager.getByTestId("review-file-header").boundingBox())!
    const clearsHeader = async () => {
      const box = await tip.boundingBox()
      return box !== null && (box.y + box.height <= headerBox.y + 1 || box.y >= headerBox.y + headerBox.height - 1)
    }
    await expect.poll(clearsHeader).toBe(true)
    await window.keyboard.press("Escape")
    await expect(tip).toHaveCount(0)
    await expect(window.getByTestId("commit-review")).toBeVisible()
    // a click shows the opening of its step, the paragraph outlined
    await window.mouse.move(0, 0)
    await mark.click()
    await expect(bar.getByTestId("review-step-position")).toHaveText("Step 2 / 4")
    await expect(opening.locator("[data-story-block][data-highlight]")).toContainText("One rule decides now")
    await expect(window.locator("[data-story-block][data-highlight]")).toHaveCount(1)

    // the Look closely of the author is a callout in the opening of its step and in the contents; its tests are folded
    // always, named on the line of the folded parts
    const restart = window.getByTestId("review-opening").filter({ has: window.getByRole("button", { name: "Step 4 · the restart story checks the layout", exact: true }) })
    await expect(restart.getByTestId("commit-body-look-closely")).toContainText("the story restarts the window alone, never the whole IDE.")
    // the callout holds the question alone: the label right above its paragraph heads the story after it
    await expect(restart.getByTestId("commit-body-look-closely")).not.toContainText("takes a minute")
    await expect(restart.getByTestId("commit-body-part").filter({ hasText: "Why not the whole IDE" })).toContainText("A restart of the IDE takes a minute")
    await expect(window.getByTestId("review-contents-step").nth(3).getByTestId("review-contents-step-look-closely")).toContainText("the story restarts the window alone")
    const tests = restart.getByTestId("commit-body-fold")
    const folded = restart.getByTestId("commit-body-folded")
    await expect(folded).toHaveText("More:Tests")
    await expect(tests).toHaveAttribute("aria-expanded", "false")
    const foldedTest = restart.getByText("restart.json restarts the window once", { exact: true })
    await expect(foldedTest).toBeAttached()
    await expect(foldedTest).toBeHidden()
    // a name in a folded part marks its section too: a click opens the part, its block outlined; the open part is
    // headed by its label, the toggle that folds it, and leaves the line of what is still folded
    await tree.getByRole("treeitem", { name: "tests/restart.json", exact: true }).click()
    const restartMark = window.locator('[data-testid="diff-view-file"][data-path="tests/restart.json"]').getByTestId("review-story-mark")
    await window.mouse.move(0, 0)
    await restartMark.click()
    await expect(bar.getByTestId("review-step-position")).toHaveText("Step 4 / 4")
    const unfold = restart.getByTestId("commit-body-unfold")
    await expect(unfold).toHaveText("Tests")
    await expect(unfold).toHaveAttribute("aria-expanded", "true")
    await expect(folded).toHaveCount(0)
    await expect(restart.locator("[data-story-block][data-highlight]")).toHaveText("restart.json restarts the window once")
    // its label folds it again, back into the line, and the line opens it again below
    await unfold.click()
    await expect(tests).toHaveAttribute("aria-expanded", "false")
    await expect(foldedTest).toBeHidden()
    await tests.click()
    await expect(restart.getByTestId("commit-body-part").filter({ hasText: "restarts the window once" })).toBeVisible()

    // the outline plays once: the commit shown alone, then the whole story again, mount their stories with no block
    // outlined and no fold opened by the click before
    const { commitRow, column } = reviewSidebar(window)
    await commitRow("4 · the restart story checks the layout").click()
    await commitRow("4 · the restart story checks the layout").press("Enter")
    const header = window.getByTestId("range-review-commit-header")
    await expect(header.getByTestId("commit-body-fold")).toHaveAttribute("aria-expanded", "false")
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

  test("should make the row of a step its commit, its rounds rows of it, show the step on a click, fold it by its chevron, Space, Left and Right, show the commit alone on Enter, Show Commit Alone and Option+Down, and mark a step new since the review New", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("review_tour")
    turnOnReviewPlan(repo)
    const window = await openPlanWindow(page, syncAndWaitForBranches, "layout")
    const { tree, commitRow, column } = reviewSidebar(window)
    const header = window.getByTestId("range-review-commit-header")
    const bar = window.getByTestId("review-step-bar")
    const paint = commitRow("3 · the view paints with the first frame")

    // the column keeps the scopes; the commits are in the steps of the tree: the row of a step is the commit that starts
    // it, its round a row of it without the issue key, which says so
    await expect(commitRow("the rule names its flag again")).toBeVisible()
    await expect(column).toHaveText([/^All Changes5 commits$/])
    // its role is its indent and its description, never a word that takes the width of its subject
    await expect(commitRow("the rule names its flag again")).toHaveAttribute("aria-description", /^follows up, commit [0-9a-f]{7,}$/)
    await expect(commitRow("the rule names its flag again").getByText("follows up")).toHaveCount(0)
    await expect(commitRow("the open keeps a stored layout")).toHaveCount(0)
    await expect(commitRow("2 · the open keeps a stored layout")).toHaveAttribute("aria-description", /^4 files, \+\d+ −\d+, commit [0-9a-f]{7,}$/)
    await captureHtmlSnapshot(commitRow("2 · the open keeps a stored layout"), "review-tree-step-commit")
    // the row of a step is its commit: its hash in its card, no native hover over the card, and no box of its own that
    // takes the width of the title: the title runs to the mark at the end of the row
    const stepRow = commitRow("2 · the open keeps a stored layout")
    const stepTitle = stepRow.locator("[data-group]")
    await expect(stepTitle).not.toHaveAttribute("title")
    const titleRoom = await stepRow.evaluate(row => {
      const title = row.querySelector("[data-group]")!.getBoundingClientRect()
      const mark = row.querySelector("[data-testid=commit-row-mark]")!.getBoundingClientRect()
      return { gap: mark.left - title.right, sameLine: Math.abs(mark.top - title.top) < 8 }
    })
    expect(titleRoom.sameLine).toBe(true)
    expect(titleRoom.gap).toBeLessThan(12)
    // no title of a step or a commit is cut by the width: a title wraps to a second line, at 1440 and at 1024
    const cutTitles = () =>
      tree
        .locator("[data-group=concern], [data-testid=review-commit-subject]")
        .evaluateAll(titles => titles.filter(title => title.scrollWidth > title.clientWidth + 1 || title.scrollHeight > title.clientHeight + 1).length)
    await window.setViewportSize({ width: 1440, height: 900 })
    await expect.poll(cutTitles).toBe(0)
    // a narrow window: the directory after the name of a file shows whole or not at all, never cut to its last letters
    const wide = window.viewportSize()!
    await window.setViewportSize({ width: 1024, height: 768 })
    await expect.poll(cutTitles).toBe(0)
    const cut = () =>
      tree.getByTestId("file-label-directory").evaluateAll(
        directories =>
          directories.filter(directory => {
            const box = directory.getBoundingClientRect()
            const line = directory.parentElement!.getBoundingClientRect()
            return box.top < line.bottom && box.right > line.right + 0.5
          }).length,
      )
    await expect.poll(cut).toBe(0)
    await window.setViewportSize(wide)

    // a commit with a body shows the mark of its card: its hover opens the card, I on the focused row too, the focus in
    // it; Escape closes the card alone and gives the focus back to the row
    const card = window.getByTestId("commit-card")
    await window.mouse.move(0, 0)
    await commitRow("the rule names its flag again").getByTestId("commit-row-mark").hover()
    await expect(card.getByTestId("commit-message-body")).toHaveText("The review asked for the name of the flag to say what it holds.")
    await window.mouse.move(0, 0)
    await expect(card).toHaveCount(0)
    // the hover of its subject opens it too
    await commitRow("the rule names its flag again").getByTestId("review-commit-subject").hover()
    await expect(card.getByTestId("commit-message-body")).toHaveText("The review asked for the name of the flag to say what it holds.")
    await window.mouse.move(0, 0)
    await expect(card).toHaveCount(0)
    // the card of the row of a step opens beside the tree at the line of that row, the rows around it in view
    const stepMark = commitRow("2 · the open keeps a stored layout").getByTestId("commit-row-mark")
    await stepMark.hover()
    await expect(card).toBeVisible()
    await expect(card.getByTestId("commit-header-meta")).toContainText(/^[0-9a-f]{7,}/)
    const markBox = (await stepMark.boundingBox())!
    const treeBox = (await tree.boundingBox())!
    const atTheLine = async () => {
      const box = await card.boundingBox()
      return box !== null && box.x >= treeBox.x + treeBox.width - 1 && Math.abs(box.y - markBox.y) < 24
    }
    await expect.poll(atTheLine).toBe(true)
    await window.mouse.move(0, 0)
    await expect(card).toHaveCount(0)
    await commitRow("the rule names its flag again").focus()
    await window.keyboard.press("i")
    await expect(card).toBeFocused()
    await window.keyboard.press("Escape")
    await expect(card).toHaveCount(0)
    await expect(commitRow("the rule names its flag again")).toBeFocused()
    // the row of a step of the commits is its commit: I opens the card of that commit
    await commitRow("2 · the open keeps a stored layout").focus()
    await window.keyboard.press("i")
    await expect(card).toBeFocused()
    await expect(card.getByTestId("commit-message-body")).toContainText("The product decision is to keep the layout of the user.")
    await window.keyboard.press("Escape")
    await expect(card).toHaveCount(0)
    await expect(commitRow("2 · the open keeps a stored layout")).toBeFocused()

    // a click selects the step and puts its opening at the top; the commit stays in the story
    await expect(bar.getByTestId("review-step-position")).toHaveText("Overview")
    await paint.click()
    await expect(paint).toHaveAttribute("aria-selected", "true")
    await expect(bar.getByTestId("review-step-position")).toHaveText("Step 3 / 4")
    await expect(header).toHaveCount(0)
    // the click folds nothing: Space and the chevron do, a click on the folded row opens it again and shows its step
    await expect(paint).toHaveAttribute("aria-expanded", "true")
    await paint.press("Space")
    await expect(paint).toHaveAttribute("aria-expanded", "false")
    await paint.getByTestId("tree-row-chevron").click()
    await expect(paint).toHaveAttribute("aria-expanded", "true")
    await paint.getByTestId("tree-row-chevron").click()
    await expect(paint).toHaveAttribute("aria-expanded", "false")
    // → opens the focused step and ← folds it again, its opening stays at the top
    await paint.press("ArrowRight")
    await expect(paint).toHaveAttribute("aria-expanded", "true")
    await paint.press("ArrowLeft")
    await expect(paint).toHaveAttribute("aria-expanded", "false")
    await expect(bar.getByTestId("review-step-position")).toHaveText("Step 3 / 4")
    await commitRow("1 · the manager drops the hooks nothing sets").click()
    await expect(bar.getByTestId("review-step-position")).toHaveText("Step 1 / 4")
    await paint.click()
    await expect(paint).toHaveAttribute("aria-expanded", "true")
    await expect(bar.getByTestId("review-step-position")).toHaveText("Step 3 / 4")
    // ↓ moves the selection to the row below, ↑ back to the step, its opening at the top again
    await paint.press("ArrowDown")
    await expect(commitRow("src/a/Frame.kt, also in 3 · the view paints with the first frame")).toHaveAttribute("aria-selected", "true")
    await commitRow("src/a/Frame.kt, also in 3 · the view paints with the first frame").press("ArrowUp")
    await expect(paint).toHaveAttribute("aria-selected", "true")
    await expect(bar.getByTestId("review-step-position")).toHaveText("Step 3 / 4")
    // the copy key on the row copies the hash of its commit, whole: the one its description says short
    const hash = /commit ([0-9a-f]{7,})$/.exec((await paint.getAttribute("aria-description"))!)![1]
    await pressCopyKey(window)
    await expect.poll(() => readClipboard(window)).toMatch(new RegExp(`^${hash}[0-9a-f]{32,}$`))

    // Enter shows the commit alone: its header, the column lists every commit again; All Changes goes back to the story
    await paint.press("Enter")
    await expect(header).toContainText("the view paints with the first frame")
    await expect(column).toHaveCount(6)
    await column.filter({ hasText: "All Changes" }).click()
    await expect(header).toHaveCount(0)
    await expect(column).toHaveCount(1)

    // Show Commit Alone of its right-click menu does the same
    await commitRow("1 · the manager drops the hooks nothing sets").click({ button: "right" })
    await window.getByRole("menuitem", { name: "Show Commit Alone" }).click()
    await expect(header).toContainText("the manager drops the hooks nothing sets")
    await column.filter({ hasText: "All Changes" }).click()
    await expect(header).toHaveCount(0)

    // Option+Down steps from All Changes to the newest commit alone, as the column did
    await window.getByTestId("commit-review-code").locator("[tabindex='0']").first().focus()
    await window.keyboard.press("Alt+ArrowDown")
    await expect(header).toContainText("the rule names its flag again")

    // after Mark Reviewed a commit on top with a story: the row of its step says it is new since the review, the steps
    // reviewed before say nothing
    await column.filter({ hasText: "All Changes" }).click()
    await expect(header).toHaveCount(0)
    await runAndWaitForAnswer(window, "mark_branch_reviewed", () => window.getByTestId("branch-review-mark-reviewed").click())
    await expect(window.getByText("Marked layout reviewed")).toBeVisible()
    await repo.commit(
      "(layout) the splash shows before the frame\n\nA splash shows while the frame registers its tool windows.",
      { "src/b/Splash.kt": "package b\n\nclass Splash {\n  fun show(): Int = 2\n}\n" },
      1704124800,
    )
    await syncAndWaitForBranches()
    const splash = tree.getByRole("treeitem", { name: /^\d · the splash shows before the frame$/ })
    await expect(splash.getByTestId("review-commit-since")).toHaveText("New")
    await expect(splash).toHaveAttribute("aria-description", /, New, commit [0-9a-f]{7,}$/)
    await expect(paint.getByTestId("review-commit-since")).toHaveCount(0)
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
      "1 · the manager drops the hooks nothing sets",
      "2 · the open keeps a stored layout",
      "3 · the view paints with the first frame",
      "4 · the restart story checks the layout",
    ]
    const frameAlso = row("src/a/Frame.kt, also in 3 · the view paints with the first frame")
    // the steps, each its commit, their files (Manager.kt also in step 1) and the round of step 2
    await expect(tree.getByRole("treeitem")).toHaveCount(13)

    await window.getByTestId("branch-review-owner").filter({ hasText: "Design" }).click()
    await expect(window.getByTestId("branch-review-owner").filter({ hasText: "Design" })).toHaveAttribute("aria-pressed", "true")
    // a chip hides nothing: every step and file stays, the steps with its files count them, the others are muted, their
    // commits with them
    await expect(tree.getByRole("treeitem")).toHaveCount(13)
    await expect(row(steps[1]!).getByTestId("review-owner-mark")).toHaveText("Design2")
    await expect(row(steps[1]!)).toHaveAttribute("aria-description", /, 2 files of Design$/)
    await expect(row(steps[2]!).getByTestId("review-owner-mark")).toHaveText("Design1")
    await expect(row(steps[1]!)).not.toHaveAttribute("data-muted")
    for (const muted of [steps[0]!, steps[3]!, "src/a/Manager.kt", "test/a/ManagerTest.kt"]) {
      await expect(row(muted)).toHaveAttribute("data-muted", "")
    }
    await expect(row("the rule names its flag again")).not.toHaveAttribute("data-muted")
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

  test("should start a step at a commit with a story of its own though it rewrites the code of others, and name it in a step it took the code of, which shows its step", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("review_tour")
    turnOnReviewPlan(repo)
    // a story of its own, a lead and its labelled parts, that rewrites the focus of the welcome step 2 wrote and the
    // paint of step 3
    await repo.commit(
      "(layout) the welcome keeps its own focus\n\nThe welcome asked the rule for its focus, so a stored layout took it away. Now the welcome focuses itself, and the frame paints as the manager opens.\n\nThe cause:\n- the rule answered for the welcome too\n\nTests:\n- `WelcomeTest` focuses the welcome",
      {
        "src/b/Welcome.kt": "package b\n\nimport a.Manager\n\nclass Welcome(val manager: Manager) {\n  fun focus(): Int = 1\n}\n",
        "src/a/Frame.kt":
          'package a\n\nclass Frame(val manager: Manager) {\n  fun show(): Int = if (manager.allowed("Project")) manager.open() else 0\n\n  fun paint(): Int = manager.open()\n}\n',
      },
      1704121200,
    )
    const window = await openPlanWindow(page, syncAndWaitForBranches, "layout")
    const overview = window.getByTestId("review-overview")
    await expect(overview.getByTestId("review-contents-step-title")).toHaveText([
      "the manager drops the hooks nothing sets",
      "the open keeps a stored layout",
      "the view paints with the first frame",
      "the restart story checks the layout",
      "the welcome keeps its own focus",
    ])
    // the step that lands nothing names the step of the commit that took its code out, the title of that commit in its
    // hover; the name shows that step
    const paint = overview.getByTestId("review-contents-step").nth(2)
    await expect(paint.getByTestId("review-contents-step-facts")).toHaveText("Nothing of it lands: step 5 took it out")
    await expect(paint.getByTestId("review-step-taken-out-by")).toHaveAttribute("title", "the welcome keeps its own focus")
    await paint.getByTestId("review-step-taken-out-by").click()
    await expect(window.getByTestId("review-step-bar").getByTestId("review-step-position")).toHaveText("Step 5 / 5")
  })

  test("should title each task of one commit by the label of the section of its body it tells", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("review_tour")
    turnOnReviewPlan(repo)
    // one commit, two tasks: the splash and the accent it takes, each a labelled section of its body
    await repo.commit(
      "(story) the cold start paints a splash in the accent of the theme\n\nThe cold start paints a splash at once, and the theme of the splash follows the IDE.\n\nThe splash:\n- `Splash.show` paints the splash before the frame registers its tool windows\n- `SplashScreen.hide` takes it down after the first frame\n\nThe theme:\n- `Theme.accent` answers the accent of the current scheme\n\nTests:\n- `SplashTest` and `ThemeTest`",
      {
        "src/c/Splash.kt": "package c\n\nclass Splash {\n  fun show(): Int = 1\n}\n",
        "src/c/SplashScreen.kt": "package c\n\nclass SplashScreen(val splash: Splash) {\n  fun hide(): Int = splash.show() - 1\n}\n",
        "test/c/SplashTest.kt": "package c\n\nimport org.junit.Test\n\nclass SplashTest {\n  @Test fun `the splash shows once`() = check(SplashScreen(Splash()).hide() == 0)\n}\n",
        "src/d/Theme.kt": 'package d\n\nobject Theme {\n  fun accent(dark: Boolean): String = if (dark) "#88aaff" else "#3355cc"\n}\n',
        "test/d/ThemeTest.kt": 'package d\n\nimport org.junit.Test\n\nclass ThemeTest {\n  @Test fun `a dark accent`() = check(Theme.accent(true) == "#88aaff")\n}\n',
      },
      1704121200,
    )
    const window = await openPlanWindow(page, syncAndWaitForBranches, "story")
    const titles = window.getByTestId("review-overview").getByTestId("review-contents-step-title")
    await expect(titles.first()).toHaveText("The splash")
    await expect(titles.nth(1)).toHaveText("The theme")
    // the identifier stays the evidence that joined the task
    await expect(window.getByRole("tree", { name: "Changed files" }).getByRole("treeitem", { name: "1 · The splash", exact: true })).toBeVisible()
    await titles.nth(1).click()
    await expect(window.getByTestId("review-step-bar").getByTestId("review-step-title")).toHaveText("The theme")
  })

  test("should tell each step of one commit by the paragraph of its message that names its code, titled by its first sentence, which its opening reads a step larger", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("review_tour")
    turnOnReviewPlan(repo)
    // one commit, its paragraphs each naming the code it tells: the splash, then the theme, then the tests
    await repo.commit(
      "(story) the cold start paints a splash in the accent of the theme\n\nThe cold start paints a splash at once, in the accent of the theme.\n\nThe splash shows before the frame. `Splash.show` paints it and `SplashScreen.hide` takes it down.\n\nThe theme answers the accent of the scheme. `Theme.accent` reads it.\n\nTests:\n- `SplashTest` and `ThemeTest`",
      {
        "src/c/Splash.kt": "package c\n\nclass Splash {\n  fun show(): Int = 1\n}\n",
        "src/c/SplashScreen.kt": "package c\n\nclass SplashScreen(val splash: Splash) {\n  fun hide(): Int = splash.show() - 1\n}\n",
        "test/c/SplashTest.kt": "package c\n\nimport org.junit.Test\n\nclass SplashTest {\n  @Test fun `the splash shows once`() = check(SplashScreen(Splash()).hide() == 0)\n}\n",
        "src/d/Theme.kt": 'package d\n\nobject Theme {\n  fun accent(dark: Boolean): String = if (dark) "#88aaff" else "#3355cc"\n}\n',
        "test/d/ThemeTest.kt": 'package d\n\nimport org.junit.Test\n\nclass ThemeTest {\n  @Test fun `a dark accent`() = check(Theme.accent(true) == "#88aaff")\n}\n',
      },
      1704121200,
    )
    const window = await openPlanWindow(page, syncAndWaitForBranches, "story")
    const overview = window.getByTestId("review-overview")
    await expect(overview.getByTestId("review-contents-source")).toHaveText("One commit: the steps are the parts of its message")
    const titles = overview.getByTestId("review-contents-step-title")
    await expect(titles).toHaveText(["The splash shows before the frame.", "The theme answers the accent of the scheme.", "Tests"])
    // the tree names each step as its author did, by its number as the overview tells it
    const tree = window.getByRole("tree", { name: "Changed files" })
    await expect(tree.getByRole("treeitem", { name: "2 · The theme answers the accent of the scheme.", exact: true })).toBeVisible()
    // a step of its contents shows its opening at the top, which reads the paragraph of its title a step larger
    await titles.nth(1).click()
    await expect(window.getByTestId("review-step-bar").getByTestId("review-step-title")).toHaveText("The theme answers the accent of the scheme.")
    const opening = window.getByTestId("review-opening").filter({ hasText: "Step 2" })
    await expect(opening.getByTestId("commit-body-lead")).toHaveText("The theme answers the accent of the scheme. Theme.accent reads it.")
  })

  test("should tell no step of the one concern that covers the range: no Steps, no opening, no place in the bar", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("review_tour")
    turnOnReviewPlan(repo)
    await repo.commit(
      "(solo) the splash shows once\n\nThe cold start paints a splash at once and takes it down after the first frame.\n\nThe splash:\n- `Splash.show` paints it\n- `SplashScreen.hide` takes it down",
      {
        "src/c/Splash.kt": "package c\n\nclass Splash {\n  fun show(): Int = 1\n}\n",
        "src/c/SplashScreen.kt": "package c\n\nclass SplashScreen(val splash: Splash) {\n  fun hide(): Int = splash.show() - 1\n}\n",
      },
      1704121200,
    )
    const window = await openPlanWindow(page, syncAndWaitForBranches, "solo")
    const overview = window.getByTestId("review-overview")
    // the overview tells the whole body under the heading of the commit, its section too; the tree lists the files
    await expect(overview.getByTestId("range-review-commit-header")).toContainText("SplashScreen.hide takes it down")
    await expect(window.getByRole("tree", { name: "Changed files" }).getByRole("treeitem", { name: "src/c", exact: true })).toBeVisible()
    await expect(overview.getByTestId("review-contents")).toHaveCount(0)
    await expect(window.getByTestId("review-opening")).toHaveCount(0)
    await expect(window.getByTestId("review-step-bar")).toHaveCount(0)
  })

  test("should tell What Changes in whole paragraphs changed by word, a long one by the sentence of its change, and show the file its link names", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    turnOnReviewPlan(await setupRepo("review_prose"))
    const window = await openPlanWindow(page, syncAndWaitForBranches, "prose")
    const overview = window.getByTestId("review-overview")
    // the issue every subject names, also the one that starts with two keys, whose title keeps the other key
    await expect(overview.getByTestId("review-overview-issue")).toHaveText("IJAI-5640")
    await expect(overview.getByTestId("review-contents-step-title")).toHaveText([
      "the layout file keeps the tool window in focus",
      "IJAI-1184 the store drops only the layouts no pinned tool window holds",
    ])
    // one section once, its paragraphs whole: never the changed lines of a hard-wrapped paragraph
    const storage = overview.getByTestId("review-brief-section").filter({ has: window.getByRole("heading", { name: "Storage", exact: true }) })
    await expect(storage).toHaveCount(1)
    const paragraphs = storage.getByTestId("review-brief-paragraph")
    await expect(paragraphs).toHaveCount(3)
    // the long paragraph shows the sentence of its change, what the change wrote and what it removed in place
    const long = paragraphs.nth(0)
    await expect(long).toHaveAttribute("data-change", "changed")
    await expect(long.getByTestId("review-brief-inserted")).toHaveText(PROSE_INSERTED)
    await expect(long.getByTestId("review-brief-deleted")).toHaveText(PROSE_DELETED)
    await expect(long).toContainText("The file holds the tool windows of each project: their side, their size")
    await expect(long).not.toContainText("renamed aside")
    // an ellipsis opens it whole
    await long.getByRole("button", { name: "Show the whole paragraph" }).first().click()
    await expect(long).toContainText("A file that cannot be read is renamed aside, and the window opens with the default layout.")
    await expect(long.getByRole("button", { name: "Show the whole paragraph" })).toHaveCount(0)
    // words the second commit appends to the next paragraph
    await expect(paragraphs.nth(1).getByTestId("review-brief-inserted")).toHaveText([" that pin no tool window"])
    // the paragraph the change adds reads its link as its text, which shows the file it names in the review
    const added = paragraphs.nth(2)
    await expect(added).toHaveAttribute("data-change", "added")
    await expect(added).toContainText("The focus of a tool window is a fact the window derives from the last click. Window Focus owns its rules.")
    await added.getByTestId("review-brief-link").click()
    await expect(reviewSidebar(window).tree.getByRole("treeitem", { name: "spec/focus.spec.md", exact: true })).toHaveAttribute("aria-selected", "true")
    await captureHtmlSnapshot(overview.getByTestId("review-brief-contract"), "review-brief-prose")
  })

  test("should name a repeated substitution once, show its paragraphs quiet in place, a change inside code inside its chip and a fenced block as code", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    turnOnReviewPlan(await setupRepo("review_prose"))
    const window = await openPlanWindow(page, syncAndWaitForBranches, "prose")
    const overview = window.getByTestId("review-overview")
    const contract = overview.getByTestId("review-brief-contract")
    const substitution = contract.getByTestId("review-brief-substitution")
    await expect(substitution).toContainText("Renames bd → brd · 9 places in 5 files")
    await expect(substitution.getByTestId("review-brief-substitution-file")).toHaveCount(3)
    const moreFiles = substitution.getByTestId("review-brief-substitution-more")
    await expect(moreFiles).toHaveText("and 2 more")
    await captureHtmlSnapshot(contract, "review-brief-substitutions")
    await moreFiles.hover()
    const tooltip = window.getByRole("tooltip")
    await expect(tooltip).toContainText("spec/terminal.spec.md")
    await expect(tooltip).toContainText("docs/agent-guide.md")

    const summaryFile = substitution.getByTestId("review-brief-substitution-file").first()
    const path = await summaryFile.getAttribute("data-path")
    if (path === null) {
      throw new Error("The substitution file button must identify its file")
    }
    await summaryFile.click()
    await expect(reviewSidebar(window).tree.getByRole("treeitem", { name: path, exact: true })).toHaveAttribute("aria-selected", "true")

    const commands = overview.getByTestId("review-brief-section").filter({ has: window.getByRole("heading", { name: "Commands", exact: true }) })
    const quiet = commands.getByTestId("review-brief-paragraph").filter({ hasText: "to read the whole change." })
    await expect(quiet).toHaveAttribute("data-change", "substituted")
    const quietWord = quiet.getByTestId("review-brief-substitution-word")
    await quietWord.hover()
    await expect(window.getByRole("tooltip").filter({ hasText: "was bd" })).toHaveText("was bd")

    const mixed = commands.getByTestId("review-brief-paragraph").filter({ hasText: "Verify the status." })
    const chip = mixed.getByTestId("review-brief-code-chip")
    await expect(mixed).toHaveAttribute("data-change", "changed")
    await expect(chip).toHaveCount(1)
    await expect(chip.getByTestId("review-brief-deleted")).toHaveText("short")
    await expect(chip.getByTestId("review-brief-inserted")).toHaveText("compact")
    await expect(chip.getByTestId("review-brief-substitution-word")).toContainText("brd")

    const code = commands.getByTestId("review-brief-code-block")
    await expect(code).toHaveAttribute("data-change", "substituted")
    await expect(code.getByTestId("review-brief-substitution-word")).toHaveCount(3)
    const expand = code.getByRole("button", { name: "Show the whole paragraph" })
    await expect(expand).toHaveText(/⋯ \d+ more lines/)
    await expand.hover()
    await expect(window.getByRole("tooltip").filter({ hasText: "Show the whole paragraph" })).toHaveText("Show the whole paragraph")
    await expand.click()
    await expect(code).toContainText("printf 'complete'")
  })
})

/** What the first commit of `review_prose` writes and removes in the long paragraph, as the word diff reads it */
const PROSE_INSERTED = [",", " and the tool window in focus"]
const PROSE_DELETED = ["and"]
