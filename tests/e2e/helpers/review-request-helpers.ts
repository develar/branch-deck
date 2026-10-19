import type { Locator, Page } from "@playwright/test"
import { expect } from "@playwright/test"
import type { ReviewRequest, ReviewRequestsResult } from "../../../src/lib/bindings"
import type { PullRequestReviewWindowData } from "../../../src/lib/pull-request-review"
import { getOpenedSubWindows, openLinkedSubWindow } from "./sub-window-helpers"
import type { OpenedSubWindow } from "./sub-window-helpers"
import { sendProgress } from "./wait-helpers"

/** The GitHub repository of the review requests of the E2E repositories */
export const REVIEW_REPOSITORY = "acme/monorepo"

/** The time of the Playwright clock (`base-test.ts`), Unix seconds: a Tuesday */
export const NOW = Date.parse("2024-01-02T10:00:00Z") / 1000

const HOUR = 3600

/**
 * A pull request of carol that waits for the user's review, asked by dave `hoursAgo` hours ago. Its branch is the virtual
 * branch `branch` of the test repository (`user-name/virtual/<branch>`); the empty commits make the test server review
 * that branch from its baseline.
 */
export function reviewRequest(branch: string, number: number, hoursAgo: number, patch: Partial<ReviewRequest> = {}): ReviewRequest {
  const waitingSince = NOW - hoursAgo * HOUR
  return {
    number,
    title: `Review ${branch}`,
    url: `https://github.com/${REVIEW_REPOSITORY}/pull/${number}`,
    bodyText: `The change of ${branch}.`,
    author: "carol",
    requestedBy: "dave",
    createdAt: waitingSince,
    updatedAt: waitingSince,
    requestedAt: waitingSince,
    waitingSince,
    backToYou: null,
    additions: 12,
    deletions: 3,
    changedFiles: 2,
    checksState: null,
    reviewDecision: "reviewRequired",
    reRequested: false,
    reviewedCommit: null,
    headRefName: `user-name/virtual/${branch}`,
    headRefOid: "",
    baseRefName: "master",
    baseRefOid: "",
    isCrossRepository: false,
    baseIsDefault: true,
    ...patch,
  }
}

/** The answer of `get_review_requests` with these requests; `totalCount` beyond them when GitHub has more */
export function availableReviewRequests(requests: ReviewRequest[], totalCount = requests.length): ReviewRequestsResult {
  const [owner, name] = REVIEW_REPOSITORY.split("/") as [string, string]
  return { availability: { status: "available", owner, name }, totalCount, requests }
}

export function getYourTurnCard(page: Page): Locator {
  return page.getByTestId("your-turn-card")
}

export function getReviewRow(page: Page, number: number): Locator {
  return getYourTurnCard(page).locator(`[data-testid=review-request-row][data-review-number="${number}"]`)
}

/**
 * Clicks the row of `number`: the review window opens (`open_sub_window`, the `count`-th of the page), shown as a page
 * of its own linked to the main window (`openLinkedSubWindow`), its body focused. `beforeLoad` routes what the window
 * asks itself (its fetch).
 */
export async function openReviewWindow(
  page: Page,
  number: number,
  { count = 1, beforeLoad }: { count?: number; beforeLoad?: (window: Page) => Promise<unknown> } = {},
): Promise<{ window: Page; review: Locator; opened: OpenedSubWindow<PullRequestReviewWindowData> }> {
  await getReviewRow(page, number).getByTestId("review-request-title").click()
  const opened = await openedReviewWindow(page, count)
  expect(opened.data.number).toBe(number)
  const window = await openLinkedSubWindow(page, opened, { beforeLoad })
  const review = window.getByTestId("review-window")
  await expect(review.getByTestId("review-dialog-body")).toBeFocused()
  return { window, review, opened }
}

/** The `count`-th review window the main window opened, once it is recorded */
export async function openedReviewWindow(page: Page, count = 1): Promise<OpenedSubWindow<PullRequestReviewWindowData>> {
  await expect.poll(async () => (await getOpenedSubWindows(page)).length).toBe(count)
  const opened = (await getOpenedSubWindows<PullRequestReviewWindowData>(page))[count - 1]!
  expect(opened).toMatchObject({ windowId: "pull-request-review", url: "/pull-request-review" })
  return opened
}

/**
 * Holds `fetch_review_head` in the page: the progress messages first, then, once `release` resolves, the test server's
 * range of the repository (a failure is the `fetchError` of `setReviewRequests`); records the parameters of each call
 */
export async function routeReviewFetch(page: Page, answer: { progress: number[]; release: Promise<void> }): Promise<Record<string, unknown>[]> {
  const calls: Record<string, unknown>[] = []
  await page.route("**/invoke/fetch_review_head", async route => {
    calls.push(route.request().postDataJSON() as Record<string, unknown>)
    const channel = route.request().headers()["x-progress-channel"]
    if (channel !== undefined) {
      await sendProgress(
        page,
        channel,
        answer.progress.map(percent => ({ percent, transferred: null, speed: null, line: `Receiving objects: ${percent}%` })),
      )
    }
    await answer.release
    await route.continue()
  })
  return calls
}
