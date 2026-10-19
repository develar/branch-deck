// @spec spec/commit/commit-files.spec.md
import { describe, expect, it } from "vite-plus/test"
import { inlineParts, lineParts } from "@/lib/diff/line-parts"
import type { LinePart } from "@/lib/diff/line-parts"

/** The parts as `text:style`, `+` for a changed range, `-` for a deleted part */
function describeParts(text: (part: LinePart) => string, parts: readonly LinePart[]): string[] {
  return parts.map(part => `${text(part)}:${part.style}${part.change >= 0 ? "+" : ""}${part.deleted ? "-" : ""}`)
}

describe("lineParts", () => {
  it("cuts a line at each run of the tokens and each bound of a changed range", () => {
    const line = "val count = 42"
    // a keyword, the default foreground, a number
    const runs = Uint32Array.from([0, 1, 3, 0, 12, 4])
    const parts = lineParts(line.length, runs, [12, 14, 2])
    expect(describeParts(part => line.slice(part.start, part.end), parts)).toEqual(["val:1", " count = :0", "42:4+"])
    expect(describeParts(part => line.slice(part.start, part.end), lineParts(line.length, null, [4, 9, 2]))).toEqual(["val :0", "count:0+", " = 42:0"])
    expect(lineParts(0, null, null)).toEqual([])
  })
})

describe("lineParts with the kinds of the engine", () => {
  it("types each changed part, and puts a marker part where the empty side of a change stands", () => {
    // `call(x)` to `call(x, y)`: the old line marks the insertion after `x`
    const parts = lineParts(7, null, [6, 6, 0])
    expect(parts.map(part => [part.start, part.end, part.change, part.marker === true])).toEqual([
      [0, 6, -1, false],
      [6, 6, 0, true],
      [6, 7, -1, false],
    ])
    expect(lineParts(10, null, [6, 9, 0]).map(part => part.change)).toEqual([-1, 0, -1])
    // a marker at the end of a line
    expect(lineParts(3, null, [3, 3, 1]).at(-1)).toMatchObject({ start: 3, end: 3, change: 1, marker: true })
  })
})

describe("inlineParts", () => {
  it("puts the deleted part of the old line before the inserted part, each with the runs of its side", () => {
    const oldLine = "val a = string"
    const newLine = "val a = name"
    const parts = inlineParts(oldLine.length, Uint32Array.from([8, 2]), [8, 14, 2], newLine.length, Uint32Array.from([8, 6]), [8, 12, 2])
    expect(parts.map(part => `${(part.side === 0 ? oldLine : newLine).slice(part.start, part.end)}:${part.style}${part.deleted ? "-" : part.change >= 0 ? "+" : ""}`)).toEqual([
      "val a = :0",
      "string:2-",
      "name:6+",
    ])
  })

  it("aligns a pure insertion: the old line without a range gets an empty one, so the row is the new line", () => {
    const newLine = "val a = state + 1"
    const parts = inlineParts(13, null, [], newLine.length, null, [13, 17, 0])
    expect(parts.map(part => `${newLine.slice(part.start, part.end)}:${part.side}${part.change >= 0 ? "+" : ""}`)).toEqual(["val a = state:1", " + 1:1+"])
    // the engine marks the empty old side; the row is the same
    const marked = inlineParts(13, null, [13, 13, 0], newLine.length, null, [13, 17, 0])
    expect(marked.map(part => `${newLine.slice(part.start, part.end)}:${part.side}${part.change >= 0 ? "+" : ""}`)).toEqual(["val a = state:1", " + 1:1+"])
  })

  it("gives the new line as it is when the ranges of the two lines do not align", () => {
    const parts = inlineParts(5, null, [0, 1, 2, 2, 3, 2], 5, null, [0, 1, 2])
    expect(parts.every(part => part.side === 1 && !part.deleted)).toBe(true)
  })
})
