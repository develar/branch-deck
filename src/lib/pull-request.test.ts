import { describe, expect, it, vi } from "vitest"
import type { ChecksSummary, PullRequest } from "@/lib/bindings"
import { formatAuthorAttention, formatAvailabilityHint, formatChecks, getPullRequestChipState } from "@/lib/pull-request"

vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: async () => {} }))
vi.mock("@/lib/errors", () => ({ notifyError: () => {} }))

function pullRequest(patch: Partial<PullRequest> = {}): PullRequest {
  return {
    number: 760,
    title: "Add login flow",
    url: "https://github.com/o/r/pull/760",
    state: "open",
    isDraft: false,
    reviewDecision: "reviewRequired",
    headRefName: "user/virtual/auth",
    headRefOid: "tip",
    checks: null,
    updatedAt: 0,
    reviewers: [],
    ...patch,
  }
}

const failing: ChecksSummary = { state: "failing", passed: 9, failed: 1, pending: 2, failedNames: ["tests"] }

describe("getPullRequestChipState", () => {
  it("colours an open pull request by its review decision", () => {
    expect(getPullRequestChipState(pullRequest({ reviewDecision: "approved" }), "tip")).toEqual({ icon: "open", color: "success", label: "Approved", checksFailing: false, outdated: false })
    expect(getPullRequestChipState(pullRequest({ reviewDecision: "changesRequested" }), "tip")).toMatchObject({ icon: "open", color: "error", label: "Changes requested" })
    expect(getPullRequestChipState(pullRequest(), "tip")).toMatchObject({ icon: "open", color: "neutral", label: "Review required" })
    expect(getPullRequestChipState(pullRequest({ reviewDecision: null }), "tip")).toMatchObject({ icon: "open", color: "neutral", label: "Open" })
  })

  it("shows the state of a draft, a merged and a closed pull request instead of the decision", () => {
    expect(getPullRequestChipState(pullRequest({ isDraft: true, reviewDecision: "approved" }), "tip")).toMatchObject({ icon: "draft", color: "neutral", label: "Draft" })
    expect(getPullRequestChipState(pullRequest({ state: "merged", reviewDecision: "approved" }), "tip")).toMatchObject({ icon: "merged", color: "info", label: "Merged" })
    expect(getPullRequestChipState(pullRequest({ state: "closed" }), "tip")).toMatchObject({ icon: "closed", color: "neutral", label: "Closed" })
  })

  it("marks failing checks and a head that is not the local tip, for open pull requests only", () => {
    expect(getPullRequestChipState(pullRequest({ checks: failing }), "other")).toMatchObject({ checksFailing: true, outdated: true })
    expect(getPullRequestChipState(pullRequest({ checks: { ...failing, state: "pending" } }), undefined)).toMatchObject({ checksFailing: false, outdated: false })
    // a merged pull request keeps its last head
    expect(getPullRequestChipState(pullRequest({ state: "merged", checks: failing }), "other")).toMatchObject({ checksFailing: false, outdated: false })
  })
})

describe("formatAuthorAttention", () => {
  it("names the requested changes and the failing checks of an open pull request", () => {
    expect(formatAuthorAttention(pullRequest({ reviewDecision: "changesRequested" }))).toBe("changes requested")
    expect(formatAuthorAttention(pullRequest({ checks: failing }))).toBe("failing checks")
    expect(formatAuthorAttention(pullRequest({ reviewDecision: "changesRequested", checks: failing }))).toBe("changes requested, failing checks")
    expect(formatAuthorAttention(pullRequest({ reviewDecision: "approved" }))).toBeNull()
    expect(formatAuthorAttention(pullRequest({ state: "closed", reviewDecision: "changesRequested" }))).toBeNull()
  })
})

describe("formatChecks", () => {
  it("lists the failing checks first", () => {
    expect(formatChecks(failing)).toBe("1 failing (tests), 2 running, 9 passed")
    expect(formatChecks({ state: "failing", passed: 0, failed: 5, pending: 0, failedNames: ["a", "b", "c"] })).toBe("5 failing (a, b, c, …)")
    expect(formatChecks({ state: "passing", passed: 4, failed: 0, pending: 0, failedNames: [] })).toBe("All 4 passed")
    expect(formatChecks({ state: "passing", passed: 1, failed: 0, pending: 0, failedNames: [] })).toBe("Passed")
  })
})

describe("formatAvailabilityHint", () => {
  it("hints at what to do, and says nothing when there is nothing to do", () => {
    expect(formatAvailabilityHint({ status: "ghMissing" })).toBe("Install the GitHub CLI (gh) to see pull requests")
    expect(formatAvailabilityHint({ status: "notAuthenticated" })).toBe("Run gh auth login to see pull requests")
    expect(formatAvailabilityHint({ status: "error", message: "error connecting to api.github.com" })).toBe("GitHub: error connecting to api.github.com")
    expect(formatAvailabilityHint({ status: "available", owner: "o", name: "r" })).toBeNull()
    expect(formatAvailabilityHint({ status: "notGitHub" })).toBeNull()
    expect(formatAvailabilityHint(null)).toBeNull()
  })
})
