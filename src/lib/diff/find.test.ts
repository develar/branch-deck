// @spec spec/commit/commit-review.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { CopyFile } from "@/lib/diff/copy-text"
import { findHits, hitsByLine, lineKey } from "@/lib/diff/find"
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
    expect(hits[4]).toEqual({ file: 0, side: OLD, line: 20, start: 4, end: 9, gap: -1 })
  })

  it("searches both sides of a change in Split and the new side of an unchanged line, and the folds on request", () => {
    const split = findHits([file("split")], "total", { sideBySide: true, folded: false })
    expect(split.filter(hit => hit.line === 20).map(hit => hit.side)).toEqual([OLD, NEW])
    expect(split.filter(hit => hit.line === 19).map(hit => hit.side)).toEqual([NEW])
    const folded = findHits([file("split")], "total3 ", { sideBySide: true, folded: true })
    // line 3 lies in the first fold, the gap before the change
    expect(folded).toEqual([{ file: 0, side: NEW, line: 3, start: 4, end: 11, gap: 0 }])
    expect(findHits([file("split")], "", { sideBySide: true, folded: true })).toEqual([])
  })

  it("groups the hits by file and line for the painter, the current one marked", () => {
    const hits = findHits([file("unified")], "t", { sideBySide: false, folded: false }).filter(hit => hit.line === 16)
    const byLine = hitsByLine(hits, 1)
    expect(byLine.get(0)?.get(lineKey(NEW, 16))).toEqual({ ranges: [4, 5, 6, 7], current: 1 })
  })
})
