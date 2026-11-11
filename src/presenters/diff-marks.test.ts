// @spec openspec/specs/commit/quiet-changes/spec.md
import { describe, expect, it } from "vite-plus/test"
import type { LineMove } from "@/lib/bindings"
import { describeMoveEnd, describeToRead, formatHiddenMatches, formatToRead, MOVE_END_LABEL, presentRemark } from "@/presenters/diff-marks"

const lines = (oldStart: number, oldEnd: number, newStart: number, newEnd: number) =>
  [
    [oldStart, oldEnd],
    [newStart, newEnd],
  ] as const

describe("presentRemark", () => {
  it("says what the lines of a run changed in and how far it reaches, by the new lines", () => {
    expect(presentRemark({ kind: "formatting", lines: lines(6, 7, 11, 14), open: null, move: null })).toEqual({
      label: "Formatting only",
      target: null,
      detail: "3 lines",
      name: "Formatting only, 3 lines",
      toggle: null,
    })
    expect(presentRemark({ kind: "comments", lines: lines(27, 28, 36, 37), open: null, move: null })).toMatchObject({ label: "Comments only", detail: "1 line", toggle: null })
    expect(presentRemark({ kind: "renamed", lines: lines(2, 4, 2, 4), open: null, move: null }).label).toBe("Renamed only")
  })

  it("names the button of a note that opens by what a press does to the old lines", () => {
    expect(presentRemark({ kind: "formatting", lines: lines(6, 7, 11, 14), open: false, move: null }).toggle).toBe("Show the old line")
    expect(presentRemark({ kind: "formatting", lines: lines(6, 7, 11, 14), open: true, move: null }).toggle).toBe("Hide the old line")
    expect(presentRemark({ kind: "formatting", lines: lines(2, 42, 3, 43), open: false, move: null })).toMatchObject({
      label: "Formatting only",
      detail: "40 lines",
      toggle: "Show the 40 old lines",
    })
    expect(presentRemark({ kind: "formatting", lines: lines(2, 42, 3, 43), open: true, move: null }).toggle).toBe("Hide the 40 old lines")
  })
})

describe("the words of a move", () => {
  // 13 lines and the blank line before them leave the old lines 119..133 (120 to 133 as a reader counts) for the new
  // lines 209..223
  const move = (extra: Partial<LineMove> = {}): LineMove => ({
    oldStart: 120,
    newStart: 209,
    len: 13,
    indent: 0,
    blanksBefore: 1,
    blanksAfter: 0,
    edited: [],
    oldRanges: [],
    newRanges: [],
    ...extra,
  })

  it("says where the lines that left stand now, by the first line of that end, and what its buttons do", () => {
    const source = (open: boolean | null) => presentRemark({ kind: "moved", lines: lines(119, 133, 300, 300), open, move: { at: "source", move: move(), other: [209, 223] } })
    expect(source(false)).toEqual({
      label: "14 lines moved",
      target: { words: "to line 210", name: "Go to line 210, where the lines stand now" },
      detail: "",
      name: "14 lines moved to line 210",
      toggle: "Show the 14 old lines",
    })
    expect([source(true).toggle, source(null).toggle]).toEqual(["Hide the 14 old lines", null])
    // one line, and the blank lines after the old lines, which stand before the new ones: the first line of the other
    // end as the view has it (`MoveEnd.other`), never counted here from the blank lines of the move
    const one = presentRemark({
      kind: "moved",
      lines: lines(7, 8, 3, 3),
      open: false,
      move: { at: "source", move: move({ oldStart: 7, newStart: 41, len: 1, blanksBefore: 0, blanksAfter: 2 }), other: [39, 42] },
    })
    expect([one.label, one.target?.words, one.toggle]).toEqual(["1 line moved", "to line 40", "Show the old line"])
    // blank lines that reach out of the block of the other end are no lines of it: the place is the line under its note
    const cut = presentRemark({
      kind: "moved",
      lines: lines(7, 8, 3, 3),
      open: false,
      move: { at: "source", move: move({ oldStart: 7, newStart: 41, len: 1, blanksBefore: 0, blanksAfter: 2 }), other: [40, 42] },
    })
    expect([cut.target?.words, cut.target?.name]).toEqual(["to line 41", "Go to line 41, where the lines stand now"])
  })

  it("says where the lines that stand now were, whether they were indented again and how many of them were edited on the way", () => {
    const destination = (extra: Partial<LineMove> = {}) =>
      presentRemark({ kind: "moved", lines: lines(300, 300, 209, 223), open: null, move: { at: "destination", move: move(extra), other: [119, 133] } })
    expect(destination()).toEqual({
      label: "Moved",
      target: { words: "from line 120", name: "Go to line 120, where the lines were" },
      detail: "",
      name: "Moved from line 120",
      toggle: null,
    })
    expect([destination({ indent: 2 }).detail, destination({ indent: -4 }).detail, destination({ edited: [3] }).detail]).toEqual(["re-indented", "re-indented", "1 line edited"])
    expect(destination({ indent: 2, edited: [1, 4] })).toMatchObject({ detail: "re-indented · 2 lines edited", name: "Moved from line 120, re-indented, 2 lines edited" })
  })

  it("names the action that shows the other end of a move, and says where it goes from each end", () => {
    expect([MOVE_END_LABEL, describeMoveEnd("source"), describeMoveEnd("destination")]).toEqual([
      "Go to Other End of Move",
      "Show where the moved lines stand now",
      "Show where the moved lines were",
    ])
  })
})

describe("the words of the find and of what is left to read", () => {
  it("names the hits in the old lines a note stands for", () => {
    expect(formatHiddenMatches(1, 1)).toBe("1 match in the old line")
    expect(formatHiddenMatches(1, 3)).toBe("1 match in the old lines")
    expect(formatHiddenMatches(4, 3)).toBe("4 matches in the old lines")
  })

  it("says how many changes of a file are left to read, short beside its counts and in full for a hover", () => {
    expect([formatToRead(10), formatToRead(1), formatToRead(0)]).toEqual(["10 to read", "1 to read", "Nothing to read"])
    expect([describeToRead(10, 11), describeToRead(1, 2)]).toEqual(["10 of 11 changes to read", "1 of 2 changes to read"])
    expect([describeToRead(0, 3), describeToRead(0, 1)]).toEqual(["None of the 3 changes needs reading", "The change needs no reading"])
  })
})
