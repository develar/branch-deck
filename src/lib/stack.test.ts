import { describe, expect, it } from "vite-plus/test"
import { summarizePushState } from "@/lib/push-summary"
import type { StackBranchInfo } from "@/lib/stack"
import {
  buttonStackPushOrder,
  describeParentState,
  describePrefixChange,
  formatMoveUnstackAction,
  formatPushWithChildrenLabel,
  formatStackPushing,
  offersPushWithChildren,
  passedParentOf,
  pullRequestBaseOf,
  stackAncestors,
  stackChildrenPushOrder,
  stackPushOrder,
  stackPushSubject,
} from "@/lib/stack"
import type { Branch, RemoteStatus } from "@/stores/branch-sync"
import { testBranch, testRemoteStatus } from "@/stores/test-branch"

const table: Record<string, StackBranchInfo> = {
  auth: { declaredParent: null, commitCount: 2 },
  login: { declaredParent: "auth", commitCount: 2 },
  docs: { declaredParent: null, commitCount: 1 },
}

function change(before: string, after: string, stacks = true) {
  return describePrefixChange(before, after, "Saving", stacks, "master", name => table[name])
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

  it("warns about a parent level that differs from the stored parent", () => {
    const stored = (before: string, after: string) => describePrefixChange(before, after, "Saving", true, "master", name => table[name], {}, { login: "auth" })
    expect(stored("(login) Add form", "(docs/login) Add form")).toEqual({
      text: "login is stacked on auth in the stack parents: the prefix would conflict with it",
      warning: true,
    })
    // dropping a parent level that the stack parents repeat keeps the branch where it is
    expect(stored("(auth/login) Add form", "(login) Add form")).toEqual({ text: "The first line is the subject", warning: false })
    expect(stored("(login) Add form", "(auth/login) Add form").warning).toBe(false)
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

  it("lists the branches a branch is built on, root first", () => {
    expect(stackAncestors("oauth", name => branches[name]).map(branch => branch.name)).toEqual(["auth", "login"])
    expect(stackAncestors("auth", name => branches[name])).toEqual([])
  })

  it("starts the stack push of the button above an ancestor whose push drops remote commits", () => {
    const dropping: RemoteStatus = { ...unpushed, commitsBehind: 2, fastForward: false }
    const tree: Record<string, Branch> = {
      auth: testBranch("auth", null, { remoteStatus: unpushed }),
      login: testBranch("login", "auth", { remoteStatus: dropping }),
      oauth: testBranch("oauth", "login", { remoteStatus: unpushed }),
      device: testBranch("device", "oauth", { remoteStatus: unpushed }),
    }
    const branchOf = (name: string) => tree[name]
    // the Push Stack of the button's description pushes every ancestor, the push of login asks first through its row
    expect(stackPushOrder("device", branchOf)).toEqual(["auth", "login", "oauth", "device"])
    // the button leaves no stale login between auth and oauth: it pushes what is above login
    expect(stackPushOrder("device", branchOf, { stopAtConfirm: true })).toEqual(["oauth", "device"])
    expect(buttonStackPushOrder("device", branchOf)).toEqual(["oauth", "device"])
    // right above login there is nothing else to push: the button pushes oauth alone
    expect(buttonStackPushOrder("oauth", branchOf)).toEqual([])
  })

  it("pushes the stack from the button only while the branch's own push asks nothing", () => {
    const branchOf = (name: string) => branches[name]
    expect(buttonStackPushOrder("oauth", branchOf)).toEqual(["auth", "oauth"])
    // a root, and a branch whose ancestors have nothing to push
    expect(buttonStackPushOrder("auth", branchOf)).toEqual([])
    const pushedAuth: Record<string, Branch> = { ...branches, auth: testBranch("auth", null, { remoteStatus: pushed }) }
    expect(buttonStackPushOrder("oauth", name => pushedAuth[name])).toEqual([])
    // nothing of its own to push, or a push that drops remote commits: the button is about the branch
    const upToDate: Record<string, Branch> = { ...branches, oauth: testBranch("oauth", "login", { remoteStatus: pushed }) }
    expect(buttonStackPushOrder("oauth", name => upToDate[name])).toEqual([])
    const dropping: Record<string, Branch> = { ...branches, oauth: testBranch("oauth", "login", { remoteStatus: { ...unpushed, commitsBehind: 1, fastForward: false } }) }
    expect(buttonStackPushOrder("oauth", name => dropping[name])).toEqual([])
  })

  it("pushes an amended or restacked ancestor with the stack and asks nothing: its push replaces older versions", () => {
    const tree: Record<string, Branch> = {
      auth: testBranch("auth", null, { remoteStatus: { ...unpushed, commitsAmended: 1, fastForward: false } }),
      // no commit of its own to push: the remote has the older version of auth's commit
      login: testBranch("login", "auth", { remoteStatus: { ...pushed, commitsRestacked: 1, fastForward: false } }),
      oauth: testBranch("oauth", "login", { remoteStatus: { ...unpushed, commitsRestacked: 1, fastForward: false } }),
    }
    expect(buttonStackPushOrder("oauth", name => tree[name])).toEqual(["auth", "login", "oauth"])
    expect(stackChildrenPushOrder("auth", ["login", "oauth"], name => tree[name])).toEqual(["auth", "login", "oauth"])
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

  it("leaves out the branches a running Safe Push holds, and starts a stack push above them", () => {
    const tree: Record<string, Branch> = {
      auth: testBranch("auth", null, { remoteStatus: unpushed }),
      login: testBranch("login", "auth", { remoteStatus: unpushed }),
      oauth: testBranch("oauth", "login", { remoteStatus: unpushed }),
    }
    const branchOf = (name: string) => tree[name]
    // the Safe Push of login lands auth too
    const isHeld = (name: string) => name === "auth" || name === "login"
    expect(stackPushOrder("oauth", branchOf, { isHeld })).toEqual(["oauth"])
    expect(buttonStackPushOrder("oauth", branchOf, isHeld)).toEqual([])
    expect(stackChildrenPushOrder("auth", ["login", "oauth"], branchOf, isHeld)).toEqual(["oauth"])
    expect(stackPushOrder("oauth", branchOf, { isHeld: name => name === "auth" })).toEqual(["login", "oauth"])
  })

  it("names Push with Children by what it pushes, and offers it only beyond the branch itself", () => {
    expect(formatPushWithChildrenLabel(["auth", "oauth", "login"])).toBe("Push with Children (3 branches)")
    expect(formatPushWithChildrenLabel(["oauth"])).toBe("Push with Children (oauth)")
    expect(offersPushWithChildren("auth", ["auth"])).toBe(false)
    expect(offersPushWithChildren("auth", [])).toBe(false)
    expect(offersPushWithChildren("auth", ["oauth"])).toBe(true)
  })

  it("names what a stack push sends while it runs: its one push of several branches, or the one branch", () => {
    expect(formatStackPushing("oauth", ["auth", "login", "oauth"])).toBe("Pushing 3 branches")
    expect(formatStackPushing("auth", ["login"])).toBe("Pushing login")
    expect(formatStackPushing("auth", ["auth"])).toBe("Pushing")
    // waiting for a sync, or no stack push: the branch's own push
    expect(formatStackPushing("auth", [])).toBe("Pushing")
    expect(formatStackPushing("auth", null)).toBe("Pushing")
    expect(stackPushSubject("auth", ["auth", "login"])).toBe("2 branches")
    expect(stackPushSubject("auth", [])).toBe("auth")
  })
})

describe("formatMoveUnstackAction", () => {
  it("names the branch it unstacks unless it is the target the toast names", () => {
    expect(formatMoveUnstackAction("perf-part-2", "IJPL-257337", "perf-part-2")).toBe("Unstack from IJPL-257337")
    expect(formatMoveUnstackAction("a", "r", "b")).toBe("Unstack a from r")
    expect(formatMoveUnstackAction("a", "r", null)).toBe("Unstack a from r")
  })
})
