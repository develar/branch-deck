// @spec spec/repository/review-requests.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { PullRequestReviewWindowData } from "@/lib/pull-request-review"
import { pullRequestReviewWindowKey } from "@/lib/pull-request-review"
import { reviewRequest } from "@/test-support/review-request"

function data(patch: Partial<PullRequestReviewWindowData>): PullRequestReviewWindowData {
  const request = reviewRequest(7)
  return {
    repositoryPath: "/repo",
    number: 7,
    request,
    index: 0,
    intent: null,
    source: "yourTurn",
    openedAt: 1,
    state: { repositoryPath: "/repo", selected: true, source: "yourTurn", requests: [request], involvements: {}, actions: {}, busy: {} },
    ...patch,
  }
}

describe("the review window of a pull request", () => {
  it("keeps its review for a plain open of the request it shows, and starts over for another request or an intent", () => {
    const shown = pullRequestReviewWindowKey(data({}))
    expect(pullRequestReviewWindowKey(data({ openedAt: 2 }))).toBe(shown)
    expect(pullRequestReviewWindowKey(data({ number: 8 }))).not.toBe(shown)
    // the same pull request opened from Reviews walks another list
    expect(pullRequestReviewWindowKey(data({ source: "reviews" }))).not.toBe(shown)
    // a write of the row's menu, D on the row: every time
    const withIntent = data({ intent: "requestChanges", openedAt: 3 })
    expect(pullRequestReviewWindowKey(withIntent)).not.toBe(shown)
    expect(pullRequestReviewWindowKey({ ...withIntent, openedAt: 4 })).not.toBe(pullRequestReviewWindowKey(withIntent))
  })
})
