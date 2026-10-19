import type { DiffBlock, Scope, TextDiff } from "@/lib/diff/file-diff-model"
import { middleLineCount } from "@/lib/diff/file-merge-model"
import type { MergeBlock, MergeMiddle, TextMerge } from "@/lib/diff/file-merge-model"
import type { DiffViewMode } from "@/stores/settings/diff-view-settings"

/*
 * The rows of a file's diff or merge, from the blocks of the engine (docs/design/diff-view.md, "The row model"): a
 * short list of segments, O(blocks) also for a file of 10,000 lines. A row is made only when it is asked for.
 */

/** The unchanged lines next to a change, as IntelliJ's default */
export const CONTEXT_LINES = 4
/** The level of a fold that shows all its lines; the levels below show 4, 8 and 16 lines next to a change */
export const EXPANDED = 3
/** A fold hides at least this many rows: one separator for one line saves nothing */
const MIN_FOLDED_ROWS = 2

/** The sides of a row of a diff: the index into `Row.lines` */
export const OLD = 0
export const NEW = 1
/** The sides of a row of a merge: ours (the target), the middle column (the result or the base), theirs (the commit) */
export const LEFT = 0
export const MIDDLE = 1
export const RIGHT = 2

/** How the rows lay out: the three layouts of a diff, or the three columns of a merge */
export type RowsMode = DiffViewMode | "merge"

/**
 * What a row shows: `context` an unchanged line; `deleted` and `added` the lines of a block in Unified; `inline` a
 * similar pair as one row; `changed` both sides of a block in Split; `merge` the sides of a block of a merge;
 * `ignored` a block the policy ignores, drawn unchanged
 */
export type LineRowKind = "context" | "deleted" | "added" | "inline" | "changed" | "merge" | "ignored"

export interface LineRow {
  kind: LineRowKind
  /** The line index on each side, or null: a gap (Split and a merge fill it), or the old line Unified leaves out */
  lines: readonly (number | null)[]
  /** The index of the block in the blocks of the diff or the merge, -1 for unchanged lines */
  block: number
  /** The row of a merge block from its first one: every side shows line `start + blockRow` of the block */
  blockRow: number
  /**
   * The tint of a changed line, as IntelliJ's `SimpleDiffChangeUi`: `full` for a block without inner ranges, `pale` for
   * a block with them (the ranges carry the full colour), `whitespace` for a block that differs only in whitespace
   */
  tint: LineTint
}

export type LineTint = "full" | "pale" | "whitespace"

export interface FoldRow {
  kind: "fold"
  /** The index of the gap between two changes, the key of its level */
  gap: number
  level: number
  /** The folded lines of each side: [start, end) */
  lines: readonly (readonly [number, number])[]
  /** The innermost scope around the folded lines */
  label: string | null
}

export type Row = LineRow | FoldRow

type Item =
  /** Lines equal on every side, from `starts` */
  | { kind: "equal"; starts: readonly number[]; count: number }
  | { kind: "block"; block: DiffBlock; index: number }
  /** A block of a merge: its lines on each side, from `starts` */
  | { kind: "merge"; block: MergeBlock; index: number; starts: readonly number[]; counts: readonly number[] }

/** Rows `[from, from + count)` of an item, or a fold */
type Segment = { kind: "lines"; item: Item; from: number; count: number } | { kind: "fold"; row: FoldRow }

/** The unchanged rows between two changes, which fold together */
export interface FoldGap {
  /** Its rows when nothing folds */
  rows: number
  changeBefore: boolean
  changeAfter: boolean
}

export interface FileRows {
  readonly mode: RowsMode
  readonly rowCount: number
  /** The gaps between the changes: the levels of their folds are kept by this index */
  readonly gapCount: number
  readonly gaps: readonly FoldGap[]
  readonly segments: readonly Segment[]
  /** The first row of each segment, and the row count at the end */
  readonly segmentStarts: Uint32Array
  /** The first row of each block of changed lines (not an ignored one), in order */
  readonly changeStarts: readonly number[]
}

interface Gap extends FoldGap {
  items: Item[]
}

/** Adds `count` equal lines from `starts` to the items, joined to an equal run that ends there */
function pushEqual(items: Item[], starts: readonly number[], count: number): void {
  if (count <= 0) {
    return
  }
  const last = items.at(-1)
  if (last?.kind === "equal" && last.starts.every((start, side) => start + last.count === starts[side])) {
    last.count += count
  } else {
    items.push({ kind: "equal", starts, count })
  }
}

/**
 * The items of a diff in order: runs of equal lines and the blocks; an ignored block with equal sides is equal text.
 * The empty last line that the engine counts and no block touches is left out.
 */
function itemsOf(diff: TextDiff): Item[] {
  const items: Item[] = []
  let oldLine = 0
  let newLine = 0
  diff.blocks.forEach((block, index) => {
    pushEqual(items, [oldLine, newLine], block.newStart - newLine)
    if (block.kind === "ignored" && block.oldEnd - block.oldStart === block.newEnd - block.newStart) {
      pushEqual(items, [block.oldStart, block.newStart], block.newEnd - block.newStart)
    } else {
      items.push({ kind: "block", block, index })
    }
    oldLine = block.oldEnd
    newLine = block.newEnd
  })
  const trailing = Math.min(diff.oldLineCount - oldLine, diff.newLineCount - newLine)
  pushEqual(items, [oldLine, newLine], diff.emptyLastLine.old && diff.emptyLastLine.new ? trailing - 1 : trailing)
  return items
}

/**
 * The items of a merge in order, its sides left, middle and right. The middle side counts the lines of the result or
 * of the base. Every block is a change, an ignored one too, as in IntelliJ's merge viewer. The empty last line that the
 * engine counts and no block touches is left out.
 */
function mergeItemsOf(merge: TextMerge, middle: MergeMiddle): Item[] {
  const items: Item[] = []
  // the next line of each side
  let lines: readonly number[] = [0, 0, 0]
  let baseLine = 0
  merge.blocks.forEach((block, index) => {
    const equal = block.baseStart - baseLine
    pushEqual(items, lines, equal)
    const starts = [block.leftStart, lines[MIDDLE]! + Math.max(equal, 0), block.rightStart]
    const counts = [block.leftEnd - block.leftStart, middleLineCount(block, middle), block.rightEnd - block.rightStart]
    items.push({ kind: "merge", block, index, starts, counts })
    lines = starts.map((start, side) => start + counts[side]!)
    baseLine = block.baseEnd
  })
  const trailing = merge.baseLineCount - baseLine
  const { left, base, right } = merge.emptyLastLine
  pushEqual(items, lines, left && base && right ? trailing - 1 : trailing)
  return items
}

function isChange(item: Item): boolean {
  return (item.kind === "block" && item.block.kind !== "ignored") || item.kind === "merge"
}

/** The unchanged items between the changes; there is one gap before each change and one after the last */
function gapsOf(items: readonly Item[], mode: RowsMode): { gaps: Gap[]; changes: Item[] } {
  const gaps: Gap[] = [{ items: [], rows: 0, changeBefore: false, changeAfter: false }]
  const changes: Item[] = []
  for (const item of items) {
    if (isChange(item)) {
      gaps.at(-1)!.changeAfter = true
      changes.push(item)
      gaps.push({ items: [], rows: 0, changeBefore: true, changeAfter: false })
    } else {
      const gap = gaps.at(-1)!
      gap.items.push(item)
      gap.rows += itemRowCount(item, mode)
    }
  }
  return { gaps, changes }
}

/** The row count of an item in a mode */
function itemRowCount(item: Item, mode: RowsMode): number {
  if (item.kind === "equal") {
    return item.count
  }
  if (item.kind === "merge") {
    return Math.max(...item.counts)
  }
  const { block } = item
  const deletions = block.oldEnd - block.oldStart
  const additions = block.newEnd - block.newStart
  if (mode === "split") {
    return Math.max(deletions, additions)
  }
  if (block.kind === "ignored" || (mode === "inline" && block.inlinePairs && deletions === additions)) {
    return additions
  }
  return deletions + additions
}

/** Row `row` of an item in a mode */
function itemRow(item: Item, row: number, mode: RowsMode): LineRow {
  if (item.kind === "equal") {
    return { kind: "context", lines: item.starts.map(start => start + row), block: -1, blockRow: 0, tint: "full" }
  }
  if (item.kind === "merge") {
    const { block, index, starts, counts } = item
    const lines = starts.map((start, side) => (row < counts[side]! ? start + row : null))
    return { kind: "merge", lines, block: index, blockRow: row, tint: mergeTintOf(block) }
  }
  const { block, index } = item
  const deletions = block.oldEnd - block.oldStart
  const additions = block.newEnd - block.newStart
  const tint = tintOf(block)
  const at = (kind: LineRowKind, lines: readonly (number | null)[]): LineRow => ({ kind, lines, block: index, blockRow: row, tint })
  if (mode === "split") {
    return at(block.kind === "ignored" ? "ignored" : "changed", [row < deletions ? block.oldStart + row : null, row < additions ? block.newStart + row : null])
  }
  if (block.kind === "ignored") {
    return at("ignored", [null, block.newStart + row])
  }
  if (mode === "inline" && block.inlinePairs && deletions === additions) {
    return at("inline", [block.oldStart + row, block.newStart + row])
  }
  return row < deletions ? at("deleted", [block.oldStart + row, null]) : at("added", [null, block.newStart + row - deletions])
}

/** The tint of the lines of a block: pale where the inner ranges mark what changed */
function tintOf(block: DiffBlock): LineTint {
  if (block.kind === "whitespaceOnly") {
    return "whitespace"
  }
  return block.oldRanges.length > 0 || block.newRanges.length > 0 ? "pale" : "full"
}

/**
 * The tint of the lines of a merge block, as IntelliJ's `DiffViewerHighlighters`: pale for a block with inner ranges,
 * also empty ones, and for an ignored block; the full colour for an insertion or a deletion without them
 */
function mergeTintOf(block: MergeBlock): LineTint {
  return block.ignored || block.leftRanges.length > 0 || block.baseRanges.length > 0 || block.rightRanges.length > 0 ? "pale" : "full"
}

/** The lines of each side that rows `[from, from + count)` of an item cover */
function itemLines(item: Item, from: number, count: number, mode: RowsMode): [number, number][] {
  if (item.kind === "equal") {
    return item.starts.map(start => [start + from, start + from + count])
  }
  const clip = (start: number, length: number): [number, number] => [start + Math.min(from, length), start + Math.min(from + count, length)]
  if (item.kind === "merge") {
    return item.starts.map((start, side) => clip(start, item.counts[side]!))
  }
  const { block } = item
  if (from === 0 && count === itemRowCount(item, mode)) {
    return [
      [block.oldStart, block.oldEnd],
      [block.newStart, block.newEnd],
    ]
  }
  return [clip(block.oldStart, block.oldEnd - block.oldStart), clip(block.newStart, block.newEnd - block.newStart)]
}

/** The context a fold of `level` keeps next to a change */
function contextOf(level: number): number {
  return CONTEXT_LINES << level
}

/** The rows a gap hides at a level */
function foldedRows(gap: FoldGap, level: number): number {
  if (level >= EXPANDED) {
    return 0
  }
  const context = contextOf(level)
  const hidden = gap.rows - (gap.changeBefore ? context : 0) - (gap.changeAfter ? context : 0)
  return hidden >= MIN_FOLDED_ROWS ? hidden : 0
}

/** Rows `[from, to)` of a gap, item by item: the rows of each item it overlaps */
function forEachRows(gap: Gap, from: number, to: number, mode: RowsMode, action: (item: Item, start: number, count: number) => void): void {
  let offset = 0
  for (const item of gap.items) {
    const rows = itemRowCount(item, mode)
    const start = Math.max(from - offset, 0)
    const end = Math.min(to - offset, rows)
    if (start < end) {
      action(item, start, end - start)
    }
    offset += rows
  }
}

/** The scopes of each side, and the order in which a fold label looks at the sides */
interface Labels {
  scopes: readonly (readonly Scope[])[]
  order: readonly number[]
}

/**
 * The rows of a file's diff in a mode. The gaps between the changes fold as IntelliJ's `FoldingModelSupport` nests
 * them: at `levels[gap]` 0, 1 and 2 they keep 4, 8 and 16 lines next to each change, at `EXPANDED` all. A file
 * without a change has no rows.
 */
export function buildFileRows(diff: TextDiff, mode: DiffViewMode, levels: ArrayLike<number> = []): FileRows {
  return rowsOf(itemsOf(diff), mode, levels, { scopes: [diff.oldScopes, diff.newScopes], order: [NEW, OLD] })
}

/** The rows of a file's merge in three columns: left, the middle (`middle`), right. The gaps between the blocks fold as in a diff. */
export function buildMergeRows(merge: TextMerge, middle: MergeMiddle, levels: ArrayLike<number> = []): FileRows {
  // the result has no scopes of its own
  const middleScopes = middle === "base" ? merge.baseScopes : []
  return rowsOf(mergeItemsOf(merge, middle), "merge", levels, { scopes: [merge.leftScopes, middleScopes, merge.rightScopes], order: [MIDDLE, LEFT, RIGHT] })
}

function rowsOf(items: readonly Item[], mode: RowsMode, levels: ArrayLike<number>, labels: Labels): FileRows {
  const { gaps, changes } = gapsOf(items, mode)
  const segments: Segment[] = []
  const starts: number[] = []
  const changeStarts: number[] = []
  let rowCount = 0
  const push = (segment: Segment, rows: number) => {
    segments.push(segment)
    starts.push(rowCount)
    rowCount += rows
  }
  const pushRows = (gap: Gap, from: number, to: number) => {
    forEachRows(gap, from, to, mode, (item, start, count) => push({ kind: "lines", item, from: start, count }, count))
  }

  if (changes.length > 0) {
    gaps.forEach((gap, index) => {
      const level = Math.min(levels[index] ?? 0, EXPANDED)
      const hidden = foldedRows(gap, level)
      if (hidden === 0) {
        pushRows(gap, 0, gap.rows)
      } else {
        const before = gap.changeBefore ? contextOf(level) : 0
        pushRows(gap, 0, before)
        push({ kind: "fold", row: foldRow(labels, gap, index, level, before, before + hidden, mode) }, 1)
        pushRows(gap, before + hidden, gap.rows)
      }
      const change = changes[index]
      if (change !== undefined) {
        changeStarts.push(rowCount)
        push({ kind: "lines", item: change, from: 0, count: itemRowCount(change, mode) }, itemRowCount(change, mode))
      }
    })
  }
  starts.push(rowCount)
  const foldGaps = gaps.map(({ rows, changeBefore, changeAfter }) => ({ rows, changeBefore, changeAfter }))
  return { mode, rowCount, gapCount: gaps.length, gaps: foldGaps, segments, segmentStarts: Uint32Array.from(starts), changeStarts }
}

function foldRow(labels: Labels, gap: Gap, index: number, level: number, from: number, to: number, mode: RowsMode): FoldRow {
  let ranges: [number, number][] | undefined
  forEachRows(gap, from, to, mode, (item, start, count) => {
    const lines = itemLines(item, start, count, mode)
    ranges = ranges === undefined ? lines : ranges.map((range, side) => [range[0], lines[side]![1]])
  })
  const lines = ranges ?? labels.scopes.map((): [number, number] => [0, 0])
  // a scope around all the folded lines, the sides in their order; else one around the last folded line
  const around = (start: (side: number) => number) =>
    labels.order.reduce<Scope | undefined>((found, side) => found ?? innermostScope(labels.scopes[side] ?? [], start(side), lines[side]![1]), undefined)
  const label = around(side => lines[side]![0]) ?? around(side => lines[side]![1] - 1)
  return { kind: "fold", gap: index, level, lines, label: label?.label ?? null }
}

/** The smallest scope around the lines `[start, end)` */
function innermostScope(scopes: readonly Scope[], start: number, end: number): Scope | undefined {
  let best: Scope | undefined
  for (const scope of scopes) {
    if (scope.startLine <= start && scope.endLine >= end && (best === undefined || scope.endLine - scope.startLine < best.endLine - best.startLine)) {
      best = scope
    }
  }
  return best
}

/** The index of the segment that holds row `row` */
function segmentAt(rows: FileRows, row: number): number {
  let low = 0
  let high = rows.segments.length - 1
  while (low < high) {
    const middle = (low + high + 1) >> 1
    if (rows.segmentStarts[middle]! <= row) {
      low = middle
    } else {
      high = middle - 1
    }
  }
  return low
}

/** Row `row` of a file, made when it is asked for */
export function rowAt(rows: FileRows, row: number): Row {
  const index = segmentAt(rows, row)
  const segment = rows.segments[index]!
  return segment.kind === "fold" ? segment.row : itemRow(segment.item, segment.from + row - rows.segmentStarts[index]!, rows.mode)
}

/** The row of an item where `line` of a side, or a later line of it, shows; the row count when no row does */
function itemRowOfLine(item: Item, side: number, line: number, mode: RowsMode): number {
  if (item.kind === "equal") {
    return Math.max(line - item.starts[side]!, 0)
  }
  if (item.kind === "merge") {
    const start = item.starts[side]!
    return line >= start + item.counts[side]! ? itemRowCount(item, mode) : Math.max(line - start, 0)
  }
  const { block } = item
  const deletions = block.oldEnd - block.oldStart
  const start = side === OLD ? block.oldStart : block.newStart
  const count = side === OLD ? deletions : block.newEnd - block.newStart
  if (line >= start + count) {
    return itemRowCount(item, mode)
  }
  const offset = Math.max(line - start, 0)
  if (mode === "split" || (side === NEW && itemRowCount(item, mode) === count)) {
    return offset
  }
  if (block.kind === "ignored") {
    // Unified leaves the old lines of an ignored block out: its first row stands for them
    return 0
  }
  return side === OLD ? offset : deletions + offset
}

/**
 * The row that shows `line` of a side, or the first row after it: the anchor of the scroll position across a new
 * layout. A folded line gives its fold.
 */
export function rowOfLine(rows: FileRows, side: number, line: number): number {
  for (const [index, segment] of rows.segments.entries()) {
    const start = rows.segmentStarts[index]!
    if (segment.kind === "fold") {
      if (line < segment.row.lines[side]![1]) {
        return start
      }
    } else {
      const offset = itemRowOfLine(segment.item, side, line, rows.mode)
      if (offset < segment.from + segment.count) {
        return start + Math.max(offset - segment.from, 0)
      }
    }
  }
  return Math.max(rows.rowCount - 1, 0)
}

/**
 * The level after a click on the fold of a gap: the next one that hides fewer rows, so a level that shows no more
 * lines than the one before it is skipped
 */
export function nextFoldLevel(rows: FileRows, gap: number, level: number): number {
  const target = rows.gaps[gap]
  if (target === undefined) {
    return EXPANDED
  }
  const hidden = foldedRows(target, level)
  let next = level + 1
  while (next < EXPANDED && foldedRows(target, next) >= hidden) {
    next++
  }
  return Math.min(next, EXPANDED)
}

// the levels of the folds of a diff or a merge while the store keeps it, so the lines it expanded stay expanded
const foldLevels = new WeakMap<TextDiff | TextMerge, readonly number[]>()

/** The levels of the folds of a diff or a merge, by gap; a missing one is 0 */
export function getFoldLevels(model: TextDiff | TextMerge): readonly number[] {
  return foldLevels.get(model) ?? []
}

export function setFoldLevels(model: TextDiff | TextMerge, levels: readonly number[]): void {
  foldLevels.set(model, levels)
}
