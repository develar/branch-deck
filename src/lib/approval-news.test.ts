// @spec spec/repository/review-requests.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { ReadinessSnapshot } from "@/lib/approval-news"
import { approvalNewsKey, describeApprovalNews, findApprovalNews } from "@/lib/approval-news"
import type { MissingReviewGroup, Person, ReviewReadiness } from "@/lib/bindings"
import { stackReadinessKey } from "@/lib/whole-stack"

const ilya: Person = { login: "ikorennoy", fullName: "Ilya Korennoy" }
const anna: Person = { login: "aivanova", fullName: "Anna Ivanova" }

function group(name: string): MissingReviewGroup {
  return { name, needed: 1, approvedBy: [], requested: [ilya], suggested: [ilya], changesRequestedBy: [] }
}

const waiting: ReviewReadiness = { kind: "missing", groups: [group("IJPL Code Platform")], notCounted: [], reason: "approvalsMissing" }
const approved: ReviewReadiness = { kind: "approved", groups: [{ name: "IJPL Code Platform", approvedBy: [ilya] }] }

let reviews = 0

/** A snapshot of another input each time: new reviews, unless `key` says otherwise */
function snapshot(readiness: ReviewReadiness, patch: Partial<ReadinessSnapshot> = {}): ReadinessSnapshot {
  return { key: `reviews ${++reviews}`, tip: "t1", pullRequest: 1078, readiness, ...patch }
}

describe("findApprovalNews", () => {
  it("tells the approvals that made a waiting pull request ready, for the same tip", () => {
    const news = findApprovalNews("wsm-skip-wa-2", snapshot(waiting), snapshot(approved))
    expect(news).toEqual({ branchName: "wsm-skip-wa-2", pullRequest: 1078, approvers: [ilya], groups: ["IJPL Code Platform"] })
    expect(describeApprovalNews(news!)).toEqual({ title: "Ilya Korennoy approved wsm-skip-wa-2", description: "#1078 · IJPL Code Platform" })
  })

  it("tells the approval of a reviewer GitHub waited for while Patronus can't judge the approvals", () => {
    const unknown: ReviewReadiness = { kind: "unknown", reason: "approvalsNotEvaluated" }
    const before = snapshot(unknown, { waitingFor: ["ikorennoy"], approvedBy: ["junie-jetbrains"] })
    const news = findApprovalNews("wsm-skip-wa-2", before, snapshot(unknown, { waitingFor: [], approvedBy: ["junie-jetbrains", "ikorennoy"] }))
    expect(news).toEqual({ branchName: "wsm-skip-wa-2", pullRequest: 1078, approvers: [{ login: "ikorennoy", fullName: null }], groups: [] })
    expect(describeApprovalNews(news!)).toEqual({ title: "ikorennoy approved wsm-skip-wa-2", description: "#1078" })
    // still waiting for someone else, or nobody approved (the request was withdrawn): no news
    expect(
      findApprovalNews("wsm-skip-wa-2", snapshot(unknown, { waitingFor: ["ikorennoy", "alice"] }), snapshot(unknown, { waitingFor: ["alice"], approvedBy: ["ikorennoy"] })),
    ).toBeNull()
    expect(findApprovalNews("wsm-skip-wa-2", before, snapshot(unknown, { waitingFor: [], approvedBy: ["junie-jetbrains"] }))).toBeNull()
  })

  it("names every approver of the groups that waited, each once; a group approved before isn't news", () => {
    const before: ReviewReadiness = { ...waiting, groups: [group("IJPL Code Platform"), group("Air Platform")] }
    const after: ReviewReadiness = {
      kind: "approved",
      groups: [
        { name: "IJPL Code Platform", approvedBy: [ilya] },
        { name: "Air Platform", approvedBy: [anna, ilya] },
        { name: "Docs", approvedBy: [{ login: "bob", fullName: "Bob" }] },
      ],
    }
    const news = findApprovalNews("auth", snapshot(before), snapshot(after))!
    expect(describeApprovalNews(news)).toEqual({ title: "Ilya Korennoy and Anna Ivanova approved auth", description: "#1078 · IJPL Code Platform and Air Platform" })
  })

  it("says the code owners approved when Patronus needs no approval any more and names nobody", () => {
    const news = findApprovalNews("auth", snapshot(waiting), snapshot({ kind: "notRequired" }))!
    expect(describeApprovalNews(news).title).toBe("The code owners approved auth")
  })

  it("names the whole stack for the approvals of its pull request", () => {
    const news = findApprovalNews(stackReadinessKey("air"), snapshot(waiting), snapshot(approved))!
    expect(describeApprovalNews(news)).toEqual({ title: "Ilya Korennoy approved the whole air stack", description: "#1078 · IJPL Code Platform" })
  })

  it("is no news for the first answer, another tip, a start through the API or a readiness still missing", () => {
    expect(findApprovalNews("auth", undefined, snapshot(approved))).toBeNull()
    // a new commit of the user, not a review
    expect(findApprovalNews("auth", snapshot(waiting), snapshot(approved, { tip: "t2" }))).toBeNull()
    expect(findApprovalNews("auth", snapshot(waiting, { pullRequest: null }), snapshot(approved, { pullRequest: null }))).toBeNull()
    expect(findApprovalNews("auth", snapshot(waiting, { pullRequest: 7 }), snapshot(approved))).toBeNull()
    expect(findApprovalNews("auth", snapshot(waiting), snapshot(waiting))).toBeNull()
    // a recheck of the same reviews: the kept answer was older, no review is new
    expect(findApprovalNews("auth", snapshot(waiting, { key: "same" }), snapshot(approved, { key: "same" }))).toBeNull()
    expect(findApprovalNews("auth", snapshot({ kind: "unknown", reason: "approvalsNotEvaluated" }), snapshot(approved))).toBeNull()
  })

  it("keys an approval by the branch, the pull request and the approvers, whatever their order", () => {
    const a = { branchName: "auth", pullRequest: 7, approvers: [ilya, anna], groups: [] }
    const b = { branchName: "auth", pullRequest: 7, approvers: [anna, ilya], groups: ["x"] }
    expect(approvalNewsKey(a)).toBe(approvalNewsKey(b))
    expect(approvalNewsKey(a)).not.toBe(approvalNewsKey({ ...a, pullRequest: 8 }))
  })
})
