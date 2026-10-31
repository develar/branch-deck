// @spec spec/branch/review-tour.spec.md
import { test, expect } from "../fixtures/test-fixtures"
import { captureHtmlSnapshot } from "../helpers/aria-snapshot-helpers"
import { openPlanWindow, reviewedPaths, turnOnReviewPlan } from "../helpers/review-plan-helpers"

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

    // the tree lists the steps, each with its files, a file of two steps with its row in the step of its most lines and a
    // row "also in" the other; the diffs tell the story by them
    const tree = window.getByRole("tree", { name: "Changed files" })
    const rows = () => tree.getByRole("treeitem").evaluateAll(items => items.map(item => item.getAttribute("aria-label")))
    await expect
      .poll(rows)
      .toEqual([
        "1 · The manager drops the hooks nothing sets",
        "src/a/Manager.kt, also in 1 · The manager drops the hooks nothing sets",
        "2 · The open keeps a stored layout",
        "src/a/Manager.kt",
        "src/a/Frame.kt",
        "src/b/Welcome.kt",
        "test/a/ManagerTest.kt",
        "3 · The view paints with the first frame",
        "src/a/Frame.kt, also in 3 · The view paints with the first frame",
        "4 · The restart story checks the layout",
        "spec/window.spec.md",
        "tests/restart.json",
      ])
    await expect(window.getByTestId("review-concern-heading")).toHaveText([
      "1 · The manager drops the hooks nothing sets",
      "2 · The open keeps a stored layout",
      "3 · The view paints with the first frame",
      "4 · The restart story checks the layout",
    ])
    // the rule first in its step, its uses right after it, its test with them (the view renders the cards on screen)
    await expect
      .poll(async () => (await reviewedPaths(window)).slice(0, 7))
      .toEqual(["src/a/Manager.kt", "src/a/Manager.kt", "src/a/Frame.kt", "src/b/Welcome.kt", "test/a/ManagerTest.kt", "src/a/Frame.kt", "spec/window.spec.md"])

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
  })
})
