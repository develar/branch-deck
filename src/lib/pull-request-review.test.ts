// @spec spec/repository/review-requests.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { PullRequestReviewWindowData } from "@/lib/pull-request-review"
import { getReviewSafePush, pullRequestReviewWindowKey } from "@/lib/pull-request-review"
import { reviewRequest } from "@/test-support/review-request"

function data(patch: Partial<PullRequestReviewWindowData>): PullRequestReviewWindowData {
  const request = reviewRequest(7)
  return {
    repositoryPath: "/repo",
    number: 7,
    request,
    index: 0,
    intent: null,
    openedAt: 1,
    state: { repositoryPath: "/repo", selected: true, requests: [request], reviewed: {}, busy: {}, patronus: { available: false, target: null } },
    ...patch,
  }
}

describe("the review window of a pull request", () => {
  it("keeps its review for a plain open of the request it shows, and starts over for another request or an intent", () => {
    const shown = pullRequestReviewWindowKey(data({}))
    expect(pullRequestReviewWindowKey(data({ openedAt: 2 }))).toBe(shown)
    expect(pullRequestReviewWindowKey(data({ number: 8 }))).not.toBe(shown)
    // a write of the row's menu, D on the row: every time
    const withIntent = data({ intent: "requestChanges", openedAt: 3 })
    expect(pullRequestReviewWindowKey(withIntent)).not.toBe(shown)
    expect(pullRequestReviewWindowKey({ ...withIntent, openedAt: 4 })).not.toBe(pullRequestReviewWindowKey(withIntent))
  })

  it("says why Approve and Safe Push waits for the branch Patronus pushes into, and a read that failed as a failure", () => {
    const request = reviewRequest(7)
    expect(getReviewSafePush(request, { available: true, target: undefined })).toEqual({ blocked: "Reading the branch Patronus pushes into…" })
    // a failed read is no "the baseline is none"
    expect(getReviewSafePush(request, { available: true, target: undefined, targetError: "Patronus answered 503" })).toEqual({
      blocked: "Cannot read the branch Patronus pushes into: Patronus answered 503",
    })
    expect(getReviewSafePush(request, { available: true, target: null })).toEqual({ blocked: "Patronus pushes into a branch of origin: the baseline is none" })
    expect(getReviewSafePush(request, { available: true, target: "master" })).toEqual({ blocked: null })
  })
})
