import { describe, expect, it } from "vite-plus/test"
import type { CreatedPullRequest, PatronusReviewer, PatronusReviewRequirement, PullRequest } from "@/lib/bindings"
import { createLatestOnly, formatCreatedToast, joinLogins, preselectedReviewers, searchResults, suggestedReviewers } from "@/lib/create-pull-request"

function reviewer(fullName: string, githubLogin: string | null, patch: Partial<PatronusReviewer> = {}): PatronusReviewer {
  return { fullName, username: fullName.toLowerCase(), githubLogin, available: true, suggested: true, ...patch }
}

function requirement(groups: { name: string; reviewers: PatronusReviewer[] }[], required = true): PatronusReviewRequirement {
  return {
    required,
    mergeRequest: null,
    skipped: false,
    groups: groups.map(group => ({ ...group, paths: [], branchWide: false, pathsSource: "patronus" })),
  } as PatronusReviewRequirement
}

describe("the code owners of the dialog", () => {
  const groups = requirement([
    { name: "Platform", reviewers: [reviewer("Alice", "alice-gh"), reviewer("Bob", null), reviewer("Carol", "carol", { available: false })] },
    { name: "UI", reviewers: [reviewer("Dave", "dave", { suggested: false }), reviewer("Alice", "ALICE-gh"), reviewer("Me", "develar")] },
    { name: "Build", reviewers: [reviewer("Erin", "erin")] },
  ])

  it("are the suggested and available reviewers, without the user", () => {
    expect(suggestedReviewers(groups, "develar")).toEqual([
      { group: "Platform", fullName: "Alice", login: "alice-gh" },
      { group: "Platform", fullName: "Bob", login: null },
      { group: "UI", fullName: "Alice", login: "ALICE-gh" },
      { group: "Build", fullName: "Erin", login: "erin" },
    ])
    // no approval is missing: nobody to suggest
    expect(suggestedReviewers(requirement([{ name: "Platform", reviewers: [reviewer("Alice", "alice")] }], false), null)).toEqual([])
  })

  it("start with one reviewer with a login per group, each once", () => {
    expect(preselectedReviewers(suggestedReviewers(groups, "develar"))).toEqual([
      { login: "alice-gh", name: "Alice" },
      { login: "erin", name: "Erin" },
    ])
  })
})

describe("the reviewer search", () => {
  it("shows the users not chosen yet, without the user", () => {
    const users = [
      { login: "alice", name: "Alice" },
      { login: "Develar", name: null },
      { login: "bob", name: null },
    ]
    expect(searchResults(users, [{ login: "ALICE", name: null }], "develar")).toEqual([{ login: "bob", name: null }])
  })

  it("takes only the answer of the latest request", () => {
    const latest = createLatestOnly()
    const first = latest.next()
    const second = latest.next()
    expect([latest.isLatest(first), latest.isLatest(second)]).toEqual([false, true])
  })
})

describe("the toast of a created pull request", () => {
  const pullRequest = { number: 123, url: "https://github.com/o/r/pull/123", reviewers: [{ login: "alice", state: "requested" }] } as PullRequest
  const result = (patch: Partial<CreatedPullRequest>): CreatedPullRequest => ({ pullRequest, created: true, assignees: ["develar"], warnings: [], ...patch })

  it("says the base, who reviews and the assignee", () => {
    expect(formatCreatedToast("feature-auth", "master", "develar", result({}))).toEqual({
      type: "success",
      title: "Created pull request #123",
      description: "feature-auth → master · review requested from alice · assigned to you",
    })
  })

  it("is a warning that names what GitHub refused", () => {
    const toast = formatCreatedToast("feature-auth", "auth", "develar", result({ assignees: [], warnings: ["cannot request review from bob: not a collaborator"] }))
    expect(toast).toEqual({
      type: "warning",
      title: "Created pull request #123",
      description: "feature-auth → auth · review requested from alice · cannot request review from bob: not a collaborator",
    })
  })

  it("says that one which existed got nothing of the dialog", () => {
    expect(formatCreatedToast("feature-auth", "master", "develar", result({ created: false }))).toEqual({
      type: "info",
      title: "feature-auth already has pull request #123",
      description: "The title, the reviewers and the assignee weren't applied to it",
    })
  })

  it("joins the logins as a sentence", () => {
    expect([joinLogins(["a"]), joinLogins(["a", "b"]), joinLogins(["a", "b", "c"])]).toEqual(["a", "a and b", "a, b and c"])
  })
})
