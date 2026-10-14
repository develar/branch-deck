// @spec spec/commit/squash.spec.md
import { describe, expect, it } from "vitest"
import { buildSquashMessage, countPushedCommits } from "@/lib/squash-message"
import type { RemoteStatus, SyncedCommit } from "@/stores/branch-sync"
import { testRemoteStatus } from "@/stores/test-branch"

describe("buildSquashMessage", () => {
  it("keeps the oldest message and appends the others as paragraphs without their prefix", () => {
    expect(buildSquashMessage([
      "(auth) Add login\n\nThe login form",
      "(auth) Add logout",
      "(auth) Remember the user\n\nIn a cookie\n",
    ])).toBe("(auth) Add login\n\nThe login form\n\nAdd logout\n\nRemember the user\n\nIn a cookie")
  })

  it("removes the autosquash marker and leaves out a message already included", () => {
    expect(buildSquashMessage(["(auth) Add login", "fixup! (auth) Add login", "squash! Add login\n\nAlso validate", "amend! Validate"]))
      .toBe("(auth) Add login\n\nAdd login\n\nAlso validate\n\nValidate")
    expect(buildSquashMessage(["Add login", "fixup! Add login", "Add login  "])).toBe("Add login")
  })

  it("cleans up trailing whitespace and blank lines", () => {
    expect(buildSquashMessage(["\nAdd a  \n\n", "\n\nAdd b\n"])).toBe("Add a\n\nAdd b")
  })

  it("keeps the issue key of a newer subject grouped by its prefix", () => {
    expect(buildSquashMessage(["ABC-1: Add parser", "(lexer) ABC-1: Add lexer"])).toBe("ABC-1: Add parser\n\nABC-1: Add lexer")
  })

  it("removes the issue key a newer subject is grouped by", () => {
    expect(buildSquashMessage(["ABC-123: Add parser", "ABC-123: Add tokens\n\nFor the lexer", "ABC-123 Fix the parser", "fixup! ABC-123: Add parser"]))
      .toBe("ABC-123: Add parser\n\nAdd tokens\n\nFor the lexer\n\nFix the parser")
    // a key after a `[category]` or a `type:` stays
    expect(buildSquashMessage(["ABC-1: Add a", "fix: ABC-1 Add b"])).toBe("ABC-1: Add a\n\nfix: ABC-1 Add b")
  })
})

describe("countPushedCommits", () => {
  const commit = (hash: string | undefined): SyncedCommit => ({
    originalHash: `original-${hash}`,
    hash,
    strippedSubject: "",
    message: "",
    author: "",
    authorTime: 0,
    committerTime: 0,
  })
  const remote = (exists: boolean, unpushedCommits: string[]): RemoteStatus => testRemoteStatus({
    exists,
    unpushedCommits,
    commitsAhead: unpushedCommits.length,
    myCommitsAhead: unpushedCommits.length,
  })

  it("counts the synced commits that are not ahead of the remote branch", () => {
    expect(countPushedCommits([commit("a"), commit("b"), commit("c")], remote(true, ["c"]))).toBe(2)
  })

  it("counts nothing without a remote branch or for commits not synced yet", () => {
    expect(countPushedCommits([commit("a")], remote(false, []))).toBe(0)
    expect(countPushedCommits([commit("a")], null)).toBe(0)
    expect(countPushedCommits([commit(undefined)], remote(true, []))).toBe(0)
  })
})
