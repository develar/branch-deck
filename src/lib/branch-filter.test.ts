import { describe, expect, it, vi } from "vite-plus/test"
import type { PatronusRobot, PullRequest } from "@/lib/bindings"
import type { BranchFilter, BranchStateSources } from "@/lib/branch-filter"
import { chipCount, collectChipMatches, filterBranches, formatNoMatch, isFilterActive, keepShown, matchesBranchText, NO_FILTER } from "@/lib/branch-filter"
import type { Branch, SyncedCommit } from "@/stores/branch-sync"
import { testBranch, testRemoteStatus } from "@/stores/test-branch"

function commit(name: string, subject: string): SyncedCommit {
  return { originalHash: `${name}-original`, hash: `${name}-tip`, strippedSubject: subject, message: subject, author: "Ann", authorTime: 0, committerTime: 0 }
}

/** A branch at the commit `${name}-tip` with one commit of `subject`, not pushed */
function branch(name: string, overrides: Partial<Branch> = {}, subject = `Work on ${name}`): Branch {
  return testBranch(name, null, { commits: [commit(name, subject)], remoteStatus: testRemoteStatus({ exists: false }), ...overrides })
}

function robot(branchName: string, status: PatronusRobot["status"]): PatronusRobot {
  return {
    id: `robot-${branchName}`,
    name: branchName,
    status,
    pushMode: "rebaseAutosquash",
    branchName,
    sourceRevision: `${branchName}-tip`,
    targetBranch: "master",
    startTime: 1,
    finishTime: null,
    cancellationReason: null,
    url: "https://patronus/robot",
  }
}

function pullRequest(branchName: string, patch: Partial<PullRequest>): PullRequest {
  return {
    number: 7,
    title: branchName,
    url: "https://github.com/o/r/pull/7",
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

function sources(branches: Branch[], patch: Partial<Omit<BranchStateSources, "branches">> = {}): BranchStateSources {
  return { branches: Object.fromEntries(branches.map(it => [it.name, it])), robots: undefined, pullRequests: {}, isLanded: () => false, ...patch }
}

// `@/lib/patronus` opens robots and reports errors
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: async () => {} }))
vi.mock("@/lib/errors", () => ({ notifyError: () => {} }))

describe("branch filter", () => {
  it("matches every word in the name, the issue key, the summary, the description title or a commit subject, ignoring case", () => {
    const auth = branch("feature-auth", { issueKey: "IJPL-12", summary: "Add the login form", descriptionTitle: "Sign in with OAuth" }, "Hash the passwords")
    expect(matchesBranchText(auth, "AUTH")).toBe(true)
    expect(matchesBranchText(auth, "ijpl-12")).toBe(true)
    expect(matchesBranchText(auth, "login")).toBe(true)
    expect(matchesBranchText(auth, "oauth")).toBe(true)
    expect(matchesBranchText(auth, "passwords")).toBe(true)
    // the words may be in different texts, but all must be there
    expect(matchesBranchText(auth, "  feature   passwords ")).toBe(true)
    expect(matchesBranchText(auth, "feature docs")).toBe(false)
    expect(matchesBranchText(auth, "  ")).toBe(true)
  })

  it("puts a branch into To Push by the rule of the outgoing counts", () => {
    const branches = [
      branch("new"),
      branch("pushed", { remoteStatus: testRemoteStatus() }),
      branch("ahead", { remoteStatus: testRemoteStatus({ unpushedCommits: ["a"], commitsAhead: 1, myCommitsAhead: 1 }) }),
      // nothing ahead, but the remote has older versions of its commits: a force push replaces them
      branch("amended", { remoteStatus: testRemoteStatus({ commitsAmended: 1, fastForward: false }) }),
      branch("restacked", { remoteStatus: testRemoteStatus({ commitsRestacked: 2, fastForward: false }) }),
      branch("blocked", { status: "Blocked" }),
      branch("broken", { hasError: true, status: "Error" }),
      branch("landed"),
      branch("loading", { remoteStatus: null }),
    ]
    const matches = collectChipMatches(
      branches.map(it => it.name),
      sources(branches, { isLanded: name => name === "landed" }),
    )
    expect([...matches.toPush]).toEqual(["new", "ahead", "amended", "restacked"])
    expect(chipCount(matches, "all")).toBe(9)
    expect(chipCount(matches, "toPush")).toBe(4)
  })

  it("puts a branch whose robot runs or cancels into Running", () => {
    const branches = [branch("running"), branch("canceling"), branch("failed"), branch("idle")]
    const robots = Object.fromEntries([robot("running", "running"), robot("canceling", "canceling"), robot("failed", "failure")].map(it => [it.id, it]))
    const matches = collectChipMatches(
      branches.map(it => it.name),
      sources(branches, { robots }),
    )
    expect([...matches.running]).toEqual(["running", "canceling"])
  })

  it("puts an error, a conflict, a blocked branch, a failed robot of the branch as it is and a pull request waiting for its author into Attention", () => {
    const branches = [
      branch("broken", { hasError: true, status: "Error" }),
      branch("conflict", { hasError: true, status: "MergeConflict" }),
      branch("blocked", { status: "Blocked" }),
      branch("failed"),
      // the robot took a commit the branch no longer has: its failure is old news
      branch("changed", { commits: [commit("changed-now", "Work on changed")], remoteStatus: testRemoteStatus({ remoteTip: "changed-now-tip" }) }),
      branch("review"),
      branch("fine"),
    ]
    const robots = Object.fromEntries([robot("failed", "failure"), robot("changed", "failure")].map(it => [it.id, it]))
    const pullRequests = { review: pullRequest("review", { reviewDecision: "changesRequested" }), fine: pullRequest("fine", { reviewDecision: "approved" }) }
    const matches = collectChipMatches(
      branches.map(it => it.name),
      sources(branches, { robots, pullRequests }),
    )
    expect([...matches.attention]).toEqual(["broken", "conflict", "blocked", "failed", "review"])
  })

  it("shows the branches of the chosen chip that match the text, in table order; an inactive filter shows all", () => {
    const branches = [branch("auth"), branch("api", { remoteStatus: testRemoteStatus() }), branch("docs", {}, "Document the API")]
    const all = sources(branches)
    const matches = collectChipMatches(
      branches.map(it => it.name),
      all,
    )
    const filter = (patch: Partial<BranchFilter>) => filterBranches(matches, all.branches, { ...NO_FILTER, ...patch })
    expect(filter({})).toBeNull()
    expect(isFilterActive({ text: " ", chip: "all" })).toBe(false)
    expect(filter({ text: "api" })).toEqual(["api", "docs"])
    expect(filter({ chip: "toPush" })).toEqual(["auth", "docs"])
    expect(filter({ text: "api", chip: "toPush" })).toEqual(["docs"])
    expect(filter({ text: "gone" })).toEqual([])
  })

  it("keeps the branches shown since the filter changed, and adds the new matches", () => {
    const first = keepShown(null, ["auth", "api"])
    expect([...first]).toEqual(["auth", "api"])
    // auth was pushed: it no longer matches To Push, but stays
    expect(keepShown(first, ["api"])).toBe(first)
    expect([...keepShown(first, ["api", "docs"])]).toEqual(["auth", "api", "docs"])
  })

  it("says what found nothing", () => {
    expect(formatNoMatch({ text: " foo ", chip: "all" })).toBe("No branches match “foo”")
    expect(formatNoMatch({ text: "foo", chip: "toPush" })).toBe("No branches to push match “foo”")
    expect(formatNoMatch({ text: "", chip: "toPush" })).toBe("Nothing to push")
    expect(formatNoMatch({ text: "", chip: "running" })).toBe("No robot is running")
    expect(formatNoMatch({ text: "", chip: "attention" })).toBe("Nothing needs attention")
  })
})
