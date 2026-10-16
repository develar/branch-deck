import { describe, expect, it, vi } from "vite-plus/test"
import type { ChecksSummary, PullRequest } from "@/lib/bindings"
import { formatAuthorAttention, formatAvailabilityHint, formatChecks, formatPullRequestCommitNotice, getAvailabilityAction, getCompareUrl, getPullRequestChipState } from "@/lib/pull-request"
import type { RemoteStatus } from "@/stores/branch-sync"
import { testRemoteStatus } from "@/stores/test-branch"

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
/** The remote branch is at the head ("tip"), the local branch has a commit more */
const unpushed: RemoteStatus = testRemoteStatus({ remoteTip: "tip", unpushedCommits: ["fix"], commitsAhead: 1, myCommitsAhead: 1 })

describe("getPullRequestChipState", () => {
  it("colours an open pull request by its review decision", () => {
    expect(getPullRequestChipState(pullRequest({ reviewDecision: "approved" }), "tip", null)).toEqual({ icon: "open", color: "success", label: "Approved", checksFailing: false, commit: "current", outdated: false })
    expect(getPullRequestChipState(pullRequest({ reviewDecision: "changesRequested" }), "tip", null)).toMatchObject({ icon: "open", color: "error", label: "Changes requested" })
    expect(getPullRequestChipState(pullRequest(), "tip", null)).toMatchObject({ icon: "open", color: "neutral", label: "Review required" })
    expect(getPullRequestChipState(pullRequest({ reviewDecision: null }), "tip", null)).toMatchObject({ icon: "open", color: "neutral", label: "Open" })
  })

  it("shows the state of a draft, a merged and a closed pull request instead of the decision", () => {
    expect(getPullRequestChipState(pullRequest({ isDraft: true, reviewDecision: "approved" }), "tip", null)).toMatchObject({ icon: "draft", color: "neutral", label: "Draft" })
    expect(getPullRequestChipState(pullRequest({ state: "merged", reviewDecision: "approved" }), "tip", null)).toMatchObject({ icon: "merged", color: "info", label: "Merged" })
    expect(getPullRequestChipState(pullRequest({ state: "closed" }), "tip", null)).toMatchObject({ icon: "closed", color: "neutral", label: "Closed" })
  })

  it("marks failing checks and a head the branch changed since, for open pull requests only", () => {
    expect(getPullRequestChipState(pullRequest({ checks: failing }), "fixed", unpushed)).toMatchObject({ checksFailing: true, commit: "changedLocally", outdated: true })
    expect(getPullRequestChipState(pullRequest({ checks: { ...failing, state: "pending" } }), undefined, null)).toMatchObject({ checksFailing: false, outdated: false })
    // the sync rebuilt the branch on a newer baseline: new hashes, the commits the pull request shows
    expect(getPullRequestChipState(pullRequest(), "rebuilt", testRemoteStatus({ remoteTip: "tip" }))).toMatchObject({ commit: "rebased", outdated: false })
    // a merged pull request keeps its last head
    expect(getPullRequestChipState(pullRequest({ state: "merged", checks: failing }), "fixed", unpushed)).toMatchObject({ checksFailing: false, commit: "current", outdated: false })
  })
})

describe("formatPullRequestCommitNotice", () => {
  it("asks for a push when the branch changed, waits for GitHub after one, and says quietly that it shows an earlier build", () => {
    const shown = pullRequest({ headRefOid: "3264f30f0123" })
    const notice = (localTip: string, remote: RemoteStatus) => formatPullRequestCommitNotice(shown, getPullRequestChipState(shown, localTip, remote), remote.remoteTip)
    const ahead = testRemoteStatus({ remoteTip: "3264f30f0123", unpushedCommits: ["fix"], commitsAhead: 1, myCommitsAhead: 1 })
    expect(notice("88de421d4567", ahead)).toBe("Shows 3264f30f; the branch has changed since: push to update the pull request.")
    expect(notice("88de421d4567", testRemoteStatus({ remoteTip: "88de421d4567" }))).toBe("Shows 3264f30f; GitHub doesn't show the pushed 88de421d yet.")
    expect(notice("88de421d4567", testRemoteStatus({ remoteTip: "3264f30f0123" }))).toBe("Shows an earlier build of the same commits.")
    expect(notice("3264f30f0123", ahead)).toBeNull()
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
    expect(formatAvailabilityHint({ status: "signedOut" })).toBe("No GitHub token to see pull requests")
    expect(formatAvailabilityHint({ status: "notAuthenticated", source: "gh" })).toBe("GitHub rejected the token from gh")
    expect(formatAvailabilityHint({ status: "noAccess", source: "gitCredential", message: "Not Found" })).toBe("GitHub: Not Found (the token from the git credentials)")
    expect(formatAvailabilityHint({ status: "noAccess", source: "branchDeck", message: "no repository o/r" })).toBe(
      "GitHub: no repository o/r (the token from the Branch Deck sign-in). An organization that restricts OAuth apps shows its private repositories only after an admin approves Branch Deck: request it, or run gh auth login",
    )
    expect(formatAvailabilityHint({ status: "error", message: "error connecting to api.github.com" })).toBe("GitHub: error connecting to api.github.com")
    expect(formatAvailabilityHint({ status: "available", owner: "o", name: "r" })).toBeNull()
    expect(formatAvailabilityHint({ status: "notGitHub" })).toBeNull()
    expect(formatAvailabilityHint(null)).toBeNull()
  })
})

describe("getAvailabilityAction", () => {
  const signIn = { signInAvailable: true, manageUrl: "https://github.com/settings/connections/applications/c" }
  const noSignIn = { signInAvailable: false, manageUrl: null }
  const label = (...args: Parameters<typeof getAvailabilityAction>) => getAvailabilityAction(...args)?.label ?? null

  it("offers the sign-in for a missing or rejected token, Settings in a build without it, and nothing while unknown", () => {
    expect(label({ status: "signedOut" }, signIn)).toBe("Sign In…")
    expect(label({ status: "notAuthenticated", source: "gh" }, signIn)).toBe("Sign In…")
    expect(label({ status: "signedOut" }, noSignIn)).toBe("Settings…")
    expect(label({ status: "notAuthenticated", source: "gh" }, noSignIn)).toBe("Settings…")
    expect(label({ status: "signedOut" }, null)).toBeNull()
    expect(label({ status: "ssoRequired", url: "https://github.com/orgs/o/sso" }, null)).toBe("Authorize…")
    expect(label({ status: "ssoRequired", url: null }, signIn)).toBeNull()
    expect(label({ status: "error", message: "offline" }, signIn)).toBeNull()
  })

  it("asks an organization to approve the token of the app's sign-in, on the page of the app on GitHub", () => {
    expect(label({ status: "noAccess", source: "branchDeck", message: "no repository o/r" }, signIn)).toBe("Request Access…")
    expect(label({ status: "noAccess", source: "branchDeck", message: "no repository o/r" }, null)).toBeNull()
    // the page of the Branch Deck app does nothing for the token of gh
    expect(label({ status: "noAccess", source: "gh", message: "no repository o/r" }, signIn)).toBeNull()
  })
})

describe("getCompareUrl", () => {
  it("opens the pull request form of the pushed branch into its base", () => {
    const repository = { owner: "JetBrains", name: "ultimate" }
    expect(getCompareUrl(repository, "master", "develar/virtual/auth")).toBe("https://github.com/JetBrains/ultimate/compare/master...develar/virtual/auth?expand=1")
    // a stacked branch targets its parent's pushed branch; a name keeps its slashes, the rest is encoded
    expect(getCompareUrl(repository, "develar/virtual/auth", "develar/virtual/login#2")).toBe("https://github.com/JetBrains/ultimate/compare/develar/virtual/auth...develar/virtual/login%232?expand=1")
  })
})
