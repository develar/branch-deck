// @spec spec/branch/issue-groups.spec.md
import { describe, expect, it } from "vite-plus/test"
import { getRemovePrefixLabel } from "@/hooks/useCommitMoveList"
import type { Commit, SubjectParts } from "@/lib/bindings"
import { testSubject } from "@/test-support/subject-parts"

/** A commit with this message, its subject split as Rust splits it: `(prefix)` and the issue key of the rest */
function prefixed(message: string, prefix: string | null, issueKey: string | null): Commit {
  const subjectParts: SubjectParts = testSubject("", { prefix, issueKey })
  return { originalHash: message, strippedSubject: message, subjectParts, message, author: "a", authorTime: 1, committerTime: 1 }
}

describe("getRemovePrefixLabel", () => {
  it("names the issue group the commits fall back to", () => {
    const issueGroups = { "ABC-1": "parser" }
    expect(getRemovePrefixLabel([prefixed("(lexer) ABC-1 Add tokens", "lexer", "ABC-1")], issueGroups)).toBe("Move Back to parser")
    expect(getRemovePrefixLabel([prefixed("(lexer) ABC-2 Add tokens", "lexer", "ABC-2")], issueGroups)).toBe("Move Back to ABC-2")
    expect(getRemovePrefixLabel([prefixed("(lexer) ABC-1 Add tokens", "lexer", "ABC-1"), prefixed("(lexer) ABC-2 Add more", "lexer", "ABC-2")], issueGroups)).toBe("Remove Prefix")
    expect(getRemovePrefixLabel([prefixed("(lexer) Add tokens", "lexer", null)], issueGroups)).toBe("Move to Unassigned")
    expect(getRemovePrefixLabel([prefixed("ABC-3 Add grammar", null, "ABC-3")], issueGroups)).toBeNull()
  })

  it("moves a commit of an issue prefix without a key of its own to the unassigned commits", () => {
    // the backend copies `(ABC-1) Add tokens` as `ABC-1 Add tokens`; without the prefix the commit has no key
    expect(getRemovePrefixLabel([prefixed("(ABC-1) Add tokens", "ABC-1", null)])).toBe("Move to Unassigned")
  })

  it("moves the commits that the issue groups put into a branch back to the issue branch of their key", () => {
    const issueGroups = { "ABC-1": "parser", "ABC-2": "parser" }
    expect(getRemovePrefixLabel([prefixed("ABC-1 Add grammar", null, "ABC-1")], issueGroups)).toBe("Move Back to ABC-1")
    expect(getRemovePrefixLabel([prefixed("ABC-1 Add grammar", null, "ABC-1"), prefixed("ABC-2 Add maps", null, "ABC-2")], issueGroups)).toBe("Move Back")
    expect(getRemovePrefixLabel([prefixed("ABC-1 Add grammar", null, "ABC-1"), prefixed("(lexer) ABC-1 Add tokens", "lexer", "ABC-1")], issueGroups)).toBe("Move Back")
  })
})
