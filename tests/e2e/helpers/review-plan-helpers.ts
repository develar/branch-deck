import { appendFileSync } from "node:fs"
import path from "node:path"
import type { Page } from "@playwright/test"
import { expect } from "../fixtures/test-fixtures"
import { openReviewWindow } from "./review-window-helpers"
import { openLinkedSubWindow } from "./sub-window-helpers"
import type { TestRepositoryBuilder } from "./test-repository"

/*
 * The review window of a branch with the review plan on (spec/branch/review-plan.spec.md, spec/branch/review-tour.spec.md):
 * the flag turned on, the window opened from the menu of the row, and the files its diffs show
 */

/** The review plan on for the repository: the flag is off when unset, and the global config of the test machine says so too (`TEST_IDENTITY` of the server) */
export function turnOnReviewPlan(repo: TestRepositoryBuilder): void {
  // `git config branchdeck.reviewExperimental true` in a terminal: the next sync reads it, and it wins over the global
  appendFileSync(path.join(repo.path, ".git", "config"), "[branchdeck]\n\treviewExperimental = true\n")
}

/** The sidebar of a review with the plan: the tree, a commit row of it by its subject, and the rows of the column above it */
export function reviewSidebar(window: Page) {
  const tree = window.getByRole("tree", { name: "Changed files" })
  return {
    tree,
    commitRow: (subject: string) => tree.getByRole("treeitem", { name: subject, exact: true }),
    column: window.getByTestId("range-review-commits").getByTestId("range-review-commit"),
  }
}

/** The paths of the diffs the review shows of its files, in their order: no use left outside */
export async function reviewedPaths(page: Page): Promise<string[]> {
  return page
    .locator('[data-testid="diff-view-file"]:not([data-section^="ghost:"]) [data-testid="review-file-header"]')
    .evaluateAll(headers => headers.map(header => header.getAttribute("data-path")!))
}

/** The review window of `branch` (`plan`) after a sync, as a page of its own beside the main window, its diffs shown */
export async function openPlanWindow(page: Page, syncAndWaitForBranches: () => Promise<unknown>, branch = "plan"): Promise<Page> {
  await syncAndWaitForBranches()
  const window = await openLinkedSubWindow(page, await openReviewWindow(page, branch))
  await expect(window.getByTestId("review-file-header").first()).toBeVisible()
  return window
}
