import { appendFileSync } from "node:fs"
import path from "node:path"
import type { Locator, Page } from "@playwright/test"
import { expect } from "../fixtures/test-fixtures"
import { writeFixtureIndex } from "./marks-helpers"
import { virtualBranch } from "./push-helpers"
import { openReviewWindow } from "./review-window-helpers"
import { openLinkedSubWindow } from "./sub-window-helpers"
import { nextAnswer } from "./sync-helpers"
import type { TestRepositoryBuilder } from "./test-repository"
import { expectClosed } from "./wait-helpers"

/*
 * The review window of a branch with the review plan on (openspec/specs/branch/review-plan/spec.md, openspec/specs/branch/review-tour/spec.md):
 * the flag turned on, the window opened from the menu of the row, and the files its diffs show
 */

/** The review plan on for the repository: the flag is off when unset, and the global config of the test machine says so too (`TEST_IDENTITY` of the server) */
export function turnOnReviewPlan(repo: TestRepositoryBuilder): void {
  // `git config branchdeck.reviewExperimental true` in a terminal: the next sync reads it, and it wins over the global
  appendFileSync(path.join(repo.path, ".git", "config"), "[branchdeck]\n\treviewExperimental = true\n")
}

/**
 * accio's modules (`accio modules`) of the `review_modules` template in the fixture of its index, at the base and the
 * head of the branch `media` the sync builds: the plugin, `shared/code` and `view.gui` the base declares, and the
 * module `view.gui.media` the head adds
 */
export async function answerPlayerModules(repo: TestRepositoryBuilder, syncAndWaitForBranches: () => Promise<unknown>): Promise<void> {
  await syncAndWaitForBranches()
  const head = await repo.revision(virtualBranch("media"))
  const base = await repo.revision(`${virtualBranch("media")}~2`)
  const module = (directory: string, name: string) => ({ root: `plugins/player/${directory}`, jps: { name } })
  const kept = [module("plugin", "intellij.player.plugin"), module("shared/code", "intellij.player.shared.code"), module("view.gui", "intellij.player.view.gui")]
  writeFixtureIndex(repo.path, { modules: { [base]: kept, [head]: [...kept, module("view.gui.media", "intellij.player.view.gui.media")] } })
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

/** The titles of a step or commit in the tree that are cut by their row or their own box */
export function cutTreeTitles(tree: Locator): Promise<number> {
  return tree.locator("[data-group=concern], [data-testid=review-commit-subject]").evaluateAll(
    titles =>
      titles.filter(title => {
        const row = title.closest('[role="treeitem"]')
        if (row === null) {
          return true
        }
        const clamp = getComputedStyle(title).getPropertyValue("-webkit-line-clamp")
        const titleRect = title.getBoundingClientRect()
        const rowRect = row.getBoundingClientRect()
        return (
          (clamp !== "none" && title.scrollHeight > title.clientHeight) ||
          title.scrollWidth > title.clientWidth ||
          titleRect.top < rowRect.top - 1 ||
          titleRect.right > rowRect.right + 1 ||
          titleRect.bottom > rowRect.bottom + 1 ||
          titleRect.left < rowRect.left - 1
        )
      }).length,
  )
}

/** The paths of the diffs the review shows of its files, in their order: no use left outside */
export async function reviewedPaths(page: Page): Promise<string[]> {
  return page
    .locator('[data-testid="diff-view-file"]:not([data-section^="ghost:"]) [data-testid="review-file-header"]')
    .evaluateAll(headers => headers.map(header => header.getAttribute("data-path")!))
}

/**
 * The review window of `branch` (`plan`) after a sync, as a page of its own beside the main window, its diffs shown. The
 * first file shows after the answers of the window's plan and of its diffs (`get_review_plan`, `get_file_diffs`): 0.1 s on
 * a quiet machine, 2.5 s at a load of 50-300 and past an assertion's 5 s in 3% of the opens, so each answer is waited for
 */
export async function openPlanWindow(page: Page, syncAndWaitForBranches: () => Promise<unknown>, branch = "plan"): Promise<Page> {
  await syncAndWaitForBranches()
  let answered!: Promise<unknown>
  const window = await openLinkedSubWindow(page, await openReviewWindow(page, branch), {
    beforeLoad: async window => {
      answered = Promise.all([nextAnswer(window, "get_review_plan"), nextAnswer(window, "get_file_diffs")])
    },
  })
  await answered
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
  await expectClosed(card)
  await header.focus()
  await window.keyboard.press("i")
  await expect(card).toBeFocused()
  await window.keyboard.press("Escape")
  await expectClosed(card)
  await expect(header).toBeFocused()
  await expect(window.getByTestId("commit-review")).toBeVisible()
}
