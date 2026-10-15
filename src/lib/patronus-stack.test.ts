import { describe, expect, it } from "vitest"
import type { StackStart } from "@/lib/patronus-stack"
import { describeStackStart } from "@/lib/patronus-stack"

function start(patch: Partial<StackStart>) {
  return describeStackStart({ branchName: "oauth", mode: "rebaseAutosquash", target: "master", ancestors: [], children: [], pullRequest: null, childrenHavePullRequests: false, ...patch })
}

describe("describeStackStart", () => {
  it("leads a Safe Push of a stacked branch root first, and says that its own start lands the ancestors", () => {
    expect(start({ ancestors: ["login"] })).toEqual({
      lead: { text: "oauth is built on login, which isn't in master yet. Safe Push login first, then oauth: after a merge, the next sync moves the branches built on it onto master.", root: "login" },
      note: "Or start oauth now: the Safe Push lands login with it.",
    })
    expect(start({ ancestors: ["auth", "login"], pullRequest: 8 })).toEqual({
      lead: { text: "oauth is built on auth and login, which aren't in master yet. Safe Push auth first, then the next: after a merge, the next sync moves the branches built on it onto master.", root: "auth" },
      note: "Or start oauth now: the Safe Push lands auth and login with it. The approvals of pull request #8 don't count: it compares oauth with login, and Patronus merges a pull request into its base.",
    })
  })

  it("says that a dry run checks the ancestors' commits too", () => {
    expect(start({ mode: "dryRun", ancestors: ["auth", "login"] })).toEqual({ lead: null, note: "It checks the commits of auth and login too: oauth is built on them." })
    expect(start({ mode: "dryRun", children: ["device"] })).toEqual({ lead: null, note: null })
  })

  it("says what the merge of a parent does to the branches built on it", () => {
    expect(start({ branchName: "auth", children: ["login"] }).note).toBe("login is built on auth: after the merge, the next sync moves them onto master.")
    expect(start({ branchName: "auth", children: ["login", "oauth"], childrenHavePullRequests: true }).note)
      .toBe("login and oauth are built on auth: after the merge, the next sync moves them onto master; their pull requests are retargeted to master when they are pushed.")
    expect(start({ branchName: "auth" })).toEqual({ lead: null, note: null })
  })
})
