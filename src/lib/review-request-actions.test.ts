// @spec spec/repository/review-requests.spec.md
import { describe, expect, it, vi } from "vite-plus/test"
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
  it("lists the same actions in the row menu and the review, the writes of the row opening the review", () => {
    const row = getReviewRequestActions(reviewRequest(7), context({ reviewedCommit: { id: null, reason: "" } }), "rowMenu")
    expect(labels(reviewRequestMenu(row))).toEqual([["Review"], ["Approve…", "Request Changes…"], ["Open Pull Request"], ["Copy Pull Request Link", "Copy Branch Name"]])
    // the row's items open the review: never disabled for want of a commit, and only the keys the row has
    expect([row.approve.disabled, row.requestChanges.disabled]).toEqual([false, false])
    expect([row.review.kbds, row.approve.kbds, row.openPullRequest.kbds]).toEqual([["enter"], undefined, ["o"]])

    const window = getReviewRequestActions(reviewRequest(7), context(), "window")
    expect([window.approve.label, window.approve.kbds, window.requestChanges.kbds, window.review.kbds]).toEqual(["Approve", ["a"], ["r"], ["d"]])
    expect(window.safePush).toBeNull()
    // every surface names an action by the same id
    expect([window.approve.id, row.approve.id]).toEqual(["approve", "approve"])
  })

  it("offers Approve and Safe Push where Patronus merges into the base, the Safe Push alone once approved", () => {
    const row = getReviewRequestActions(reviewRequest(7), context({ safePush: { blocked: null } }), "rowMenu")
    expect(labels(reviewRequestMenu(row))[1]).toEqual(["Approve…", "Approve and Safe Push…", "Request Changes…"])

    const blocked = getReviewRequestActions(reviewRequest(7), context({ safePush: { blocked: "#7 merges into release/253" } }), "window")
    expect([blocked.safePush?.disabled, blocked.safePush?.description, blocked.safePush?.kbds]).toEqual([true, "#7 merges into release/253", ["m"]])

    const approved = getReviewRequestActions(reviewRequest(7), context({ safePush: { blocked: null }, submitted: { event: "approve", commitId: HEAD } }), "window")
    expect([approved.safePush?.label, approved.safePush?.disabled]).toEqual(["Safe Push…", false])
  })

  it("disables the writes of the review until the commit the user saw is there, and an approval of the same commit", () => {
    const fetching = getReviewRequestActions(reviewRequest(7), context({ reviewedCommit: { id: null, reason: "Wait until #7 is fetched" } }), "window")
    expect([fetching.approve.disabled, fetching.approve.description, fetching.requestChanges.disabled]).toEqual([true, "Wait until #7 is fetched", true])

    const approved = getReviewRequestActions(reviewRequest(7), context({ submitted: { event: "approve", commitId: HEAD } }), "window")
    expect([approved.approve.label, approved.approved, approved.approve.disabled, approved.approve.description]).toEqual(["Approved", true, true, "You approved 01234567"])
    // a new head can be approved again
    const moved = getReviewRequestActions(reviewRequest(7), context({ submitted: { event: "approve", commitId: "f".repeat(40) } }), "window")
    expect(moved.approve.disabled).toBe(false)

    // the row compares with the head GitHub shows
    const row = getReviewRequestActions(reviewRequest(7, { headRefOid: HEAD }), context({ submitted: { event: "approve", commitId: HEAD } }), "rowMenu")
    expect([row.approve.label, row.approve.disabled, row.approve.description]).toEqual(["Approve…", true, "You approved 01234567"])

    const busy = getReviewRequestActions(reviewRequest(7), context({ busy: "approve" }), "window")
    expect([busy.approve.disabled, busy.requestChanges.disabled]).toEqual([true, true])
  })
})
