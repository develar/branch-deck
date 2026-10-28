import { describe, expect, it } from "vite-plus/test"
import type { CommitGroup } from "@/lib/bindings"
import type { StackBranchInfo } from "@/lib/stack"
import {
  baselineOnRemote,
  describeParentState,
  describePrefixChange,
  formatMoveUnstackAction,
  formatPushWithChildrenLabel,
  formatStackPushing,
  passedParentOf,
  stackPushSubject,
} from "@/lib/stack"
import { testBranch } from "@/stores/test-branch"

const table: Record<string, StackBranchInfo> = {
  auth: { declaredParent: null, commitCount: 2 },
  login: { declaredParent: "auth", commitCount: 2 },
  docs: { declaredParent: null, commitCount: 1 },
}

/** A group as Rust previews it (`preview_group`): of a prefix, its parent declared by the prefix */
function prefix(branch: string, parent: string | null = null, invalid = false): CommitGroup {
  return { branch, origin: "prefix", parent, parentSource: parent === null ? null : "Prefix", invalid }
}

/** A group whose parent is its line in the stack parents */
function stored(branch: string, parent: string): CommitGroup {
  return { branch, origin: "prefix", parent, parentSource: "Store", invalid: false }
}

function change(from: CommitGroup | null, to: CommitGroup | null, stacks = true, stackParents: Readonly<Record<string, string>> = {}) {
  return describePrefixChange(from, to, "Saving", stacks, "master", name => table[name], stackParents)
}

describe("describePrefixChange", () => {
  it("is quiet while the commit stays where it is", () => {
    expect(change(prefix("login", "auth"), prefix("login", "auth"))).toEqual({ text: "The first line is the subject", warning: false })
  })

  it("names the new branch and its parent", () => {
    expect(change(prefix("docs"), prefix("login", "auth")).text).toBe("Saving moves the commit to the branch login on auth")
    expect(change(prefix("docs"), null).text).toBe("Saving moves the commit to the unassigned commits")
  })

  it("warns about a parent the other commits of the branch don't declare", () => {
    expect(change(prefix("login", "auth"), prefix("login", "docs"))).toEqual({
      text: "The other commits of login declare auth: its parents would conflict",
      warning: true,
      refused: "Saving is refused: the other commits of login declare auth, its parents would conflict",
    })
  })

  it("warns about an empty level of the prefix, which a reword refuses", () => {
    expect(change(prefix("docs"), prefix("docs", null, true))).toEqual({
      text: "The prefix has an empty level: docs will report an error",
      warning: true,
      refused: "Saving is refused: the prefix has an empty level",
    })
  })

  it("says a single-commit branch is restacked or unstacked", () => {
    expect(change(prefix("docs"), prefix("docs", "auth")).text).toBe("Saving stacks docs on auth")
    expect(change(prefix("docs", "auth"), prefix("docs")).text).toBe("Saving unstacks docs")
  })

  it("warns about a parent without commits", () => {
    expect(change(prefix("docs"), prefix("docs", "api"))).toEqual({ text: "api has no commits: docs is built on master until it has", warning: true })
  })

  it("warns about a parent level that differs from the stored parent", () => {
    const parents = { login: "auth" }
    expect(change(stored("login", "auth"), prefix("login", "docs"), true, parents)).toEqual({
      text: "login is stacked on auth in the stack parents: the prefix would conflict with it",
      warning: true,
    })
    // dropping a parent level that the stack parents repeat keeps the branch where it is
    expect(change(prefix("login", "auth"), stored("login", "auth"), true, parents)).toEqual({ text: "The first line is the subject", warning: false })
    expect(change(stored("login", "auth"), prefix("login", "auth"), true, parents).warning).toBe(false)
  })

  it("compares the whole prefix without stacked branches", () => {
    expect(change(prefix("docs"), prefix("auth/login"), false).text).toBe("Saving moves the commit to the branch auth/login")
    expect(change(prefix("auth/login"), prefix("auth/login"), false).warning).toBe(false)
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

describe("the pull request base", () => {
  it("is the baseline as the remote names it for every branch, stacked or not", () => {
    expect(baselineOnRemote("origin/master")).toBe("master")
    expect(baselineOnRemote("upstream/release/252")).toBe("release/252")
    expect(baselineOnRemote("main")).toBe("main")
  })
})

describe("pushes of stacked branches", () => {
  it("names Push with Children by what it pushes", () => {
    expect(formatPushWithChildrenLabel(["auth", "oauth", "login"])).toBe("Push with Children (3 branches)")
    expect(formatPushWithChildrenLabel(["oauth"])).toBe("Push with Children (oauth)")
    // the stack branch of the whole stack that goes along is one more branch it pushes
    expect(formatPushWithChildrenLabel(["auth", "oauth"], true)).toBe("Push with Children (3 branches)")
    expect(formatPushWithChildrenLabel([], true)).toBe("Push with Children (the whole stack branch)")
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
