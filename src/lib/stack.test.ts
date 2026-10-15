import { describe, expect, it } from "vitest"
import type { StackBranchInfo } from "@/lib/stack"
import { summarizePushState } from "@/lib/push-summary"
import { describeParentState, describePrefixChange, passedParentOf, pullRequestBaseOf, stackChildrenPushOrder, stackPushOrder } from "@/lib/stack"
import type { Branch, RemoteStatus } from "@/stores/branch-sync"
import { testBranch, testRemoteStatus } from "@/stores/test-branch"

const table: Record<string, StackBranchInfo> = {
  auth: { declaredParent: null, commitCount: 2 },
  login: { declaredParent: "auth", commitCount: 2 },
  docs: { declaredParent: null, commitCount: 1 },
}

function change(before: string, after: string, stacks = true) {
  return describePrefixChange(before, after, "Saving", stacks, "origin/master", name => table[name])
}

describe("describePrefixChange", () => {
  it("is quiet while the commit stays where it is", () => {
    expect(change("(auth/login) Add form", "(auth/login) Add the form")).toEqual({ text: "The first line is the subject", warning: false })
  })

  it("names the new branch and its parent", () => {
    expect(change("(docs) Document", "(auth/login) Document").text).toBe("Saving moves the commit to the branch login on auth")
    expect(change("(docs) Document", "Document").text).toBe("Saving moves the commit to the unassigned commits")
  })

  it("warns about a parent the other commits of the branch don't declare", () => {
    expect(change("(auth/login) Add form", "(docs/login) Add form").text).toBe("The other commits of login declare auth: its parents would conflict")
  })

  it("says a single-commit branch is restacked or unstacked", () => {
    expect(change("(docs) Document", "(auth/docs) Document").text).toBe("Saving stacks docs on auth")
    expect(change("(auth/docs) Document", "(docs) Document", true).text).toBe("Saving unstacks docs")
  })

  it("warns about a parent without commits", () => {
    expect(change("(docs) Document", "(api/docs) Document")).toEqual({ text: "api has no commits: docs is built on master until it has", warning: true })
  })

  it("compares the whole prefix without stacked branches", () => {
    expect(change("(docs) Document", "(auth/login) Document", false).text).toBe("Saving moves the commit to the branch auth/login")
    expect(change("(auth/login) Add form", "(auth/login) Add the form", false).warning).toBe(false)
  })
})

describe("a parent without commits", () => {
  it("says why the branch isn't built on its parent", () => {
    expect(describeParentState("air-misc", "Integrated")).toBe("air-misc landed")
    expect(describeParentState("air-misc", "Unapplied")).toBe("air-misc is unapplied")
    expect(describeParentState("air-misc", "Missing")).toBe("air-misc has no commits")
  })

  it("is passed only when the branch is built elsewhere", () => {
    const stack = { ...testBranch("IJAI-1184", "air-misc").stack!, parentState: "Integrated" as const, effectiveBase: null, prBaseChanged: true }
    expect(passedParentOf(testBranch("IJAI-1184", "air-misc", { stack }))).toEqual({ parent: "air-misc", state: "Integrated" })
    expect(passedParentOf(testBranch("login", "auth"))).toBeNull()
    expect(passedParentOf(testBranch("auth", null))).toBeNull()
  })
})

describe("push summary with stacked branches", () => {
  it("leaves out a blocked branch: it offers no push", () => {
    const remoteStatus = testRemoteStatus({ exists: false, commitsAhead: 1, myCommitsAhead: 1 })
    const summary = summarizePushState([
      { name: "auth", remoteStatus, hasError: false, commitCount: 1, status: "Created" },
      { name: "login", remoteStatus, hasError: false, commitCount: 1, status: "Blocked" },
    ])
    expect(summary.branches.map(branch => branch.name)).toEqual(["auth"])
  })
})

describe("pull request bases of stacked branches", () => {
  const unpushed: RemoteStatus = testRemoteStatus({ unpushedCommits: ["c1"], commitsAhead: 1, myCommitsAhead: 1 })
  const pushed: RemoteStatus = { ...unpushed, unpushedCommits: [], commitsAhead: 0, myCommitsAhead: 0 }
  const branches: Record<string, Branch> = {
    auth: testBranch("auth", null, { remoteStatus: unpushed }),
    login: testBranch("login", "auth", { remoteStatus: pushed }),
    oauth: testBranch("oauth", "login", { remoteStatus: unpushed }),
  }

  it("targets the parent's branch, or the baseline on the remote", () => {
    const fullName = (name: string) => `user/virtual/${name}`
    const branchOf = (name: string) => branches[name]
    expect(pullRequestBaseOf(branches.oauth!, "origin/master", fullName, branchOf)).toBe("user/virtual/login")
    expect(pullRequestBaseOf(branches.auth!, "origin/master", fullName, branchOf)).toBe("master")
    expect(pullRequestBaseOf(branches.auth!, "main", fullName, branchOf)).toBe("main")
  })

  it("targets the baseline while the parent isn't on the remote", () => {
    const fullName = (name: string) => `user/virtual/${name}`
    const unpushedParent: Record<string, Branch> = { ...branches, login: testBranch("login", "auth", { remoteStatus: testRemoteStatus({ exists: false }) }) }
    expect(pullRequestBaseOf(branches.oauth!, "origin/master", fullName, name => unpushedParent[name])).toBe("master")
    // not loaded yet: the parent stays the base
    const notLoaded: Record<string, Branch> = { ...branches, login: testBranch("login", "auth", { remoteStatus: null }) }
    expect(pullRequestBaseOf(branches.oauth!, "origin/master", fullName, name => notLoaded[name])).toBe("user/virtual/login")
  })

  it("pushes the ancestors that have work for the remote first, root first", () => {
    expect(stackPushOrder("oauth", name => branches[name])).toEqual(["auth", "oauth"])
    expect(stackPushOrder("auth", name => branches[name])).toEqual(["auth"])
  })

  it("pushes the branch and its children that have work for the remote, parents first", () => {
    const dropping: RemoteStatus = { ...unpushed, commitsBehind: 2, fastForward: false }
    const tree: Record<string, Branch> = {
      ...branches,
      session: testBranch("session", "auth", { remoteStatus: unpushed, status: "Blocked" }),
      guide: testBranch("guide", "auth", { remoteStatus: dropping }),
    }
    const descendants = ["login", "session", "guide", "oauth"]
    // login is pushed, session is blocked, a push of guide drops remote commits: it asks first in its row
    expect(stackChildrenPushOrder("auth", descendants, name => tree[name])).toEqual(["auth", "oauth"])
    // a pushed branch pushes only its children
    expect(stackChildrenPushOrder("login", ["oauth"], name => tree[name])).toEqual(["oauth"])
    expect(stackChildrenPushOrder("oauth", [], name => tree[name])).toEqual(["oauth"])
  })
})
