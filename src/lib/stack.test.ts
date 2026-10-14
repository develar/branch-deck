import { describe, expect, it } from "vitest"
import type { StackBranchInfo } from "@/lib/stack"
import { summarizePushState } from "@/lib/push-summary"
import { describePrefixChange } from "@/lib/stack"

const table: Record<string, StackBranchInfo> = {
  auth: { declaredParent: null, commitCount: 2 },
  login: { declaredParent: "auth", commitCount: 2 },
  docs: { declaredParent: null, commitCount: 1 },
}

function change(before: string, after: string, stacks = true) {
  return describePrefixChange(before, after, "Saving", stacks, name => table[name])
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
    expect(change("(docs) Document", "(api/docs) Document").text).toBe("api has no commits: docs would be blocked until it has")
  })

  it("compares the whole prefix without stacked branches", () => {
    expect(change("(docs) Document", "(auth/login) Document", false).text).toBe("Saving moves the commit to the branch auth/login")
    expect(change("(auth/login) Add form", "(auth/login) Add the form", false).warning).toBe(false)
  })
})

describe("push summary with stacked branches", () => {
  it("leaves out a blocked branch: it offers no push", () => {
    const remoteStatus = { exists: false, unpushedCommits: [], commitsAhead: 1, commitsBehind: 0, myCommitsAhead: 1, fastForward: true, lastPushTime: 0 }
    const summary = summarizePushState([
      { name: "auth", remoteStatus, hasError: false, commitCount: 1, status: "Created" },
      { name: "login", remoteStatus, hasError: false, commitCount: 1, status: "Blocked" },
    ])
    expect(summary.branches.map(branch => branch.name)).toEqual(["auth"])
  })
})
