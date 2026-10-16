// @spec spec/repository/review-requests.spec.md
import { describe, expect, it, vi } from "vitest"
import type { ReviewRequestActionContext } from "@/lib/review-request-actions"
import { getReviewRequestActions, reviewRequestMenu } from "@/lib/review-request-actions"
import { reviewRequest } from "@/test-support/review-request"

vi.mock("@/lib/clipboard", () => ({ copyText: async () => {} }))

const HEAD = "0123456789abcdef0123456789abcdef01234567"

function context(patch: Partial<ReviewRequestActionContext> = {}): ReviewRequestActionContext {
  return {
    reviewedCommit: { id: HEAD },
    submitted: undefined,
    busy: undefined,
    safePush: null,
    onReview: () => {},
    onApprove: () => {},
    onSafePush: () => {},
    onRequestChanges: () => {},
    onOpenPullRequest: () => {},
    ...patch,
  }
}

function labels(groups: ReturnType<typeof reviewRequestMenu>): string[][] {
  return groups.map(group => group.map(item => item.label))
}

describe("getReviewRequestActions", () => {
  it("lists the same actions in the row menu and the dialog, the writes of the row opening the dialog", () => {
    const row = getReviewRequestActions(reviewRequest(7), context({ reviewedCommit: { id: null, reason: "" } }), "rowMenu")
    expect(labels(reviewRequestMenu(row, "rowMenu"))).toEqual([
      ["Review"],
      ["Approve…", "Request Changes…"],
      ["Open Pull Request"],
      ["Copy Pull Request Link", "Copy Branch Name"],
    ])
    // the row's items open the dialog: never disabled for want of a commit, and only the keys the row has
    expect([row.approve.disabled, row.requestChanges.disabled]).toEqual([false, false])
    expect([row.review.kbds, row.approve.kbds, row.openPullRequest.kbds]).toEqual([["enter"], undefined, ["o"]])

    const dialog = getReviewRequestActions(reviewRequest(7), context(), "dialog")
    expect([dialog.approve.label, dialog.approve.kbds, dialog.requestChanges.kbds]).toEqual(["Approve", ["a"], ["r"]])
  })

  it("disables the writes of the dialog until the commit the user saw is there, and an approval of the same commit", () => {
    const fetching = getReviewRequestActions(reviewRequest(7), context({ reviewedCommit: { id: null, reason: "Wait until #7 is fetched" } }), "dialog")
    expect([fetching.approve.disabled, fetching.approve.description, fetching.requestChanges.disabled]).toEqual([true, "Wait until #7 is fetched", true])

    const approved = getReviewRequestActions(reviewRequest(7), context({ submitted: { event: "approve", commitId: HEAD } }), "dialog")
    expect([approved.approve.disabled, approved.approve.description]).toEqual([true, "You approved 01234567"])
    // a new head can be approved again
    const moved = getReviewRequestActions(reviewRequest(7), context({ submitted: { event: "approve", commitId: "f".repeat(40) } }), "dialog")
    expect(moved.approve.disabled).toBe(false)

    // the row compares with the head GitHub shows
    const row = getReviewRequestActions(reviewRequest(7, { headRefOid: HEAD }), context({ submitted: { event: "approve", commitId: HEAD } }), "rowMenu")
    expect([row.approve.disabled, row.approve.description]).toEqual([true, "You approved 01234567"])

    const busy = getReviewRequestActions(reviewRequest(7), context({ busy: "approve" }), "dialog")
    expect([busy.approve.disabled, busy.requestChanges.disabled]).toEqual([true, true])
  })
})
