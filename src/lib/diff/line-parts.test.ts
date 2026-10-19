// @spec spec/commit/commit-files.spec.md
import { describe, expect, it } from "vite-plus/test"
import { inlineParts, lineParts } from "@/lib/diff/line-parts"
import type { LinePart } from "@/lib/diff/line-parts"

/** The parts as `text:style`, `+` for a changed range, `-` for a deleted part */
function describeParts(text: (part: LinePart) => string, parts: readonly LinePart[]): string[] {
  return parts.map(part => `${text(part)}:${part.style}${part.changed ? "+" : ""}${part.deleted ? "-" : ""}`)
}

describe("lineParts", () => {
  it("cuts a line at each run of the tokens and each bound of a changed range", () => {
    const line = "val count = 42"
    // a keyword, the default foreground, a number
    const runs = Uint32Array.from([0, 1, 3, 0, 12, 4])
    const parts = lineParts(line.length, runs, [12, 14])
    expect(describeParts(part => line.slice(part.start, part.end), parts)).toEqual(["val:1", " count = :0", "42:4+"])
    expect(describeParts(part => line.slice(part.start, part.end), lineParts(line.length, null, [4, 9]))).toEqual(["val :0", "count:0+", " = 42:0"])
    expect(lineParts(0, null, null)).toEqual([])
  })
})

describe("inlineParts", () => {
  it("puts the deleted part of the old line before the inserted part, each with the runs of its side", () => {
    const oldLine = "val a = string"
    const newLine = "val a = name"
    const parts = inlineParts(oldLine.length, Uint32Array.from([8, 2]), [8, 14], newLine.length, Uint32Array.from([8, 6]), [8, 12])
    expect(parts.map(part => `${(part.side === 0 ? oldLine : newLine).slice(part.start, part.end)}:${part.style}${part.deleted ? "-" : part.changed ? "+" : ""}`)).toEqual([
      "val a = :0",
      "string:2-",
      "name:6+",
    ])
  })

  it("aligns a pure insertion: the old line without a range gets an empty one, so the row is the new line", () => {
    const newLine = "val a = state + 1"
    const parts = inlineParts(13, null, [], newLine.length, null, [13, 17])
    expect(parts.map(part => `${newLine.slice(part.start, part.end)}:${part.side}${part.changed ? "+" : ""}`)).toEqual(["val a = state:1", " + 1:1+"])
  })

  it("gives the new line as it is when the ranges of the two lines do not align", () => {
    const parts = inlineParts(5, null, [0, 1, 2, 3], 5, null, [0, 1])
    expect(parts.every(part => part.side === 1 && !part.deleted)).toBe(true)
  })
})
