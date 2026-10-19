import type { TextDiff } from "@/lib/diff/file-diff-model"
import { resultSourceOf } from "@/lib/diff/file-merge-model"
import type { MergeMiddle, TextMerge } from "@/lib/diff/file-merge-model"
import { LEFT, MIDDLE, NEW, OLD, RIGHT } from "@/lib/diff/rows"
import type { LineRow } from "@/lib/diff/rows"
import type { DiffViewMode } from "@/stores/settings/diff-view-settings"

/*
 * How the painter draws the cells of the rows of a file (docs/design/diff-view.md, "Row elements"): the columns, the
 * kind of each cell and the inner ranges of its line. A diff and a merge give their own.
 */

export interface RowCells {
  /** The sides that show a column; Unified and Inline show one column with both line numbers */
  readonly columns: readonly number[]
  /**
   * The inner ranges take the colour of their own kind (`data-typed`), as the Split of IntelliJ; otherwise the colour
   * of their cell
   */
  readonly typed: boolean
  /**
   * The kind of the cell of a side (`data-cell`): IntelliJ's diff types `inserted`, `deleted`, `modified`,
   * `conflict`, or `context` and `filler`
   */
  cell: (row: LineRow, side: number) => string
  /** The inner ranges of the line of a side, `[start, end, kind, ...]` (`RANGE_STRIDE`); null for none */
  ranges: (row: LineRow, side: number) => readonly number[] | null
  /** The cell shows a conflict that the engine resolves (`data-resolved`) */
  resolved: (row: LineRow, side: number) => boolean
}

const nonEmpty = (ranges: readonly number[] | undefined): readonly number[] | null => (ranges === undefined || ranges.length === 0 ? null : ranges)

/** The untyped `[start, end, ...]` ranges of a merge as typed ranges: a merge block has one kind, its words take it */
function typedMergeRanges(ranges: readonly number[] | undefined): readonly number[] | null {
  if (ranges === undefined || ranges.length === 0) {
    return null
  }
  const typed: number[] = []
  for (let index = 0; index + 1 < ranges.length; index += 2) {
    typed.push(ranges[index]!, ranges[index + 1]!, MODIFIED_RANGE)
  }
  return typed
}

/** `RangeKind::Modified` of the engine */
const MODIFIED_RANGE = 2

/**
 * The cells of a diff: the old and the new side in Split, one column in Unified and Inline. Split colours a block by
 * its type on both sides, as IntelliJ's `SimpleDiffChangeUi`: inserted when it has new lines only, deleted when it has
 * old lines only, modified when it has both. Unified and Inline colour a deleted line deleted and an inserted line
 * inserted (`DiffDrawUtil.createUnifiedChunkHighlighters`).
 */
export function diffCells(diff: TextDiff, mode: DiffViewMode): RowCells {
  const split = mode === "split"
  return {
    columns: split ? [OLD, NEW] : [NEW],
    typed: split,
    cell: (row, side) => {
      if (split) {
        if (row.lines[side] == null) {
          return "filler"
        }
        const block = diff.blocks[row.block]
        if (row.kind !== "changed" || block === undefined) {
          return "context"
        }
        return block.oldStart === block.oldEnd ? "inserted" : block.newStart === block.newEnd ? "deleted" : "modified"
      }
      return row.kind === "deleted" ? "deleted" : row.kind === "added" || row.kind === "inline" ? "inserted" : "context"
    },
    ranges: (row, side) => {
      const block = row.block < 0 ? undefined : diff.blocks[row.block]
      const line = row.lines[side]
      if (block === undefined || block.kind === "ignored" || line == null) {
        return null
      }
      return nonEmpty(side === OLD ? block.oldRanges[line - block.oldStart] : block.newRanges[line - block.newStart])
    },
    resolved: () => false,
  }
}

/**
 * The cells of a merge, as IntelliJ's merge viewer colours them: each block in the colour of its kind on the sides
 * that changed it and in the middle; a side that did not change it stays plain. The middle column shows the ranges of
 * the lines it takes.
 */
export function mergeCells(merge: TextMerge, middle: MergeMiddle): RowCells {
  const changes = (row: LineRow, side: number) => {
    const block = merge.blocks[row.block]
    if (row.kind !== "merge" || block === undefined || row.lines[side] == null) {
      return null
    }
    return side === LEFT ? (block.leftChanged ? block : null) : side === RIGHT ? (block.rightChanged ? block : null) : block
  }
  return {
    columns: [LEFT, MIDDLE, RIGHT],
    // the words of a merge block take the colour of the block
    typed: false,
    cell: (row, side) => {
      if (row.lines[side] == null) {
        return "filler"
      }
      const block = changes(row, side)
      return block === null ? "context" : block.kind
    },
    ranges: (row, side) => {
      const block = changes(row, side)
      if (block === null) {
        return null
      }
      const source = side === LEFT ? "left" : side === RIGHT ? "right" : middle === "base" ? "base" : resultSourceOf(block)
      switch (source) {
        case "left":
          return typedMergeRanges(block.leftRanges[row.blockRow])
        case "right":
          return typedMergeRanges(block.rightRanges[row.blockRow])
        case "base":
          return typedMergeRanges(block.baseRanges[row.blockRow])
        case "text":
          return null
      }
    },
    resolved: (row, side) => {
      const block = changes(row, side)
      return side === MIDDLE && middle === "result" && block?.kind === "conflict" && block.resolution !== null
    },
  }
}
