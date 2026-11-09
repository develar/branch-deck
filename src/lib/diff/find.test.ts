// @spec openspec/specs/commit/commit-review/spec.md
import { describe, expect, it } from "vite-plus/test"
import type { CopyFile } from "@/lib/diff/copy-text"
import { findHits, findInSegments, hitsByLine, lineKey, mergeHits } from "@/lib/diff/find"
import type { DiffFindHit, TextFindHit } from "@/lib/diff/find"
import { buildLineTable } from "@/lib/diff/line-table"
import { buildFileRows, NEW, OLD } from "@/lib/diff/rows"
import type { DiffViewMode } from "@/stores/settings/diff-view-settings"
import { block, textDiff } from "@/test-support/diff-blocks"

const OLD_TEXT = Array.from({ length: 30 }, (_, index) => (index === 20 ? "val Total = 1" : `val total${index} = ${index}`)).join("\n")
const NEW_TEXT = OLD_TEXT.replace("val Total = 1", "val total = 2")

function file(mode: DiffViewMode): CopyFile {
  return { rows: buildFileRows(textDiff(30, 30, [block(20, 21, 20, 21)]), mode), tables: [buildLineTable(OLD_TEXT), buildLineTable(NEW_TEXT)] }
}

describe("findHits", () => {
  it("finds the query in the lines the rows show, ignoring case, in the order of the rows", () => {
    // Unified: the context 16-19, -20, +20, 21-24; the folds are not searched
    const hits = findHits([file("unified")], "TOTAL", { sideBySide: false, folded: false })
    expect(hits.map(hit => `${hit.side === OLD ? "-" : "+"}${hit.line}`)).toEqual(["+16", "+17", "+18", "+19", "-20", "+20", "+21", "+22", "+23", "+24"])
    expect(hits[4]).toEqual({ kind: "diff", file: 0, side: OLD, line: 20, start: 4, end: 9, gap: -1 })
  })

  it("searches both sides of a change in Split and the new side of an unchanged line, and the folds on request", () => {
    const split = findHits([file("split")], "total", { sideBySide: true, folded: false })
    expect(split.filter(hit => hit.line === 20).map(hit => hit.side)).toEqual([OLD, NEW])
    expect(split.filter(hit => hit.line === 19).map(hit => hit.side)).toEqual([NEW])
    const folded = findHits([file("split")], "total3 ", { sideBySide: true, folded: true })
    // line 3 lies in the first fold, the gap before the change
    expect(folded).toEqual([{ kind: "diff", file: 0, side: NEW, line: 3, start: 4, end: 11, gap: 0 }])
    expect(findHits([file("split")], "", { sideBySide: true, folded: true })).toEqual([])
  })

  it("groups the hits by file and line for the painter, the current one marked", () => {
    const hits = findHits([file("unified")], "t", { sideBySide: false, folded: false }).filter(hit => hit.line === 16)
    const byLine = hitsByLine(hits, 1)
    expect(byLine.get(0)?.get(lineKey(NEW, 16))).toEqual({ ranges: [4, 5, 6, 7], current: 1 })
  })
})

describe("findInSegments", () => {
  it("matches ignoring case and finds several hits in one segment", () => {
    expect(findInSegments(["Total total TOTAL"], "tOtAl")).toEqual([
      { segment: 0, start: 0, end: 5 },
      { segment: 0, start: 6, end: 11 },
      { segment: 0, start: 12, end: 17 },
    ])
  })

  it("does not match across segments and returns no hits for an empty query", () => {
    expect(findInSegments(["over", "view"], "overview")).toEqual([])
    expect(findInSegments(["overview"], "")).toEqual([])
  })
})

describe("mergeHits", () => {
  it("puts text before diff hits for each file and keeps each list's order", () => {
    const textHit = (file: number): TextFindHit => ({ kind: "text", file, range: {} as unknown as Range, folds: [] })
    const diffHit = (file: number, line: number): DiffFindHit => ({ kind: "diff", file, side: NEW, line, start: 0, end: 1, gap: -1 })
    const prologue = textHit(-1)
    const opening = [textHit(0), textHit(0)]
    const epilogue = textHit(2)
    const rows = [diffHit(0, 1), diffHit(0, 2), diffHit(1, 3)]
    const hits = mergeHits([prologue, ...opening, epilogue], rows)

    expect(hits).toEqual([prologue, ...opening, ...rows, epilogue])
  })
})
