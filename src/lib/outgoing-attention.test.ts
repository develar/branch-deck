import { describe, expect, it } from "vite-plus/test"
import type { ChecksSummary, PullRequest } from "@/lib/bindings"
import { collectOutgoingAttention, collectReturnedToYou, formatAttentionLine, formatIndicatorLabel, formatReturnedMeta } from "@/lib/outgoing-attention"
import type { BranchTableRow } from "@/stores/branch-sync"

function row(name: string): BranchTableRow {
  return {
    kind: "branch",
    key: name,
    name,
    depth: 0,
    showIssueKey: false,
    familyIndent: 0,
    lanes: { lane: 0, forkFrom: null, up: false, down: false, through: [], alone: true },
    dimmed: false,
  }
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
    author: "me",
    ...patch,
  }
}

describe("collectOutgoingAttention", () => {
  it("names what each pull request waits for, in the table order, a branch once", () => {
    const names = ["login", "oauth", "docs", "api"]
    const pullRequests = {
      login: openPullRequest("login", { reviewDecision: "changesRequested", checks: { state: "failing", passed: 1, failed: 1, pending: 0, failedNames: ["tests"] } }),
      oauth: openPullRequest("oauth", { checks: { state: "failing", passed: 1, failed: 1, pending: 0, failedNames: ["tests"] } }),
      docs: openPullRequest("docs", { reviewDecision: "approved" }),
    }
    const issueRow: BranchTableRow = { kind: "issue", key: "issue:ABC-1", issueKey: "ABC-1", members: ["parser", "lexer"] }
    // a failed robot is the robots indicator's: told once
    expect(collectOutgoingAttention([issueRow, ...names.map(row)], pullRequests)).toEqual([
      { branchName: "login", reason: "changes requested, failing checks" },
      { branchName: "oauth", reason: "failing checks" },
    ])
    expect(collectOutgoingAttention(names.map(row), {})).toEqual([])
  })
})

describe("collectReturnedToYou", () => {
  it("names only the open pull requests a reviewer handed back, in the table order", () => {
    const failing: ChecksSummary = { state: "failing", passed: 1, failed: 1, pending: 0, failedNames: ["tests"] }
    const pullRequests = {
      ui: openPullRequest("ui", { reviewDecision: "changesRequested", checks: failing }),
      auth: openPullRequest("auth", { reviewDecision: "changesRequested" }),
      // failing checks are the chip's and the outgoing dot's
      api: openPullRequest("api", { checks: failing }),
      old: openPullRequest("old", { reviewDecision: "changesRequested", state: "closed" }),
    }
    const returned = collectReturnedToYou(["auth", "api", "ui", "old"].map(row), pullRequests)
    expect(returned.map(it => it.branchName)).toEqual(["auth", "ui"])
    expect(returned[0]!.pullRequest).toBe(pullRequests.auth)
  })

  it("names the reviewers who asked for changes", () => {
    const pullRequest = openPullRequest("auth", {
      reviewDecision: "changesRequested",
      reviewers: [
        { login: "alice", state: "changesRequested", requested: false },
        { login: "bob", state: "approved", requested: false },
        { login: "carol", state: "changesRequested", requested: false },
      ],
    })
    expect(formatReturnedMeta(pullRequest)).toBe(`#${pullRequest.number} · changes requested by alice, carol`)
    expect(formatReturnedMeta({ ...pullRequest, reviewers: [] })).toBe(`#${pullRequest.number} · changes requested`)
  })
})

describe("formatIndicatorLabel", () => {
  it("adds how many branches need attention to the title of the overview", () => {
    const attention = [{ branchName: "login", reason: "changes requested" }]
    expect(formatIndicatorLabel("3 commits to push in 2 branches", [])).toBe("3 commits to push in 2 branches")
    expect(formatIndicatorLabel("3 commits to push in 2 branches", attention)).toBe("3 commits to push in 2 branches, 1 needs attention")
    expect(formatIndicatorLabel("Nothing to push, 2 pull requests open", [...attention, { branchName: "api", reason: "failing checks" }])).toBe(
      "Nothing to push, 2 pull requests open, 2 need attention",
    )
    expect(formatAttentionLine(attention[0]!)).toBe("login needs attention: changes requested")
  })
})
