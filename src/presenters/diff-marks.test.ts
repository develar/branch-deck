// @spec openspec/specs/commit/quiet-changes/spec.md
import { describe, expect, it } from "vite-plus/test"
import { describeToRead, formatHiddenMatches, formatToRead, presentRemark } from "@/presenters/diff-marks"

const lines = (oldStart: number, oldEnd: number, newStart: number, newEnd: number) =>
  [
    [oldStart, oldEnd],
    [newStart, newEnd],
  ] as const

describe("presentRemark", () => {
  it("says what the lines of a run changed in and how far it reaches, by the new lines", () => {
    expect(presentRemark({ kind: "formatting", lines: lines(6, 7, 11, 14), open: null })).toEqual({ label: "Formatting only", detail: "3 lines", toggle: null })
    expect(presentRemark({ kind: "comments", lines: lines(27, 28, 36, 37), open: null })).toEqual({ label: "Comments only", detail: "1 line", toggle: null })
    expect(presentRemark({ kind: "renamed", lines: lines(2, 4, 2, 4), open: null }).label).toBe("Renamed only")
  })

  it("names the button of a note that opens by what a press does to the old lines", () => {
    expect(presentRemark({ kind: "formatting", lines: lines(6, 7, 11, 14), open: false }).toggle).toBe("Show the old line")
    expect(presentRemark({ kind: "formatting", lines: lines(6, 7, 11, 14), open: true }).toggle).toBe("Hide the old line")
    expect(presentRemark({ kind: "formatting", lines: lines(2, 42, 3, 43), open: false })).toEqual({
      label: "Formatting only",
      detail: "40 lines",
      toggle: "Show the 40 old lines",
    })
    expect(presentRemark({ kind: "formatting", lines: lines(2, 42, 3, 43), open: true }).toggle).toBe("Hide the 40 old lines")
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
