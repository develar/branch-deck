// @spec spec/repository/review-requests.spec.md
import { describe, expect, it, vi } from "vite-plus/test"
import type { ActionBlocker, ActionId, ActionState } from "@/lib/bindings"
import type { ReviewRequestActionContext } from "@/lib/review-request-actions"
import { getReviewRequestActions, reviewRequestMenu } from "@/lib/review-request-actions"
import { reviewRequest } from "@/test-support/review-request"

vi.mock("@/lib/clipboard", () => ({ copyText: async () => {} }))

const HEAD = "0123456789abcdef0123456789abcdef01234567"

/** The actions of a pull request as Rust decides them: each id enabled, or disabled with its blocker */
function rust(...actions: (ActionId | [ActionId, ActionBlocker])[]): ActionState[] {
  return actions.map(action =>
    typeof action === "string" ? { id: action, availability: { kind: "enabled" } } : { id: action[0], availability: { kind: "disabled", reason: action[1] } },
  )
}

const OFFERED = rust("review", "approve", "request-changes", "open-pull-request")

function context(patch: Partial<ReviewRequestActionContext> = {}): ReviewRequestActionContext {
  return {
    reviewedCommit: { id: HEAD },
    actions: OFFERED,
    busy: undefined,
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

  it("offers Approve and Safe Push where Rust offers it, disabled with its reason, the Safe Push alone once approved", () => {
    const row = getReviewRequestActions(reviewRequest(7), context({ actions: [...OFFERED, ...rust("safe-push-pull-request")] }), "rowMenu")
    expect(labels(reviewRequestMenu(row))[1]).toEqual(["Approve…", "Approve and Safe Push…", "Request Changes…"])

    const otherBase: ActionBlocker = { code: "otherBase", number: 7, base: "release/253", target: "master" }
    const blocked = getReviewRequestActions(reviewRequest(7), context({ actions: [...OFFERED, ...rust(["safe-push-pull-request", otherBase])] }), "window")
    expect([blocked.safePush?.disabled, blocked.safePush?.description, blocked.safePush?.kbds]).toEqual([
      true,
      "#7 merges into release/253: Patronus would merge it there, not into master",
      ["m"],
    ])

    const approved = getReviewRequestActions(
      reviewRequest(7),
      context({ actions: rust("review", ["approve", { code: "approved", commit: HEAD }], "safe-push-pull-request", "request-changes", "open-pull-request") }),
      "window",
    )
    expect([approved.safePush?.label, approved.safePush?.disabled]).toEqual(["Safe Push…", false])
  })

  it("disables the writes of the review until the commit the user saw is there, an approval of the same commit, and an own pull request", () => {
    const fetching = getReviewRequestActions(reviewRequest(7), context({ reviewedCommit: { id: null, reason: "Wait until #7 is fetched" } }), "window")
    expect([fetching.approve.disabled, fetching.approve.description, fetching.requestChanges.disabled]).toEqual([true, "Wait until #7 is fetched", true])

    const approvedHead = (commit: string) => context({ actions: rust("review", ["approve", { code: "approved", commit }], "request-changes", "open-pull-request") })
    const approved = getReviewRequestActions(reviewRequest(7), approvedHead(HEAD), "window")
    expect([approved.approve.label, approved.approved, approved.approve.disabled, approved.approve.description]).toEqual(["Approved", true, true, "You approved 01234567"])
    // the window shows another commit than the one approved: it can be approved
    const moved = getReviewRequestActions(reviewRequest(7), approvedHead("f".repeat(40)), "window")
    expect(moved.approve.disabled).toBe(false)

    // the row compares with the head GitHub shows
    const row = getReviewRequestActions(reviewRequest(7, { headRefOid: HEAD }), approvedHead(HEAD), "rowMenu")
    expect([row.approve.label, row.approve.disabled, row.approve.description]).toEqual(["Approve…", true, "You approved 01234567"])

    const busy = getReviewRequestActions(reviewRequest(7), context({ busy: "approve" }), "window")
    expect([busy.approve.disabled, busy.requestChanges.disabled]).toEqual([true, true])

    const own: ActionBlocker = { code: "ownPullRequest" }
    const mine = getReviewRequestActions(reviewRequest(7), context({ actions: rust("review", ["approve", own], ["request-changes", own], "open-pull-request") }), "rowMenu")
    expect([mine.approve.disabled, mine.approve.description, mine.requestChanges.disabled]).toEqual([true, "Your own pull request: GitHub takes no review of its author", true])
  })

  it("offers Show Branch of an own pull request and Remove from Reviews of a link where Rust offers them", () => {
    const actions = [...OFFERED, ...rust("show-branch", "remove-from-reviews")]
    const row = getReviewRequestActions(reviewRequest(7), context({ actions, onShowBranch: () => {}, onRemoveFromReviews: () => {} }), "rowMenu")
    expect(labels(reviewRequestMenu(row))).toEqual([
      ["Review"],
      ["Approve…", "Request Changes…"],
      ["Open Pull Request", "Show Branch"],
      ["Copy Pull Request Link", "Copy Branch Name"],
      ["Remove from Reviews"],
    ])
    // a surface without them offers none
    expect(getReviewRequestActions(reviewRequest(7), context({ actions }), "rowMenu").showBranch).toBeNull()
  })
})
