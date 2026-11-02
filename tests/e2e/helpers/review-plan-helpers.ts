import { appendFileSync } from "node:fs"
import path from "node:path"
import type { Locator, Page } from "@playwright/test"
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

/**
 * A branch `story` of one commit on the `review_tour` template whose body tells two tasks, the splash and the theme: a
 * review of it lists the commit in no row, its tree lists the tasks. `THEME_ITEM` is told in the opening of the second
 * task, never in the heading of the commit
 */
export async function commitTwoTasks(repo: TestRepositoryBuilder): Promise<void> {
  await repo.commit(
    `(story) the cold start paints a splash in the accent of the theme\n\nThe cold start paints a splash at once, in the accent of the theme.\n\nThe splash:\n- \`Splash.show\` paints the splash\n\nThe theme:\n- ${THEME_ITEM}\n`,
    {
      "src/c/Splash.kt": "package c\n\nclass Splash {\n  fun show(): Int = 1\n}\n",
      "src/c/SplashScreen.kt": "package c\n\nclass SplashScreen(val splash: Splash) {\n  fun hide(): Int = splash.show() - 1\n}\n",
      "test/c/SplashTest.kt": "package c\n\nimport org.junit.Test\n\nclass SplashTest {\n  @Test fun `the splash shows once`() = check(SplashScreen(Splash()).hide() == 0)\n}\n",
      "src/d/Theme.kt": 'package d\n\nobject Theme {\n  fun accent(dark: Boolean): String = if (dark) "#88aaff" else "#3355cc"\n}\n',
      "test/d/ThemeTest.kt": 'package d\n\nimport org.junit.Test\n\nclass ThemeTest {\n  @Test fun `a dark accent`() = check(Theme.accent(true) == "#88aaff")\n}\n',
    },
    1704121200,
  )
}

/** The item of the task of the theme in the body `commitTwoTasks` commits */
export const THEME_ITEM = "`Theme.accent` answers the accent of the scheme"

/**
 * The heading of a commit reviewed alone carries the mark of its card, as its row would: the hover of the mark opens the
 * card, which holds `told` (a part of the body another place tells), the pointer leaving closes it; I on the focused
 * heading opens it with the focus in it, and Escape closes the card alone, the focus back on the heading
 */
export async function expectHeadingCard(window: Page, header: Locator, told: string): Promise<void> {
  const card = window.getByTestId("commit-card")
  // the diffs and the plan read: the overview is the prologue of the diffs, and its height stays
  await expect(window.getByTestId("commit-review-code")).toBeVisible()
  await expect(window.getByTestId("review-overview-reading")).toHaveCount(0)
  // a hover opens it after a move of the pointer over the mark
  await window.mouse.move(0, 0)
  await header.getByTestId("commit-row-mark").hover()
  await expect(card).toBeVisible()
  await expect(card).toContainText(told)
  await window.mouse.move(0, 0)
  await expect(card).toHaveCount(0)
  await header.focus()
  await window.keyboard.press("i")
  await expect(card).toBeFocused()
  await window.keyboard.press("Escape")
  await expect(card).toHaveCount(0)
  await expect(header).toBeFocused()
  await expect(window.getByTestId("commit-review")).toBeVisible()
}
