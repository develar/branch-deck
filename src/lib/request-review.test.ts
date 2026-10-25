import { describe, expect, it } from "vite-plus/test"
import type { MissingReviewGroup, Person, PullRequest, Reviewer, ReviewReadiness } from "@/lib/bindings"
import { describeCodeOwnersFact, formatRefused, formatRequestedToast, getRequestReviewCandidates, getRequestReviewItem, requestReviewPushes } from "@/lib/request-review"
import type { RemoteStatus } from "@/stores/branch-sync"

function person(fullName: string | null, login: string | null): Person {
  return { fullName, login }
}

const ilya = person("Ilya Korennoy", "ikorennoy")
const alice = person("Alice", "alice-gh")

function group(name: string, patch: Partial<MissingReviewGroup> = {}): MissingReviewGroup {
  return { name, needed: 1, approvedBy: [], requested: [], suggested: [], changesRequestedBy: [], ...patch }
}

function missing(...groups: MissingReviewGroup[]): ReviewReadiness {
  return { kind: "missing", groups, notCounted: [], reason: "approvalsMissing" }
}

function pullRequest(reviewers: Reviewer[] = [], patch: Partial<PullRequest> = {}): PullRequest {
  return {
    number: 1078,
    title: "Skip WA",
    url: "https://github.com/acme/monorepo/pull/1078",
    state: "open",
    isDraft: false,
    reviewDecision: null,
    headRefName: "me/virtual/wsm-skip-wa-2",
    headRefOid: "abc",
    checks: null,
    updatedAt: 0,
    reviewers,
    author: "me",
    ...patch,
  }
}

function remote(patch: Partial<RemoteStatus> = {}): { remoteStatus: RemoteStatus } {
  return {
    remoteStatus: {
      exists: true,
      unpushedCommits: [],
      commitsAhead: 0,
      commitsBehind: 0,
      commitsAmended: 0,
      commitsRestacked: 0,
      myCommitsAhead: 0,
      fastForward: true,
      lastPushTime: 0,
      action: { kind: "upToDate" },
      ...patch,
    } as RemoteStatus,
  }
}

describe("the code owners Request Review… offers", () => {
  it("are the suggested ones of the missing groups, one per group nobody is asked for chosen, each once", () => {
    const readiness = missing(
      group("IJPL Code Platform", { suggested: [ilya, alice] }),
      group("UI", { suggested: [person("Alice", "ALICE-gh"), person("Bob", "bob")] }),
      group("Build", { suggested: [person("Erin", null)] }),
    )
    expect(getRequestReviewCandidates(readiness, pullRequest())).toEqual([
      { login: "ikorennoy", fullName: "Ilya Korennoy", groups: ["IJPL Code Platform"], preselected: true },
      { login: "alice-gh", fullName: "Alice", groups: ["IJPL Code Platform", "UI"], preselected: true },
      { login: "bob", fullName: "Bob", groups: ["UI"], preselected: false },
    ])
  })

  it("leave out the author, whoever is asked already and an approval that counts; a group someone is asked for chooses nobody", () => {
    const readiness = missing(
      group("IJPL Code Platform", { requested: [ilya], suggested: [ilya, alice] }),
      group("UI", { needed: 1, approvedBy: [person("Bob", "bob")], suggested: [person("Bob", "bob"), person("Me", "me"), person("Carol", "carol")] }),
    )
    const asked: Reviewer = { login: "carol", state: "commented", requested: true }
    expect(getRequestReviewCandidates(readiness, pullRequest([asked]))).toEqual([{ login: "alice-gh", fullName: "Alice", groups: ["IJPL Code Platform"], preselected: false }])
  })

  it("are none when no approval is missing or the readiness is unknown", () => {
    expect(getRequestReviewCandidates({ kind: "approved", groups: [] }, pullRequest())).toEqual([])
    expect(getRequestReviewCandidates({ kind: "unknown", reason: "approvalsNotEvaluated" }, pullRequest())).toEqual([])
    expect(getRequestReviewCandidates(null, pullRequest())).toEqual([])
  })
})

describe("what Request Review… says of the code owners", () => {
  it("says while Patronus checks them, when its check failed or doesn't tell, never nothing as if none were needed", () => {
    expect(describeCodeOwnersFact(null, true)).toBe("Asking Patronus which code owners must approve…")
    expect(describeCodeOwnersFact(null, false)).toBeNull()
    expect(describeCodeOwnersFact({ kind: "unavailable", reason: "error", message: "503" }, true)).toBe(
      "Patronus didn't answer which code owners must approve (503): ask the reviewers yourself.",
    )
    expect(describeCodeOwnersFact({ kind: "unknown", reason: "approvalsNotEvaluated" }, true)).toBe(
      "Patronus doesn't tell yet which code owners must approve: ask the reviewers yourself.",
    )
    expect(describeCodeOwnersFact({ kind: "notRequired" }, true)).toBeNull()
  })
})

describe("the Request Review… item", () => {
  const readiness = missing(group("IJPL Code Platform", { suggested: [ilya] }))
  const open = () => {}

  it("asks the chosen code owners on the open pull request", () => {
    expect(getRequestReviewItem(pullRequest(), readiness, open)).toMatchObject({
      id: "request-review",
      label: "Request Review…",
      disabled: false,
      description: "Ask Ilya Korennoy to review #1078",
    })
    expect(getRequestReviewItem(pullRequest(), null, open)).toMatchObject({ disabled: false, description: "Ask for a review of #1078" })
  })

  it("is disabled with the reason for a draft only: work the remote lacks is pushed by the request", () => {
    expect(getRequestReviewItem(pullRequest([], { isDraft: true }), readiness, open)).toMatchObject({
      disabled: true,
      description: "#1078 is a draft: mark it ready for review on GitHub first",
    })
    expect(getRequestReviewItem(pullRequest(), readiness, open)).toMatchObject({ disabled: false })
    expect(requestReviewPushes(remote({ commitsAhead: 1, action: { kind: "push" } }))).toBe(true)
    // only rebuilt on a newer baseline: the remote has the work; an unknown remote status is no push
    expect(requestReviewPushes(remote({ fastForward: false }))).toBe(false)
    expect(requestReviewPushes({ remoteStatus: null })).toBe(false)
  })

  it("is not there without an open pull request", () => {
    expect(getRequestReviewItem(undefined, readiness, open)).toBeNull()
    expect(getRequestReviewItem(pullRequest([], { state: "merged" }), readiness, open)).toBeNull()
  })
})

describe("the toast of a request", () => {
  const nameOf = (login: string) => (login === "ikorennoy" ? "Ilya Korennoy" : login)

  it("names whom GitHub asked, and whom it refused as a warning", () => {
    expect(formatRequestedToast({ number: 1078 }, { requested: ["ikorennoy"], refused: [] }, nameOf)).toEqual({
      title: "Requested a review from Ilya Korennoy on #1078",
    })
    const refused = { requested: ["ikorennoy"], refused: [{ login: "me", reason: "Review cannot be requested from pull request author." }] }
    expect(formatRequestedToast({ number: 1078 }, refused, nameOf)).toEqual({
      type: "warning",
      title: "Requested a review from Ilya Korennoy on #1078",
      description: "GitHub refused me: Review cannot be requested from pull request author.",
    })
  })

  it("is none when GitHub asked nobody: the error says why", () => {
    const result = { requested: [], refused: [{ login: "ikorennoy", reason: "Reviews may only be requested from collaborators." }] }
    expect(formatRequestedToast({ number: 1078 }, result, nameOf)).toBeNull()
    expect(formatRefused(result, nameOf)).toBe("Ilya Korennoy: Reviews may only be requested from collaborators.")
  })
})
