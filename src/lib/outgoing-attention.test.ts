import { describe, expect, it, vi } from "vitest"
import type { PatronusRobot, PullRequest } from "@/lib/bindings"
import { collectOutgoingAttention, formatAttentionLine, formatIndicatorLabel } from "@/lib/outgoing-attention"
import type { Branch, BranchTableRow, SyncedCommit } from "@/stores/branch-sync"
import { testBranch, testRemoteStatus } from "@/stores/test-branch"

vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: async () => {} }))
vi.mock("@/lib/errors", () => ({ notifyError: () => {} }))

function row(name: string): BranchTableRow {
  return { kind: "branch", key: name, name, depth: 0, showIssueKey: false }
}

/** A branch at the commit `${name}-tip` */
function branchAtTip(name: string): Branch {
  const tip: SyncedCommit = { originalHash: `${name}-original`, hash: `${name}-tip`, strippedSubject: name, message: name, author: "Ann", authorTime: 0, committerTime: 0 }
  return testBranch(name, null, { commits: [tip] })
}

function openPullRequest(branchName: string, patch: Partial<PullRequest>): PullRequest {
  return {
    number: 7,
    title: branchName,
    url: `https://github.com/o/r/pull/7`,
    state: "open",
    isDraft: false,
    reviewDecision: null,
    headRefName: `user/virtual/${branchName}`,
    headRefOid: `${branchName}-tip`,
    checks: null,
    updatedAt: 0,
    reviewers: [],
    ...patch,
  }
}

function failedRobot(branchName: string, sourceRevision: string): PatronusRobot {
  return {
    id: `robot-${branchName}`,
    name: branchName,
    status: "failure",
    pushMode: "rebaseAutosquash",
    branchName,
    sourceRevision,
    targetBranch: "master",
    startTime: 1,
    finishTime: 2,
    cancellationReason: null,
    url: "https://patronus/robot",
  }
}

describe("collectOutgoingAttention", () => {
  it("names what each branch waits for, in the table order, a branch once", () => {
    const names = ["login", "oauth", "docs", "api", "cache"]
    const branches = Object.fromEntries(names.map(name => [name, branchAtTip(name)]))
    // the sync rebuilt cache on a newer baseline: the remote has its commits as `cache-pushed`
    branches.cache = { ...branches.cache!, remoteStatus: testRemoteStatus({ remoteTip: "cache-pushed" }) }
    const pullRequests = {
      login: openPullRequest("login", { reviewDecision: "changesRequested" }),
      oauth: openPullRequest("oauth", { checks: { state: "failing", passed: 1, failed: 1, pending: 0, failedNames: ["tests"] } }),
      docs: openPullRequest("docs", { reviewDecision: "approved" }),
    }
    const robots = {
      a: failedRobot("api", "api-tip"),
      b: failedRobot("login", "login-tip"),
      // failed on a commit the branch no longer is at: fixed since
      c: failedRobot("docs", "old"),
      d: failedRobot("cache", "cache-pushed"),
    }
    const issueRow: BranchTableRow = { kind: "issue", key: "issue:ABC-1", issueKey: "ABC-1", branchCount: 2 }
    expect(collectOutgoingAttention([issueRow, ...names.map(row)], branches, pullRequests, robots)).toEqual([
      { branchName: "login", reason: "changes requested, failed Safe Push" },
      { branchName: "oauth", reason: "failing checks" },
      { branchName: "api", reason: "failed Safe Push" },
      { branchName: "cache", reason: "failed Safe Push" },
    ])
    expect(collectOutgoingAttention(names.map(row), branches, {}, undefined)).toEqual([])
  })
})

describe("formatIndicatorLabel", () => {
  it("adds how many branches need attention to the title of the overview", () => {
    const attention = [{ branchName: "login", reason: "changes requested" }]
    expect(formatIndicatorLabel("3 commits to push in 2 branches", [])).toBe("3 commits to push in 2 branches")
    expect(formatIndicatorLabel("3 commits to push in 2 branches", attention)).toBe("3 commits to push in 2 branches, 1 needs attention")
    expect(formatIndicatorLabel("Nothing to push, 2 pull requests open", [...attention, { branchName: "api", reason: "failed Safe Push" }]))
      .toBe("Nothing to push, 2 pull requests open, 2 need attention")
    expect(formatAttentionLine(attention[0]!)).toBe("login needs attention: changes requested")
  })
})
