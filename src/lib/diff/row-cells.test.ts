// @spec openspec/specs/commit/commit-row/spec.md
import { describe, expect, it } from "vite-plus/test"
import { diffCells, fileLayoutOf, mergeCells, oneSideOf } from "@/lib/diff/row-cells"
import { buildFileRows, buildMergeRows, LEFT, MIDDLE, NEW, OLD, RIGHT, rowAt } from "@/lib/diff/rows"
import type { LineRow } from "@/lib/diff/rows"
import { block, mergeBlock, run, textDiff, textMerge } from "@/test-support/diff-blocks"

const lineRow = (row: ReturnType<typeof rowAt>): LineRow => {
  if (row.kind === "fold" || row.kind === "note") {
    throw new Error("no line row")
  }
  return row
}

describe("mergeCells", () => {
  const conflict = mergeBlock([1, 2], [1, 2], [1, 2], { resolution: "text", resolvedText: "x", leftRanges: [[0, 1]], baseRanges: [[0, 2]], rightRanges: [[2, 3]] })
  const added = mergeBlock([3, 3], [3, 3], [3, 4], { kind: "inserted", leftChanged: false, resolution: "default", rightRanges: [[0, 4]] })
  const merge = textMerge([4, 4, 5], [conflict, added])

  it("colours a block in the middle and on the sides that changed it, and marks a resolved conflict in the result", () => {
    const cells = mergeCells(merge, "result")
    const rows = buildMergeRows(merge, "result")
    const conflictRow = lineRow(rowAt(rows, 1))
    const addedRow = lineRow(rowAt(rows, 3))
    expect(cells.columns).toEqual([LEFT, MIDDLE, RIGHT])
    expect([LEFT, MIDDLE, RIGHT].map(side => cells.cell(conflictRow, side))).toEqual(["conflict", "conflict", "conflict"])
    expect([LEFT, MIDDLE, RIGHT].map(side => cells.cell(addedRow, side))).toEqual(["filler", "inserted", "inserted"])
    expect(cells.cell(lineRow(rowAt(rows, 0)), MIDDLE)).toBe("context")
    expect([cells.resolved(conflictRow, MIDDLE), cells.resolved(conflictRow, LEFT)]).toEqual([true, false])
    // the result of a resolve by words has no ranges; the middle takes the ranges of the side whose lines it shows
    expect([cells.ranges(conflictRow, LEFT), cells.ranges(conflictRow, MIDDLE), cells.ranges(addedRow, MIDDLE)]).toEqual([[0, 1, 2], null, [0, 4, 2]])
    expect(mergeCells(merge, "base").ranges(conflictRow, MIDDLE)).toEqual([0, 2, 2])
    expect(mergeCells(merge, "base").resolved(conflictRow, MIDDLE)).toBe(false)
  })

  it("leaves a side that did not change a block plain", () => {
    const right = mergeBlock([1, 2], [1, 2], [1, 3], { kind: "modified", leftChanged: false, resolution: "default", leftRanges: [[0, 1]] })
    const only = textMerge([3, 3, 4], [right])
    const row = lineRow(rowAt(buildMergeRows(only, "result"), 1))
    const cells = mergeCells(only, "result")
    expect([LEFT, MIDDLE, RIGHT].map(side => cells.cell(row, side))).toEqual(["context", "modified", "modified"])
    expect(cells.ranges(row, LEFT)).toBeNull()
  })
})

describe("diffCells", () => {
  it("gives the old and the new column in Split, one column with the ranges of the line otherwise", () => {
    const diff = textDiff(3, 3, [block(1, 2, 1, 2, { oldRanges: [[0, 1, 2]], newRanges: [[1, 2, 2]] })])
    const split = diffCells(diff, "split")
    const row = lineRow(rowAt(buildFileRows(diff, "split"), 1))
    expect(split.columns).toEqual([OLD, NEW])
    expect([split.cell(row, OLD), split.cell(row, NEW), split.ranges(row, NEW)]).toEqual(["modified", "modified", [1, 2, 2]])
    const unified = diffCells(diff, "unified")
    const added = lineRow(rowAt(buildFileRows(diff, "unified"), 2))
    expect([unified.columns, unified.cell(added, NEW), unified.ranges(added, NEW)]).toEqual([[NEW], "inserted", [1, 2, 2]])
    // Split types a block by its sides, as IntelliJ: modified above, inserted and deleted here; Unified types the lines
    expect([split.typed, unified.typed]).toEqual([true, false])
    const sides = textDiff(4, 4, [block(1, 1, 1, 2), block(2, 3, 3, 3)])
    const splitSides = diffCells(sides, "split")
    const rows = buildFileRows(sides, "split")
    expect([splitSides.cell(lineRow(rowAt(rows, 1)), NEW), splitSides.cell(lineRow(rowAt(rows, 1)), OLD), splitSides.cell(lineRow(rowAt(rows, 3)), OLD)]).toEqual([
      "inserted",
      "filler",
      "deleted",
    ])
    const deletedRow = lineRow(rowAt(buildFileRows(diff, "unified"), 1))
    expect(unified.cell(deletedRow, OLD)).toBe("deleted")
  })

  it("types a line that Inline shows once as its own cell, keeps the type of a quiet line elsewhere, and draws no range in a quiet line", () => {
    // lines 2-3 indented again, a quiet run of formatting whose indentation the engine marks; line 4 really changed
    const ranges = {
      oldRanges: [
        [0, 2, 2],
        [0, 2, 2],
        [4, 5, 2],
      ],
      newRanges: [
        [0, 4, 2],
        [0, 4, 2],
        [4, 5, 2],
      ],
    }
    const diff = textDiff(8, 8, [block(2, 5, 2, 5, { inlinePairs: true, ...ranges })], { runs: [run(2, 4, 2, 4)] })
    const at = (mode: "inline" | "unified" | "split", row: number, open: number[] = []) => lineRow(rowAt(buildFileRows(diff, mode, [], false, null, new Set(open)), row))
    // Inline: the note, then the two lines once
    const inline = diffCells(diff, "inline")
    const once = at("inline", 3)
    expect([once.kind, inline.cell(once, NEW), inline.ranges(once, NEW)]).toEqual(["once", "once", null])
    // the pair after the run is a row of the block, with its ranges
    const pair = at("inline", 5)
    expect([pair.kind, inline.cell(pair, NEW), inline.ranges(pair, OLD), inline.ranges(pair, NEW)]).toEqual(["inline", "inserted", [4, 5, 2], [4, 5, 2]])
    // opened, and in Unified: a deleted and an inserted line, quiet, without their ranges
    for (const [deleted, added] of [
      [at("inline", 3, [2]), at("inline", 5, [2])],
      [at("unified", 3), at("unified", 5)],
    ]) {
      expect([deleted!.tint, inline.cell(deleted!, OLD), inline.ranges(deleted!, OLD)]).toEqual(["quiet", "deleted", null])
      expect([added!.tint, inline.cell(added!, NEW), inline.ranges(added!, NEW)]).toEqual(["quiet", "inserted", null])
    }
    // Split: both sides of the run in the type of the block, quiet, no range; the changed line keeps its typed ranges
    const split = diffCells(diff, "split")
    const quiet = at("split", 3)
    expect([quiet.tint, split.cell(quiet, OLD), split.cell(quiet, NEW), split.ranges(quiet, OLD), split.ranges(quiet, NEW)]).toEqual(["quiet", "modified", "modified", null, null])
    expect(split.ranges(at("split", 5), NEW)).toEqual([4, 5, 2])
  })

  it("shows a file with one side as that side alone in every layout, with its line numbers, as IntelliJ's one-sided viewer", () => {
    // an added file: no old blob; a deleted one: no new blob; a changed one has both
    expect([oneSideOf({ oldId: null, newId: "b" }), oneSideOf({ oldId: "a", newId: null }), oneSideOf({ oldId: "a", newId: "b" })]).toEqual([NEW, OLD, null])
    expect([fileLayoutOf("split", NEW), fileLayoutOf("inline", OLD), fileLayoutOf("split", null)]).toEqual(["unified", "unified", "split"])
    const added = textDiff(0, 3, [block(0, 0, 0, 3)])
    for (const mode of ["split", "unified", "inline"] as const) {
      const cells = diffCells(added, mode, NEW)
      const row = lineRow(rowAt(buildFileRows(added, fileLayoutOf(mode, NEW)), 1))
      expect([cells.columns, cells.numbers, row.kind, row.lines, cells.cell(row, NEW)]).toEqual([[NEW], [NEW], "added", [null, 1], "inserted"])
    }
    const deleted = textDiff(2, 0, [block(0, 2, 0, 0)])
    const cells = diffCells(deleted, "split", OLD)
    const row = lineRow(rowAt(buildFileRows(deleted, fileLayoutOf("split", OLD)), 0))
    expect([cells.columns, cells.numbers, row.kind, cells.cell(row, OLD)]).toEqual([[NEW], [OLD], "deleted", "deleted"])
    // a file with both sides keeps its layout: two columns in Split, both line numbers in one column otherwise
    const changed = textDiff(1, 1, [block(0, 1, 0, 1)])
    expect([diffCells(changed, "split").numbers, diffCells(changed, "unified").numbers]).toEqual([[], [OLD, NEW]])
  })
})
