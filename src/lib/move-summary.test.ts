// @spec spec/branch/move-commits.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { MoveChange, MoveCommitsResult } from "@/lib/bindings"
import { countMoved, describeDoneMove, describeMove, formatKeys } from "@/lib/move-summary"

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

  it("titles the toast of a move with what it did and describes only the rest, never the title again", () => {
    // Ungroup of a branch of one commit: the removal of its prefix is the title alone
    expect(describeDoneMove(result(["PrefixRemoved"]), null, 1)).toEqual({ title: "Removed the prefix of 1 commit", description: undefined })
    expect(describeDoneMove(result(["PrefixRemoved", "PrefixRemoved", "Unchanged"]), null, 2)).toEqual({ title: "Removed the prefix of 2 commits", description: undefined })
    expect(describeDoneMove(result(["Regrouped", "PrefixRemoved"], [], ["IJPL-1"]), null, 2)).toEqual({
      title: "Moved 2 commits back",
      description: "IJPL-1 is back in its issue branch, without a rewrite; removed the prefix of 1 commit",
    })
    expect(describeDoneMove(result(["Prefixed", "PrefixRemoved"]), "maps", 2)).toEqual({
      title: "Moved 2 commits to maps",
      description: "1 commit got the prefix (maps); removed the prefix of 1 commit",
    })
  })
})
