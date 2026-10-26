import { describe, expect, it } from "vite-plus/test"
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
    expect(change("(auth/login) Add form", "(docs/login) Add form")).toEqual({
      text: "The other commits of login declare auth: its parents would conflict",
      warning: true,
      refused: "Saving is refused: the other commits of login declare auth, its parents would conflict",
    })
  })

  it("warns about an empty level of the prefix, which a reword refuses", () => {
    expect(change("(docs) Document", "(auth//docs) Document")).toEqual({
      text: "The prefix has an empty level: docs will report an error",
      warning: true,
      refused: "Saving is refused: the prefix has an empty level",
    })
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
