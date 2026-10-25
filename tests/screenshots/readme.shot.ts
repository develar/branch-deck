// The images of the README, light and dark (`pnpm screenshots:readme`), from the `readme_showcase` template: the main
// window with the notes app of alex (a stack of four branches, pull requests in four states on GitHub), and the review
// window of a branch of the stack. No Patronus: the README is the pitch for any repository on GitHub.
// SHOTS_DIR=<dir> writes them there instead of docs/screenshots.
import path from "node:path"
import type { Page } from "@playwright/test"
import type { PullRequest } from "../../src/lib/bindings"
import { test, expect } from "../e2e/fixtures/test-fixtures"
import { expandBranch, findBranchRow } from "../e2e/helpers/branch-helpers"
import type { PullRequestsFixture } from "../e2e/helpers/pull-request-helpers"
import { pullRequest, reviewer } from "../e2e/helpers/pull-request-helpers"
import { routePush } from "../e2e/helpers/push-helpers"
import { getOpenedSubWindows, getSubWindowReadyOpen, openLinkedSubWindow } from "../e2e/helpers/sub-window-helpers"

const SHOTS_DIR = process.env.SHOTS_DIR ?? path.join(import.meta.dirname, "../../docs/screenshots")
const REPOSITORY = { owner: "acme", name: "notes" }

/** A pull request of the branch `branch` of alex on acme/notes */
function showcasePullRequest(branch: string, number: number, title: string, patch: Partial<PullRequest>): PullRequest {
  return pullRequest(branch, number, {
    title,
    url: `https://github.com/${REPOSITORY.owner}/${REPOSITORY.name}/pull/${number}`,
    headRefName: `alex/virtual/${branch}`,
    author: "alex",
    ...patch,
  })
}

const PULL_REQUESTS: PullRequestsFixture = {
  availability: { status: "available", ...REPOSITORY },
  pullRequests: {
    sync: showcasePullRequest("sync", 412, "Download only the edits after the last sync", {
      reviewDecision: "approved",
      checks: { state: "passing", passed: 6, failed: 0, pending: 0, failedNames: [] },
      reviewers: [reviewer("maria", "approved")],
    }),
    conflicts: showcasePullRequest("conflicts", 413, "Merge two edits of the same paragraph word by word", {
      checks: { state: "passing", passed: 6, failed: 0, pending: 0, failedNames: [] },
      reviewers: [reviewer("maria", "requested")],
    }),
    "ABC-123": showcasePullRequest("ABC-123", 409, "Show the upload progress in the status bar", {
      reviewDecision: "changesRequested",
      checks: { state: "passing", passed: 6, failed: 0, pending: 0, failedNames: [] },
      reviewers: [reviewer("sam", "changesRequested")],
    }),
    search: showcasePullRequest("search", 415, "Rank the notes whose file name matches first", {
      checks: { state: "failing", passed: 5, failed: 1, pending: 0, failedNames: ["ui-tests"] },
      reviewers: [reviewer("sam", "requested")],
    }),
    docs: showcasePullRequest("docs", 416, "Document the keyboard shortcuts", { isDraft: true }),
  },
}

/** Waits until the transitions and animations of the page end, the colours of a theme switch too (a spinner never does) */
async function settled(page: Page): Promise<void> {
  await page.evaluate(() =>
    Promise.all(
      document
        .getAnimations()
        .filter(animation => animation.effect?.getComputedTiming().endTime !== Infinity)
        .map(animation => animation.finished.catch(() => undefined)),
    ),
  )
}

/** Pushes `branch` from its row menu, as the user does (a branch of a stack with its parents), and closes its toast */
async function push(page: Page, branch: string): Promise<void> {
  await findBranchRow(page, branch).click({ button: "right" })
  await page.getByRole("menuitem", { name: /^Push (\(\d+\)|Stack )/ }).click()
  const toast = page.locator("[data-slot=toast]").filter({ hasText: /^Pushed / })
  await expect(toast).toBeVisible()
  await toast.locator("[data-slot=toast-close]").click()
  await expect(toast).toHaveCount(0)
  await page.keyboard.press("Escape")
}

test("README: the branches of a day of work", async ({ page, setupRepo, syncAndWaitForBranches }) => {
  await setupRepo("readme_showcase", { pullRequests: PULL_REQUESTS })
  await syncAndWaitForBranches()
  await expect(findBranchRow(page, "offline")).toBeVisible()
  // the E2E server has no remote: the page answers the pushes
  await routePush(page, {})
  for (const branch of ["conflicts", "ABC-123", "search", "docs"]) {
    await push(page, branch)
  }

  // the path of the fixture repository is a temporary directory: the selector shows the path a user would have
  await page
    .locator("[data-slot=select-trigger] bdi")
    .first()
    .evaluate(element => (element.textContent = "~/projects/notes"))
  // a branch of the stack with its commits; the pointer off the rows
  await expandBranch(page, findBranchRow(page, "conflicts"))
  await page.mouse.move(0, 0)
  await expect(page.getByRole("tooltip")).toHaveCount(0)
  // the window down to the last card
  const bottom = await page.evaluate(() => Math.max(...[...document.querySelectorAll("[data-slot=card]")].map(it => it.getBoundingClientRect().bottom)))
  const clip = { x: 0, y: 0, width: page.viewportSize()?.width ?? 1100, height: Math.ceil(bottom) + 12 }

  for (const colorScheme of ["light", "dark"] as const) {
    await page.emulateMedia({ colorScheme })
    await settled(page)
    await page.screenshot({ path: path.join(SHOTS_DIR, `branch-deck-${colorScheme}.png`), clip })
  }

  // Review Changes of the branch menu: the review window of `conflicts`, its files in one scrolling view
  await page.emulateMedia({ colorScheme: "light" })
  await findBranchRow(page, "conflicts").click({ button: "right" })
  await page.getByRole("menuitem", { name: "Review Changes" }).click()
  await expect.poll(async () => (await getOpenedSubWindows(page)).length).toBe(1)
  const review = await openLinkedSubWindow(page, (await getOpenedSubWindows(page))[0]!)
  await review.setViewportSize({ width: 1280, height: 760 })
  await expect(review.getByTestId("review-file-header").first()).toBeVisible()
  await expect.poll(() => getSubWindowReadyOpen(review)).not.toBeNull()
  // the changed file in view, beside the end of the added one
  await review.getByRole("tree", { name: "Changed files" }).getByText("SyncEngine.kt", { exact: true }).click()
  await review.mouse.move(0, 0)
  for (const colorScheme of ["light", "dark"] as const) {
    await review.emulateMedia({ colorScheme })
    await settled(review)
    await review.screenshot({ path: path.join(SHOTS_DIR, `review-${colorScheme}.png`) })
  }
})
