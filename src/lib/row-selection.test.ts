// @spec spec/branch/move-commits.spec.md
import { describe, expect, it } from "vite-plus/test"
import { clickRow, leadOf, NO_SELECTION } from "@/lib/row-selection"
import type { RowClick, SelectionState } from "@/lib/row-selection"

const rows = ["a", "b", "c", "d", "e"]

function click(id: string, modifiers: Partial<Pick<RowClick, "toggle" | "range">> = {}): (state: SelectionState) => SelectionState {
  return state => clickRow(state, { id, index: rows.indexOf(id), toggle: modifiers.toggle ?? false, range: modifiers.range ?? false }, rows)
}

function after(...clicks: ((state: SelectionState) => SelectionState)[]): SelectionState {
  return clicks.reduce((state, next) => next(state), NO_SELECTION)
}

function selected(state: SelectionState): string[] {
  return rows.filter(id => state.ids.has(id))
}

describe("clickRow", () => {
  it("ranges every Shift+click from the row of the last plain click, and leads with the row clicked last", () => {
    const state = after(click("b"), click("d", { range: true }), click("a", { range: true }))
    expect(selected(state)).toEqual(["a", "b"])
    expect(state.anchor).toBe("b")
    expect(state.lead).toBe("a")
    expect(leadOf(state, rows)).toBe("a")
  })

  it("adds a range to the selection with Cmd/Ctrl+Shift and moves the anchor with a Cmd/Ctrl click", () => {
    const state = after(click("a"), click("d", { toggle: true }), click("e", { toggle: true, range: true }))
    expect(selected(state)).toEqual(["a", "d", "e"])
    expect(state.anchor).toBe("d")
    expect(leadOf(state, rows)).toBe("e")
  })
})

describe("leadOf", () => {
  it("is the selected row nearest to the lead once a Cmd/Ctrl click took the lead out, the earlier of two as near", () => {
    expect(leadOf(after(click("b"), click("d", { toggle: true }), click("c", { toggle: true }), click("c", { toggle: true })), rows)).toBe("b")
    expect(leadOf(after(click("a"), click("e", { toggle: true }), click("d", { toggle: true }), click("d", { toggle: true })), rows)).toBe("e")
  })

  it("is the first selected row of a selection made by its ids, and nothing without a selection", () => {
    expect(leadOf({ ids: new Set(["d", "b"]), anchor: null, lead: null }, rows)).toBe("b")
    // a lead that left the list (the commit moved away)
    expect(leadOf({ ids: new Set(["c"]), anchor: "x", lead: "x" }, rows)).toBe("c")
    expect(leadOf(after(click("b"), click("b", { toggle: true })), rows)).toBeNull()
  })
})
