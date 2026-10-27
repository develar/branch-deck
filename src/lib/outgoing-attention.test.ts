import { describe, expect, it } from "vite-plus/test"
import type { ChecksSummary } from "@/lib/bindings"
import { collectReturnedToYou, formatReturnedMeta } from "@/lib/outgoing-attention"
import type { BranchTableRow } from "@/stores/branch-sync"
import { openPullRequest } from "@/test-support/pull-request"

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

describe("collectReturnedToYou", () => {
  it("names only the open pull requests a reviewer handed back, in the table order", () => {
    const failing: ChecksSummary = { state: "failing", passed: 1, failed: 1, pending: 0, failedNames: ["tests"] }
    const pullRequests = {
      ui: openPullRequest("ui", { reviewDecision: "changesRequested", checks: failing }),
      auth: openPullRequest("auth", { reviewDecision: "changesRequested" }),
      // failing checks are the chip's and the landing overview's
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
