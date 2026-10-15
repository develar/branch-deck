// @spec spec/branch/move-commits.spec.md
import { describe, expect, it } from "vitest"
import { inTableOrder, pruneSelection, selectBranchRange, toggleBranch } from "@/lib/branch-selection"

describe("branch selection", () => {
  const order = ["auth", "IJPL-1", "IJPL-2", "docs"]

  it("toggles a branch", () => {
    expect(toggleBranch(["auth"], "docs")).toEqual(["auth", "docs"])
    expect(toggleBranch(["auth", "docs"], "auth")).toEqual(["docs"])
  })

  it("selects a range of rows from the anchor, in table order", () => {
    expect(selectBranchRange(order, "docs", "IJPL-1")).toEqual(["IJPL-1", "IJPL-2", "docs"])
    expect(selectBranchRange(order, "auth", "auth")).toEqual(["auth"])
    // an anchor that is no row any more
    expect(selectBranchRange(order, "gone", "IJPL-2")).toEqual(["IJPL-2"])
  })

  it("prunes the branches that are gone after a sync and keeps the array otherwise", () => {
    const selected = ["IJPL-1", "IJPL-2"]
    expect(pruneSelection(selected, new Set(order))).toBe(selected)
    expect(pruneSelection(selected, new Set(["IJPL-2", "maps"]))).toEqual(["IJPL-2"])
  })

  it("orders the selection as the table", () => {
    expect(inTableOrder(["docs", "auth"], order)).toEqual(["auth", "docs"])
  })
})
