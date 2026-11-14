// @spec openspec/specs/repository/review-requests/spec.md
import { describe, expect, it, vi } from "vite-plus/test"
import type { ActionBlocker, ActionId, ActionState, ReviewStackApproval } from "@/lib/bindings"
import type { ReviewRequestActionContext } from "@/lib/review-request-actions"
import { getReviewRequestActions, getStackApprovalItem, reviewRequestMenu } from "@/lib/review-request-actions"
import { REVIEWED_COMMIT as HEAD, reviewRequest, reviewRequestActionContext } from "@/test-support/review-request"

vi.mock("@/lib/clipboard", () => ({ copyText: async () => {} }))

/** The actions of a pull request as Rust decides them: each id enabled, or disabled with its blocker */
function rust(...actions: (ActionId | [ActionId, ActionBlocker])[]): ActionState[] {
  return actions.map(action =>
    typeof action === "string" ? { id: action, availability: { kind: "enabled" } } : { id: action[0], availability: { kind: "disabled", reason: action[1] } },
  )
}

const OFFERED = rust("review", "approve", "request-changes", "open-pull-request")

function context(patch: Partial<ReviewRequestActionContext> = {}): ReviewRequestActionContext {
  return reviewRequestActionContext(OFFERED, patch)
}

function labels(groups: ReturnType<typeof reviewRequestMenu>): string[][] {
  return groups.map(group => group.map(item => item.label))
}

describe("getReviewRequestActions", () => {
  it("lists the same actions in the row menu and the review, the writes of the row opening the review", () => {
    const row = getReviewRequestActions(reviewRequest(7), context({ reviewedCommit: { id: null, reason: "" } }), "rowMenu")
    expect(labels(reviewRequestMenu(row))).toEqual([["Review"], ["Approve…", "Request Changes…"], ["Open Pull Request"], ["Copy Pull Request Link", "Copy Branch Name"]])
    // the row's items open the review: never disabled for want of a commit, and only the keys the row has
    expect([row.approve?.disabled, row.requestChanges?.disabled]).toEqual([false, false])
    expect([row.review.kbds, row.approve?.kbds, row.openPullRequest.kbds]).toEqual([["enter"], undefined, ["o"]])

    const window = getReviewRequestActions(reviewRequest(7), context(), "window")
    expect([window.approve?.label, window.approve?.kbds, window.requestChanges?.kbds, window.review.kbds]).toEqual(["Approve", ["a"], ["r"], ["d"]])
    expect(window.safePush).toBeNull()
    // every surface names an action by the same id
    expect([window.approve?.id, row.approve?.id]).toEqual(["approve", "approve"])
  })

  it("offers Approve and Safe Push where Rust offers it, disabled with its reason, the Safe Push alone once approved", () => {
    const row = getReviewRequestActions(reviewRequest(7), context({ actions: [...OFFERED, ...rust("safe-push-pull-request")] }), "rowMenu")
    expect(labels(reviewRequestMenu(row))[1]).toEqual(["Approve…", "Approve and Safe Push…", "Request Changes…"])

    const otherBase: ActionBlocker = { code: "otherBase", number: 7, base: "release/253", target: "master" }
    const blocked = getReviewRequestActions(reviewRequest(7), context({ actions: [...OFFERED, ...rust(["safe-push-pull-request", otherBase])] }), "window")
    expect([blocked.safePush?.disabled, blocked.safePush?.description, blocked.safePush?.kbds]).toEqual([
      true,
      "Patronus starts a Safe Push only from a pull request into master: #7 goes into release/253",
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
    expect([fetching.approve?.disabled, fetching.approve?.description, fetching.requestChanges?.disabled]).toEqual([true, "Wait until #7 is fetched", true])

    const approvedHead = (commit: string) => context({ actions: rust("review", ["approve", { code: "approved", commit }], "request-changes", "open-pull-request") })
    const approved = getReviewRequestActions(reviewRequest(7), approvedHead(HEAD), "window")
    expect([approved.approve?.label, approved.approved, approved.approve?.disabled, approved.approve?.description]).toEqual(["Approved", true, true, "You approved 01234567"])
    // the window shows another commit than the one approved: it can be approved
    const moved = getReviewRequestActions(reviewRequest(7), approvedHead("f".repeat(40)), "window")
    expect(moved.approve?.disabled).toBe(false)

    // the row compares with the head GitHub shows
    const row = getReviewRequestActions(reviewRequest(7, { headRefOid: HEAD }), approvedHead(HEAD), "rowMenu")
    expect([row.approve?.label, row.approve?.disabled, row.approve?.description]).toEqual(["Approve…", true, "You approved 01234567"])

    const busy = getReviewRequestActions(reviewRequest(7), context({ busy: "approve" }), "window")
    expect([busy.approve?.disabled, busy.requestChanges?.disabled]).toEqual([true, true])

    const own: ActionBlocker = { code: "ownPullRequest" }
    const mine = getReviewRequestActions(reviewRequest(7), context({ actions: rust("review", ["approve", own], ["request-changes", own], "open-pull-request") }), "rowMenu")
    expect([mine.approve?.disabled, mine.approve?.description, mine.requestChanges?.disabled]).toEqual([true, "Your own pull request: GitHub takes no review of its author", true])
  })

  it("says in Approve what it counts for after the reason it waits, never disabling it, and nothing once approved or in the row menu", () => {
    const scope = "Counts for Design · You opened 1 of its 2 files"
    const window = getReviewRequestActions(reviewRequest(7), context({ approvalScope: scope }), "window")
    expect([window.approve?.description, window.approve?.disabled, window.approvalScope]).toEqual([scope, false, scope])
    const busy = getReviewRequestActions(reviewRequest(7), context({ approvalScope: scope, busy: "requestChanges" }), "window")
    expect(busy.approve?.description).toBe(`Wait for GitHub to answer. ${scope}`)
    const row = getReviewRequestActions(reviewRequest(7), context({ approvalScope: scope }), "rowMenu")
    expect([row.approve?.description, row.approvalScope]).toEqual([undefined, null])
    const approved = getReviewRequestActions(reviewRequest(7), context({ approvalScope: scope, actions: rust(["approve", { code: "approved", commit: HEAD }]) }), "window")
    expect([approved.approved, approved.approve?.description, approved.approvalScope]).toEqual([true, "You approved 01234567", null])
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

describe("the decisions of All Changes of a stack", () => {
  it("offers no decision of one pull request where Rust offers the actions of the stack alone", () => {
    const stack = getReviewRequestActions(reviewRequest(7), context({ actions: rust("approve-stack") }), "window")
    expect([stack.approve, stack.requestChanges, stack.safePush]).toEqual([null, null, null])
    // while the session has no actions of the pull request (another repository) they are offered
    const unknown = getReviewRequestActions(reviewRequest(7), context({ actions: undefined }), "window")
    expect([unknown.approve?.id, unknown.requestChanges?.id]).toEqual(["approve", "request-changes"])
  })
})

describe("getStackApprovalItem", () => {
  const words = { operations: {}, named: {} }
  const approval = (approves: number[], approved: number[]): ReviewStackApproval => ({ approves: approves.map(number => ({ number, commit: `head-${number}` })), approved })
  const action = (reason: ActionBlocker | null = null): ActionState => ({ id: "approve-stack", availability: reason === null ? { kind: "enabled" } : { kind: "disabled", reason } })
  const item = (patch: Partial<Parameters<typeof getStackApprovalItem>[0]>) =>
    getStackApprovalItem({
      approval: approval([301, 303], [302]),
      action: action(),
      layerCount: 3,
      waiting: null,
      busy: false,
      words,
      landing: null,
      onApprove: () => {},
      ...patch,
    })

  it("counts the layers Rust approves on the heads the window shows, names the ones it skips, and waits for the heads and a write", () => {
    expect(item({})).toMatchObject({
      label: "Approve 2 Pull Requests",
      kbds: ["shift", "a"],
      disabled: false,
      description: "Skips #302: you already approved its commit",
    })
    expect(item({ approval: approval([303], []) })).toMatchObject({ label: "Approve 1 Pull Request", disabled: false, description: undefined })
    expect(item({ landing: "Patronus counts #301 for its own Safe Push, #302 and #303 once each goes into master" }).description).toBe(
      "Skips #302: you already approved its commit. Patronus counts #301 for its own Safe Push, #302 and #303 once each goes into master",
    )
    expect(item({ approval: undefined })).toMatchObject({
      label: "Approve 3 Pull Requests",
      disabled: true,
      description: "Wait until the stack is fetched: an approval is on the commits you saw",
    })
    expect(item({ waiting: "The commit #302 shows could not be fetched" })).toMatchObject({ disabled: true, description: "The commit #302 shows could not be fetched" })
    expect(item({ busy: true })).toMatchObject({ disabled: true, description: "Wait for GitHub to answer" })
    expect(item({ approval: approval([], [301, 302, 303]), action: action({ code: "stackApproved" }) })).toMatchObject({
      label: "Approve 3 Pull Requests",
      disabled: true,
      description: "You approved every pull request of the stack on the commits it shows",
    })
  })
})
