import { describe, expect, it } from "vitest"
import { neighbourRow, parentRow } from "@/lib/branch-row-keys"
import type { BranchTableRow } from "@/stores/branch-sync"

function branchRow(name: string, depth: number): BranchTableRow {
  return { kind: "branch", key: name, name, depth, showIssueKey: false, familyIndent: 0, lanes: null, dimmed: false }
}

describe("branch row keys", () => {
  it("moves to the next or the previous branch row, from the table to the first or the last, and stops at the ends", () => {
    const names = ["auth", "login", "docs"]
    expect(neighbourRow(names, "auth", 1)).toBe("login")
    expect(neighbourRow(names, "login", -1)).toBe("auth")
    expect(neighbourRow(names, null, 1)).toBe("auth")
    expect(neighbourRow(names, null, -1)).toBe("docs")
    // a row the filter hid
    expect(neighbourRow(names, "gone", 1)).toBe("auth")
    expect(neighbourRow(names, "docs", 1)).toBeNull()
    expect(neighbourRow(names, "auth", -1)).toBeNull()
    expect(neighbourRow([], null, 1)).toBeNull()
  })

  it("moves from a row to the branch it is nested under, as in a tree", () => {
    // auth ← session (a side child) and auth ← login ← oauth; docs at the top level
    const rows = [branchRow("docs", 0), branchRow("auth", 0), branchRow("session", 1), branchRow("login", 1), branchRow("oauth", 2)]
    expect(parentRow(rows, "oauth")).toBe("login")
    // past the side child to the parent of both
    expect(parentRow(rows, "login")).toBe("auth")
    expect(parentRow(rows, "session")).toBe("auth")
    expect(parentRow(rows, "auth")).toBeNull()
    expect(parentRow(rows, "gone")).toBeNull()
  })

  it("has no parent row under an issue row", () => {
    const rows: BranchTableRow[] = [{ kind: "issue", key: "i:2:ABC-1", issueKey: "ABC-1", branchCount: 2 }, branchRow("parser", 1), branchRow("lexer", 1)]
    expect(parentRow(rows, "lexer")).toBeNull()
  })
})
