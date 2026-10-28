// @spec spec/commit/squash.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { SubjectParts } from "@/lib/bindings"
import { buildSquashMessage, countPushedCommits } from "@/lib/squash-message"
import type { SquashedMessage } from "@/lib/squash-message"
import type { RemoteStatus, SyncedCommit } from "@/stores/branch-sync"
import { testRemoteStatus } from "@/stores/test-branch"
import { testSubject } from "@/test-support/subject-parts"

describe("buildSquashMessage", () => {
  // a message with its subject as Rust splits it (`subject_parts`)
  const squashed = (message: string, parts: Partial<SubjectParts> = {}): SquashedMessage => ({ message, subjectParts: testSubject("", parts) })

  it("keeps the oldest message and appends the others as paragraphs without their prefix", () => {
    const commits = [
      squashed("(auth) Add login\n\nThe login form", { prefix: "auth", rest: "Add login" }),
      squashed("(auth) Add logout", { prefix: "auth", rest: "Add logout" }),
      squashed("(auth) Remember the user\n\nIn a cookie\n", { prefix: "auth", rest: "Remember the user" }),
    ]
    expect(buildSquashMessage(commits)).toBe("(auth) Add login\n\nThe login form\n\nAdd logout\n\nRemember the user\n\nIn a cookie")
  })

  it("removes the autosquash marker and leaves out a message already included", () => {
    const commits = [
      squashed("(auth) Add login", { prefix: "auth", rest: "Add login" }),
      squashed("fixup! (auth) Add login", { autosquash: "fixup! ", prefix: "auth", rest: "Add login" }),
      squashed("squash! Add login\n\nAlso validate", { autosquash: "squash! ", rest: "Add login" }),
      squashed("amend! Validate", { autosquash: "amend! ", rest: "Validate" }),
    ]
    expect(buildSquashMessage(commits)).toBe("(auth) Add login\n\nAdd login\n\nAlso validate\n\nValidate")
    expect(buildSquashMessage([squashed("Add login"), squashed("fixup! Add login", { autosquash: "fixup! " }), squashed("Add login  ")])).toBe("Add login")
  })

  it("cleans up trailing whitespace and blank lines", () => {
    expect(buildSquashMessage([squashed("Add a  \n\n"), squashed("Add b\n")])).toBe("Add a\n\nAdd b")
  })

  it("keeps the issue key of a newer subject grouped by its prefix", () => {
    const commits = [squashed("ABC-1: Add parser", { issueKey: "ABC-1" }), squashed("(lexer) ABC-1: Add lexer", { prefix: "lexer", issueKey: "ABC-1" })]
    expect(buildSquashMessage(commits)).toBe("ABC-1: Add parser\n\nABC-1: Add lexer")
  })

  it("removes the issue key a newer subject is grouped by", () => {
    const key = { issueKey: "ABC-123" }
    const commits = [
      squashed("ABC-123: Add parser", key),
      squashed("ABC-123: Add tokens\n\nFor the lexer", key),
      squashed("ABC-123 Fix the parser", key),
      squashed("fixup! ABC-123: Add parser", { autosquash: "fixup! ", ...key }),
    ]
    expect(buildSquashMessage(commits)).toBe("ABC-123: Add parser\n\nAdd tokens\n\nFor the lexer\n\nFix the parser")
    // a key after a `[category]` or a `type:` stays
    expect(buildSquashMessage([squashed("ABC-1: Add a", { issueKey: "ABC-1" }), squashed("fix: ABC-1 Add b", { issueKey: "ABC-1" })])).toBe("ABC-1: Add a\n\nfix: ABC-1 Add b")
  })
})

describe("countPushedCommits", () => {
  const commit = (hash: string | undefined): SyncedCommit => ({
    originalHash: `original-${hash}`,
    hash,
    strippedSubject: "",
    subjectParts: testSubject(""),
    message: "",
    author: "",
    authorTime: 0,
    committerTime: 0,
  })
  const remote = (exists: boolean, unpushedCommits: string[]): RemoteStatus =>
    testRemoteStatus({
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
