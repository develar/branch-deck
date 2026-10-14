// @spec spec/branch/issue-groups.spec.md
import { describe, expect, it } from "vitest"
import { getRemovePrefixLabel } from "@/hooks/useCommitMoveList"
import type { Commit } from "@/lib/bindings"

/** A commit with this message, its subject without the prefix as the backend strips it */
function prefixed(message: string): Commit {
  const strippedSubject = message.replace(/^\([^)]*\)\s*/, "")
  return { originalHash: message, strippedSubject, message, author: "a", authorTime: 1, committerTime: 1 }
}

describe("getRemovePrefixLabel", () => {
  it("names the issue group the commits fall back to", () => {
    const issueGroups = { "ABC-1": "parser" }
    expect(getRemovePrefixLabel([prefixed("(lexer) ABC-1 Add tokens")], issueGroups)).toBe("Move Back to parser")
    expect(getRemovePrefixLabel([prefixed("(lexer) ABC-2 Add tokens")], issueGroups)).toBe("Move Back to ABC-2")
    expect(getRemovePrefixLabel([prefixed("(lexer) ABC-1 Add tokens"), prefixed("(lexer) ABC-2 Add more")], issueGroups)).toBe("Remove Prefix")
    expect(getRemovePrefixLabel([prefixed("(lexer) Add tokens")], issueGroups)).toBe("Move to Unassigned")
    expect(getRemovePrefixLabel([prefixed("ABC-1 Add grammar")], issueGroups)).toBeNull()
  })
})
