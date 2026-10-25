// @spec spec/repository/review-requests.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { ApprovalNews } from "@/lib/approval-news"
import { approvalNewsOf, describeApprovalNews } from "@/lib/approval-news"
import type { News, Person } from "@/lib/bindings"
import { stackReadinessKey } from "@/lib/whole-stack"

const ilya: Person = { login: "ikorennoy", fullName: "Ilya Korennoy" }
const anna: Person = { login: "aivanova", fullName: "Anna Ivanova" }

function news(patch: Partial<ApprovalNews> = {}): ApprovalNews {
  return { branchName: "wsm-skip-wa-2", pullRequest: 1078, approvers: [ilya], groups: ["IJPL Code Platform"], ...patch }
}

describe("approval news", () => {
  it("takes the approvals of the session's news, each with its subject, and none of the robots'", () => {
    const told: News[] = [
      { id: 1, event: { kind: "robotMerged", robotId: "r1" } },
      { id: 2, event: { kind: "approvalsArrived", subject: "auth", pullRequest: 7, approvers: [ilya], groups: ["IJPL Code Platform"] } },
    ]
    expect(approvalNewsOf(told)).toEqual([{ branchName: "auth", pullRequest: 7, approvers: [ilya], groups: ["IJPL Code Platform"] }])
  })

  it("tells the approvals that made a waiting pull request ready, for the same tip", () => {
    expect(describeApprovalNews(news())).toEqual({ title: "Ilya Korennoy approved wsm-skip-wa-2", description: "#1078 · IJPL Code Platform" })
    // GitHub's wait while Patronus can't judge the approvals: the login, no group
    const waited = news({ approvers: [{ login: "ikorennoy", fullName: null }], groups: [] })
    expect(describeApprovalNews(waited)).toEqual({ title: "ikorennoy approved wsm-skip-wa-2", description: "#1078" })
  })

  it("names every approver of the groups that waited, each once; a group approved before isn't news", () => {
    const told = news({ branchName: "auth", approvers: [ilya, anna], groups: ["IJPL Code Platform", "Air Platform"] })
    expect(describeApprovalNews(told)).toEqual({ title: "Ilya Korennoy and Anna Ivanova approved auth", description: "#1078 · IJPL Code Platform and Air Platform" })
  })

  it("says the code owners approved when Patronus needs no approval any more and names nobody", () => {
    expect(describeApprovalNews(news({ branchName: "auth", approvers: [] })).title).toBe("The code owners approved auth")
  })

  it("names the whole stack for the approvals of its pull request", () => {
    expect(describeApprovalNews(news({ branchName: stackReadinessKey("air") }))).toEqual({
      title: "Ilya Korennoy approved the whole air stack",
      description: "#1078 · IJPL Code Platform",
    })
  })
})
