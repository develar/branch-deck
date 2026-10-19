// @spec spec/commit/commit-row.spec.md
import { describe, expect, it } from "vite-plus/test"
import { diffCells, mergeCells } from "@/lib/diff/row-cells"
import { buildFileRows, buildMergeRows, LEFT, MIDDLE, NEW, OLD, RIGHT, rowAt } from "@/lib/diff/rows"
import type { LineRow } from "@/lib/diff/rows"
import { block, mergeBlock, textDiff, textMerge } from "@/test-support/diff-blocks"

const lineRow = (row: ReturnType<typeof rowAt>): LineRow => {
  if (row.kind === "fold") {
    throw new Error("a fold")
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
    expect([LEFT, MIDDLE, RIGHT].map(side => cells.cell(conflictRow, side))).toEqual(["merge-conflict", "merge-conflict", "merge-conflict"])
    expect([LEFT, MIDDLE, RIGHT].map(side => cells.cell(addedRow, side))).toEqual(["filler", "merge-inserted", "merge-inserted"])
    expect(cells.cell(lineRow(rowAt(rows, 0)), MIDDLE)).toBe("context")
    expect([cells.resolved(conflictRow, MIDDLE), cells.resolved(conflictRow, LEFT)]).toEqual([true, false])
    // the result of a resolve by words has no ranges; the middle takes the ranges of the side whose lines it shows
    expect([cells.ranges(conflictRow, LEFT), cells.ranges(conflictRow, MIDDLE), cells.ranges(addedRow, MIDDLE)]).toEqual([[0, 1], null, [0, 4]])
    expect(mergeCells(merge, "base").ranges(conflictRow, MIDDLE)).toEqual([0, 2])
    expect(mergeCells(merge, "base").resolved(conflictRow, MIDDLE)).toBe(false)
  })

  it("leaves a side that did not change a block plain", () => {
    const right = mergeBlock([1, 2], [1, 2], [1, 3], { kind: "modified", leftChanged: false, resolution: "default", leftRanges: [[0, 1]] })
    const only = textMerge([3, 3, 4], [right])
    const row = lineRow(rowAt(buildMergeRows(only, "result"), 1))
    const cells = mergeCells(only, "result")
    expect([LEFT, MIDDLE, RIGHT].map(side => cells.cell(row, side))).toEqual(["context", "merge-modified", "merge-modified"])
    expect(cells.ranges(row, LEFT)).toBeNull()
  })
})

describe("diffCells", () => {
  it("gives the old and the new column in Split, one column with the ranges of the line otherwise", () => {
    const diff = textDiff(3, 3, [block(1, 2, 1, 2, { oldRanges: [[0, 1]], newRanges: [[1, 2]] })])
    const split = diffCells(diff, "split")
    const row = lineRow(rowAt(buildFileRows(diff, "split"), 1))
    expect(split.columns).toEqual([OLD, NEW])
    expect([split.cell(row, OLD), split.cell(row, NEW), split.ranges(row, NEW)]).toEqual(["deleted", "added", [1, 2]])
    const unified = diffCells(diff, "unified")
    const added = lineRow(rowAt(buildFileRows(diff, "unified"), 2))
    expect([unified.columns, unified.cell(added, NEW), unified.ranges(added, NEW)]).toEqual([[NEW], "added", [1, 2]])
  })
})
