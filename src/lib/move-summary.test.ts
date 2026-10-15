// @spec spec/branch/move-commits.spec.md
import { describe, expect, it } from "vitest"
import type { MoveChange, MoveCommitsResult } from "@/lib/bindings"
import { countMoved, describeMove, formatKeys } from "@/lib/move-summary"

function result(changes: MoveChange[], groupedIssues: string[] = [], ungroupedIssues: string[] = []): MoveCommitsResult {
  return {
    commits: changes.map((change, index) => ({ oldId: `c${index}`, newId: `n${index}`, change })),
    groupedIssues,
    ungroupedIssues,
    head: "h",
    orphanedChildren: [],
  }
}

describe("move summary", () => {
  it("joins issue keys", () => {
    expect(formatKeys(["IJPL-1"])).toBe("IJPL-1")
    expect(formatKeys(["IJPL-1", "IJPL-2", "IJPL-3"])).toBe("IJPL-1, IJPL-2 and IJPL-3")
  })

  it("says which issues go by their key and which commits get the prefix", () => {
    const combined = result(["Regrouped", "Regrouped", "Prefixed", "Unchanged"], ["IJPL-1", "IJPL-2"])
    expect(countMoved(combined)).toBe(3)
    expect(describeMove(combined, "maps", false)).toBe("IJPL-1 and IJPL-2 go to maps by the issue key from now on, without a rewrite; rewrites 1 commit to start with (maps)")
    expect(describeMove(combined, "maps", true)).toBe("IJPL-1 and IJPL-2 go to maps by the issue key from now on, without a rewrite; 1 commit got the prefix (maps)")
  })

  it("says what a move back does", () => {
    expect(describeMove(result(["Regrouped"], [], ["IJPL-1"]), null, true)).toBe("IJPL-1 is back in its issue branch, without a rewrite")
    expect(describeMove(result(["PrefixRemoved", "PrefixRemoved"]), "ABC-1", false)).toBe("removes the prefix of 2 commits")
    expect(describeMove(result(["Unchanged"]), "maps", false)).toBeUndefined()
  })
})
