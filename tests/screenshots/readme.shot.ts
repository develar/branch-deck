// The images of the README, light and dark (`pnpm screenshots:readme`), from the `readme_showcase` template: the main
// window with the notes app of alex (a stack of four branches, pull requests in four states on GitHub, a pull request
// of maria in Your Turn that leaves nothing to read), and the review window of the commit of the stack that moves a
// function from one file to another. The review plan is on, as for a user. No Patronus: the README is the pitch for
// any repository on GitHub.
// SHOTS_DIR=<dir> writes them there instead of docs/screenshots.
import path from "node:path"
import type { Page } from "@playwright/test"
import type { PullRequest } from "../../src/lib/bindings"
import { test, expect } from "../e2e/fixtures/test-fixtures"
import { expandBranch, findBranchRow, getBranchCommitRow } from "../e2e/helpers/branch-helpers"
import { diffRows, moveNotes, renamedNames } from "../e2e/helpers/diff-helpers"
import type { PullRequestsFixture } from "../e2e/helpers/pull-request-helpers"
import { pullRequest, reviewer, VIEWER_LOGIN } from "../e2e/helpers/pull-request-helpers"
import { turnOnReviewPlan } from "../e2e/helpers/review-plan-helpers"
import { getReviewRow, reviewRequest } from "../e2e/helpers/review-request-helpers"
import { pressAndExpectReviewOpened } from "../e2e/helpers/review-window-helpers"
import { getSubWindowReadyOpen, openLinkedSubWindow } from "../e2e/helpers/sub-window-helpers"
import { closeToast, openToast } from "../e2e/helpers/toast-helpers"
import { expectClosed } from "../e2e/helpers/wait-helpers"

const SHOTS_DIR = process.env.SHOTS_DIR ?? path.join(import.meta.dirname, "../../docs/screenshots")
const REPOSITORY = { owner: "acme", name: "notes" }
/** The commit of `conflicts` that moves `diffWords` to the resolver, renames a local of `merge` and wraps its last call */
const MOVE_SUBJECT = "Move the word diff next to the resolver"

/** A pull request of the branch `branch` of alex on acme/notes, by the signed-in user: the fake GitHub answers only those */
function showcasePullRequest(branch: string, number: number, title: string, patch: Partial<PullRequest>): PullRequest {
  return pullRequest(branch, number, {
    title,
    url: `https://github.com/${REPOSITORY.owner}/${REPOSITORY.name}/pull/${number}`,
    headRefName: `alex/virtual/${branch}`,
    author: VIEWER_LOGIN,
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
  await page.getByRole("menuitem", { name: /^Push (Branch \(\d+\)|Stack )/ }).click()
  const toast = openToast(page, /^Pushed /)
  await expect(toast).toBeVisible()
  await closeToast(toast)
}

/** The pull request of maria that waits for alex's review: `maria/word-count` of the template, formatting and a renamed local */
const WORD_COUNT_REVIEW = reviewRequest("word-count", 417, 3, {
  title: "Tidy the word count",
  url: `https://github.com/${REPOSITORY.owner}/${REPOSITORY.name}/pull/417`,
  author: "maria",
  requestedBy: "maria",
  headRefName: "maria/word-count",
  additions: 4,
  deletions: 3,
  changedFiles: 1,
  labels: ["refactoring"],
})

test("README: the branches of a day of work", async ({ page, setupRepo, syncAndWaitForBranches }) => {
  const repo = await setupRepo("readme_showcase", {
    pullRequests: PULL_REQUESTS,
    reviewRequests: { availability: { status: "available", ...REPOSITORY }, totalCount: 1, requests: [WORD_COUNT_REVIEW] },
  })
  // the review plan, on for a user who never set it; the test server turns it off
  turnOnReviewPlan(repo)
  await syncAndWaitForBranches()
  await expect(findBranchRow(page, "offline")).toBeVisible()
  // Your Turn says what the pull request of maria leaves to read before it is opened
  await expect(getReviewRow(page, 417).getByTestId("review-request-reading")).toHaveText(/^Nothing to read/)
  for (const branch of ["conflicts", "ABC-123", "search", "docs"]) {
    await push(page, branch)
  }

  // the path of the fixture repository is a temporary directory: the selector shows the path a user would have
  await page
    .getByTestId("repository-path")
    .first()
    .evaluate(element => (element.textContent = "~/projects/notes"))
  // a branch of the stack with its commits; the pointer off the rows
  await expandBranch(page, findBranchRow(page, "conflicts"))
  await page.mouse.move(0, 0)
  await expectClosed(page.getByRole("tooltip"))
  // the window down to the last card
  const bottom = await page.evaluate(() => Math.max(...[...document.querySelectorAll("[data-slot=card]")].map(it => it.getBoundingClientRect().bottom)))
  const clip = { x: 0, y: 0, width: page.viewportSize()?.width ?? 1100, height: Math.ceil(bottom) + 12 }

  for (const colorScheme of ["light", "dark"] as const) {
    await page.emulateMedia({ colorScheme })
    await settled(page)
    await page.screenshot({ path: path.join(SHOTS_DIR, `branch-deck-${colorScheme}.png`), clip })
  }

  // D on the commit that moves `diffWords` from the engine to the resolver: its review window, both ends of the move
  await page.emulateMedia({ colorScheme: "light" })
  await getBranchCommitRow(page, "conflicts", MOVE_SUBJECT).hover()
  const review = await openLinkedSubWindow(page, await pressAndExpectReviewOpened(page, () => page.keyboard.press("d"), 1))
  await review.setViewportSize({ width: 1280, height: 1040 })
  await expect(review.getByTestId("review-file-header").first()).toBeVisible()
  await expect.poll(() => getSubWindowReadyOpen(review)).not.toBeNull()
  await expect(review.getByRole("button", { name: "Inline", exact: true })).toHaveAttribute("aria-pressed", "true")
  // the marks of the subject come after the diffs: a note at each end of the move, the rename named once
  await expect(moveNotes(review, "source")).toContainText(["Moved to ConflictResolver.kt"])
  await expect(moveNotes(review, "destination")).toContainText(["Moved from SyncEngine.kt"])
  await expect(renamedNames(review).first()).toBeVisible()
  // the plan of the commit above, the diffs in view from the note of the rename in `merge` down: the moved lines where
  // they stand now, then where they left the engine
  const view = review.getByTestId("diff-view")
  const renamed = diffRows(view, "note", { hasText: "Renamed" }).first()
  const header = review.getByTestId("review-file-header").first()
  const [viewBox, renamedBox] = await Promise.all([view.boundingBox(), renamed.boundingBox()])
  await view.evaluate((element, down) => (element.scrollTop += down), renamedBox!.y - viewBox!.y)
  // back up under the header of the file, which stays at the top of the view once it scrolled there
  const [headerBox, underBox] = await Promise.all([header.boundingBox(), renamed.boundingBox()])
  await view.evaluate((element, up) => (element.scrollTop -= up), headerBox!.y + headerBox!.height - underBox!.y)
  await review.mouse.move(0, 0)
  for (const colorScheme of ["light", "dark"] as const) {
    await review.emulateMedia({ colorScheme })
    await settled(review)
    await review.screenshot({ path: path.join(SHOTS_DIR, `review-${colorScheme}.png`) })
  }
})
