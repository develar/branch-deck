import {
  blockParts,
  drawnMarks,
  isChangeBlock,
  marksByBlock,
  NO_MARKS,
  partsLines,
  partsMoveNotes,
  partsNotes,
  partsRow,
  partsRowOfLine,
  partsStopRow,
  stopsOf,
} from "@/lib/diff/block-parts"
import type { BlockParts, MoveEnd, Remark } from "@/lib/diff/block-parts"
import type { DiffBlock, RunKind, Scope, TextDiff } from "@/lib/diff/file-diff-model"
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
 * `ignored` a block the policy ignores, drawn unchanged; `once` a line that Inline shows once, the new line standing
 * for the old lines too (a quiet run of the marks, `block-parts.ts`). A line of a move is a row of these kinds with
 * its `move`: `deleted` where it left, `added` where it stands now, `inline` an edited one there in Inline
 */
export type LineRowKind = "context" | "deleted" | "added" | "inline" | "changed" | "merge" | "ignored" | "once"

export interface LineRow {
  kind: LineRowKind
  /** The line index on each side, or null: a gap (Split and a merge fill it), or the old line Unified leaves out */
  lines: readonly (number | null)[]
  /** The index of the block in the blocks of the diff or the merge, -1 for unchanged lines */
  block: number
  /** The row of a merge block from its first one: every side shows line `start + blockRow` of the block */
  blockRow: number
  /** The rows of the block in this mode; 0 for unchanged lines */
  blockRows: number
  /**
   * The tint of a changed line, as IntelliJ's `SimpleDiffChangeUi`: `full` for a block without inner ranges, `pale` for
   * a block with them (the ranges carry the full colour), `whitespace` for a block that differs only in whitespace;
   * our own `quiet` for a line of a quiet run of the marks, which is played down
   */
  tint: LineTint
  /** The kind of the quiet run of the marks the line is of (`LineRun.kind`): nothing of it to read; null for any other line */
  run: RunKind | null
  /**
   * The end of the move the line is of (`TextDiff.marks.moves`): where it left or where it stands now; drawn in the
   * colour of a move, pale (`tint`) where the line was edited on the way, with the ranges of the move. Null for a line
   * of no move
   */
  move: MoveEnd | null
}

export type LineTint = "full" | "pale" | "whitespace" | "quiet"

export interface FoldRow {
  kind: "fold"
  /** The index of the gap between two changes, the key of its level */
  gap: number
  level: number
  /** The folded lines of each side: [start, end) */
  lines: readonly (readonly [number, number])[]
  /** The innermost scope around the folded lines */
  label: string | null
  /** The unchanged lines it hides: neither a block another section shows nor one the whitespace policy hides */
  unchanged: number
  /** The blocks another section shows that it hides, each with where (`SectionUnits.elsewhere`: "Shown in part 2") */
  elsewhere: readonly { readonly block: number; readonly label: string }[]
  /** The changed lines it hides that the whitespace policy ignores: of each ignored block the most of a side */
  ignored: number
}

/**
 * A change of the heading of a block: the name of its declaration, set in the code font, and the words of what the
 * change did to it, in the font of the text ("open" and "body changed"); a heading of no declaration its words alone
 * ("Changes outside a declaration"), a declaration of no words its name alone
 */
export interface HeadingPart {
  readonly name: string | null
  readonly words: string
}

/** The heading of a block, a part for each change it starts */
export type Heading = readonly HeadingPart[]

/** The heading as one line of text: "open — body changed · close — new", what a screen reader and a test read */
export function headingText(heading: Heading): string {
  return heading.map(part => (part.name === null ? part.words : part.words === "" ? part.name : `${part.name} — ${part.words}`)).join(" · ")
}

/**
 * A row of words between the lines of a diff: `unit` the heading of the change a block starts in a section of a review
 * (`SectionUnits.marks`, its parts in `heading`), `elsewhere` a block another section shows (`SectionUnits.elsewhere`),
 * which a click opens, `remark` what the diff says of the lines after it (a run of `TextDiff.marks` or an end of a
 * move, in `remark`). A heading whose block starts with a run or an end of a move says the remark too, in one row
 */
export interface NoteRow {
  kind: "note"
  tone: "unit" | "elsewhere" | "remark"
  /** Its words as one line (`headingText` of a heading); a remark is worded by its presenter (`presentRemark`) */
  label: string
  /** Of a heading, its parts: the names in the code font, the words in the font of the text */
  heading: Heading | null
  /** The block it stands before or for */
  block: number
  /**
   * The lines of each side the row stands for: [start, end); the lines of a block another section shows, the old lines
   * a remark hides while Inline shows its run once or leaves out the lines a move took, empty for a heading and for a
   * remark that hides none
   */
  lines: readonly (readonly [number, number])[]
  /** What the diff says of the lines after the row; null for a row without it */
  remark: Remark | null
}

export type Row = LineRow | FoldRow | NoteRow

/** A row that shows no line of a side: a fold or a note */
export function isLineRow(row: Row): row is LineRow {
  return row.kind !== "fold" && row.kind !== "note"
}

/**
 * What a section of a review shows of a file's diff (docs/design/diff-view.md, "Sections"): the blocks it shows with
 * the heading of the change each one starts, and a row for each block another section shows
 */
export interface SectionUnits {
  /** The heading before a block: the changes it starts */
  readonly marks: ReadonlyMap<number, Heading>
  /** The blocks another section shows, each with where: a row of words stands for its lines */
  readonly elsewhere: ReadonlyMap<number, string>
}

type Item =
  /** Lines equal on every side, from `starts`; `ignored` for the lines of an ignored block, a change the policy hides */
  | { kind: "equal"; starts: readonly number[]; count: number; ignored?: boolean }
  /** A block of a diff, its rows by its parts */
  | { kind: "block"; block: DiffBlock; index: number; parts: BlockParts }
  /** A block of a merge: its lines on each side, from `starts` */
  | { kind: "merge"; block: MergeBlock; index: number; starts: readonly number[]; counts: readonly number[] }
  /** A block another section shows: one row of words for its lines */
  | { kind: "elsewhere"; block: DiffBlock; index: number; label: string }

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
  /**
   * The stops of N and P, in order: of a diff the row the reading of each block starts at, the first row of lines of
   * what Rust reads from the line it names (`TextDiff.marks.starts`, `partsStopRow`; a block without one, an ignored
   * or a quiet one, is no stop, nor is a block of the imports alone a review plays down); of a merge the first row of
   * each block
   */
  readonly changeStarts: readonly number[]
  /** The blocks of changed lines the rows show (`isChangeBlock`), a stop or not: what `changeStarts` is counted against ("2 of 3 changes to read") */
  readonly changes: number
  /**
   * The notes that open (`Remark.open` not null), each by the old lines it hides or would hide, [start, end): its key
   * is the start. What ⌥-click opens together, and what a reveal of one of those lines opens
   */
  readonly notes: readonly (readonly [number, number])[]
  /** The row of each note of `notes`: what the key of the notes takes for the notes in view */
  readonly noteRows: readonly number[]
  /**
   * The notes at the ends of the moves the rows show, in the order of the rows, each with its row: what the key of
   * the moves takes for the notes in view, and where a jump to an end lands
   */
  readonly moveNotes: readonly { readonly end: MoveEnd; readonly row: number }[]
}

interface Gap extends FoldGap {
  items: Item[]
}

/**
 * Adds `count` equal lines from `starts` to the items, joined to an equal run that ends there; the lines of an ignored
 * block (`ignored`) are a run of their own, which a fold counts apart
 */
function pushEqual(items: Item[], starts: readonly number[], count: number, ignored = false): void {
  if (count <= 0) {
    return
  }
  const last = items.at(-1)
  if (!ignored && last?.kind === "equal" && last.ignored !== true && last.starts.every((start, side) => start + last.count === starts[side])) {
    last.count += count
  } else {
    items.push(ignored ? { kind: "equal", starts, count, ignored } : { kind: "equal", starts, count })
  }
}

/** What the rows of a diff depend on beside its blocks */
interface DiffRowsOptions {
  readonly mode: DiffViewMode
  /**
   * The blocks of the imports alone that a review plays down: pale, as a block of whitespace alone. A move with an end
   * in one is drawn at neither end (`drawnMarks`)
   */
  readonly playedImports: ReadonlySet<number>
  readonly units: SectionUnits | null
  /** The notes the user opened (`getOpenNotes`) */
  readonly open: ReadonlySet<number>
}

/**
 * The items of a diff in order: runs of equal lines and the blocks, each block in its parts (`blockParts`); an ignored
 * block with equal sides is equal text. The empty last line that the engine counts and no block touches is left out.
 */
function itemsOf(diff: TextDiff, { mode, playedImports, units, open }: DiffRowsOptions): Item[] {
  const items: Item[] = []
  // the runs and the ends of the moves of each block; `units.marks` below are the headings of a section
  const byBlock = marksByBlock(diff)
  let oldLine = 0
  let newLine = 0
  diff.blocks.forEach((block, index) => {
    pushEqual(items, [oldLine, newLine], block.newStart - newLine)
    const label = units?.elsewhere.get(index)
    if (label !== undefined) {
      items.push({ kind: "elsewhere", block, index, label })
    } else if (block.kind === "ignored" && block.oldEnd - block.oldStart === block.newEnd - block.newStart) {
      pushEqual(items, [block.oldStart, block.newStart], block.newEnd - block.newStart, true)
    } else {
      // the heading of the change the block starts, in a section of a review: the first row of the block
      const heading = block.kind === "ignored" ? undefined : units?.marks.get(index)
      // a move with an end in a block that is played down is drawn at neither end: the lines of that block stay pale,
      // with no note of where they went, and the lines of the other end are lines of their block, each with its row
      const parts = blockParts(block, index, drawnMarks(byBlock[index] ?? NO_MARKS, index, playedImports), {
        mode,
        tint: playedImports.has(index) ? "whitespace" : tintOf(block),
        open,
        heading: heading === undefined ? null : { parts: heading, label: headingText(heading) },
      })
      items.push({ kind: "block", block, index, parts })
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
function gapsOf(items: readonly Item[]): { gaps: Gap[]; changes: Item[] } {
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
      gap.rows += itemRowCount(item)
    }
  }
  return { gaps, changes }
}

/** The row count of an item: of a block of a diff, of its parts in the layout they were made for */
function itemRowCount(item: Item): number {
  if (item.kind === "equal") {
    return item.count
  }
  if (item.kind === "elsewhere") {
    return 1
  }
  if (item.kind === "merge") {
    return Math.max(...item.counts)
  }
  return item.parts.rowCount
}

/** Row `row` of an item */
function itemRow(item: Item, row: number): LineRow | NoteRow {
  if (item.kind === "elsewhere") {
    const { block, index, label } = item
    return { kind: "note", tone: "elsewhere", label, heading: null, block: index, lines: blockLines(block), remark: null }
  }
  if (item.kind === "equal") {
    return { kind: "context", lines: item.starts.map(start => start + row), block: -1, blockRow: 0, blockRows: 0, tint: "full", run: null, move: null }
  }
  if (item.kind === "merge") {
    const { block, index, starts, counts } = item
    const lines = starts.map((start, side) => (row < counts[side]! ? start + row : null))
    return { kind: "merge", lines, block: index, blockRow: row, blockRows: Math.max(...counts), tint: mergeTintOf(block), run: null, move: null }
  }
  return partsRow(item.parts, row)
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

/** The lines of each side of a block: [start, end) */
function blockLines(block: DiffBlock): [number, number][] {
  return [
    [block.oldStart, block.oldEnd],
    [block.newStart, block.newEnd],
  ]
}

/** The lines of each side that rows `[from, from + count)` of an item cover */
function itemLines(item: Item, from: number, count: number): [number, number][] {
  if (item.kind === "equal") {
    return item.starts.map(start => [start + from, start + from + count])
  }
  if (item.kind === "elsewhere") {
    return blockLines(item.block)
  }
  if (item.kind === "merge") {
    const clip = (start: number, length: number): [number, number] => [start + Math.min(from, length), start + Math.min(from + count, length)]
    return item.starts.map((start, side) => clip(start, item.counts[side]!))
  }
  return partsLines(item.parts, from, count)
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
function forEachRows(gap: Gap, from: number, to: number, action: (item: Item, start: number, count: number) => void): void {
  let offset = 0
  for (const item of gap.items) {
    const rows = itemRowCount(item)
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
 * without a change has no rows. With `quietImports` (the review plan), a block of the imports alone (`importBlocks`
 * of Rust) is pale and N and P pass it; a move with an end in such a block is drawn at neither end, its lines the
 * lines of their blocks (`drawnMarks`). `units` makes the rows of a section of a review: a heading before each block
 * it marks, one row for each block another section shows, which is no change of this section. A block shows by its
 * parts (`blockParts`): what the marks of the diff say of its lines is a note before them, and Inline shows a quiet
 * run once, and leaves out the lines a move took, until its note is opened (`open`, the notes by the first old line
 * each hides). N and P stop where Rust says the reading of a block starts (`TextDiff.marks.starts`).
 */
export function buildFileRows(
  diff: TextDiff,
  mode: DiffViewMode,
  levels: ArrayLike<number> = [],
  quietImports = false,
  units: SectionUnits | null = null,
  open: ReadonlySet<number> = NO_NOTES,
): FileRows {
  const playedImports = new Set(quietImports ? diff.importBlocks : [])
  const items = itemsOf(diff, { mode, playedImports, units, open })
  return rowsOf(items, mode, levels, { scopes: [diff.oldScopes, diff.newScopes], order: [NEW, OLD] }, stopsOf(diff, quietImports))
}

const NO_NOTES: ReadonlySet<number> = new Set()

/** The rows of a file's merge in three columns: left, the middle (`middle`), right. The gaps between the blocks fold as in a diff. */
export function buildMergeRows(merge: TextMerge, middle: MergeMiddle, levels: ArrayLike<number> = []): FileRows {
  // the result has no scopes of its own
  const middleScopes = middle === "base" ? merge.baseScopes : []
  return rowsOf(mergeItemsOf(merge, middle), "merge", levels, { scopes: [merge.leftScopes, middleScopes, merge.rightScopes], order: [MIDDLE, LEFT, RIGHT] }, null)
}

/** `stops`: of a diff the line the reading of each block starts at (`stopsOf`); null for a merge, whose every block is a stop */
function rowsOf(
  items: readonly Item[],
  mode: RowsMode,
  levels: ArrayLike<number>,
  labels: Labels,
  stops: ReadonlyMap<number, { readonly side: number; readonly line: number }> | null,
): FileRows {
  const { gaps, changes } = gapsOf(items)
  const segments: Segment[] = []
  const starts: number[] = []
  const changeStarts: number[] = []
  const notes: (readonly [number, number])[] = []
  const noteRows: number[] = []
  const moveNotes: { end: MoveEnd; row: number }[] = []
  let rowCount = 0
  const push = (segment: Segment, rows: number) => {
    segments.push(segment)
    starts.push(rowCount)
    rowCount += rows
  }
  const pushRows = (gap: Gap, from: number, to: number) => {
    forEachRows(gap, from, to, (item, start, count) => push({ kind: "lines", item, from: start, count }, count))
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
        push({ kind: "fold", row: foldRow(labels, gap, index, level, before, before + hidden) }, 1)
        pushRows(gap, before + hidden, gap.rows)
      }
      const change = changes[index]
      if (change === undefined) {
        return
      }
      const rows = itemRowCount(change)
      if (change.kind === "block") {
        // where Rust says the reading of the block starts: none for a block of the imports alone that a review plays down
        const stop = stops?.get(change.index)
        if (stop !== undefined && rows > 0) {
          changeStarts.push(rowCount + Math.min(partsStopRow(change.parts, stop.side, stop.line), rows - 1))
        }
        for (const note of partsNotes(change.parts)) {
          notes.push(note.lines)
          noteRows.push(rowCount + note.row)
        }
        for (const { end, row } of partsMoveNotes(change.parts)) {
          moveNotes.push({ end, row: rowCount + row })
        }
      } else {
        changeStarts.push(rowCount)
      }
      push({ kind: "lines", item: change, from: 0, count: rows }, rows)
    })
  } else {
    // a section of no change of its own (a declaration whose block another section holds): its rows for the blocks
    // other sections show, alone
    for (const item of items.filter(it => it.kind === "elsewhere")) {
      push({ kind: "lines", item, from: 0, count: 1 }, 1)
    }
  }
  starts.push(rowCount)
  const foldGaps = gaps.map(({ rows, changeBefore, changeAfter }) => ({ rows, changeBefore, changeAfter }))
  return {
    mode,
    rowCount,
    gapCount: gaps.length,
    gaps: foldGaps,
    segments,
    segmentStarts: Uint32Array.from(starts),
    changeStarts,
    changes: changes.filter(change => change.kind !== "block" || isChangeBlock(change.block)).length,
    notes,
    noteRows,
    moveNotes,
  }
}

function foldRow(labels: Labels, gap: Gap, index: number, level: number, from: number, to: number): FoldRow {
  let ranges: [number, number][] | undefined
  forEachRows(gap, from, to, (item, start, count) => {
    const lines = itemLines(item, start, count)
    ranges = ranges === undefined ? lines : ranges.map((range, side) => [range[0], lines[side]![1]])
  })
  const lines = ranges ?? labels.scopes.map((): [number, number] => [0, 0])
  // a scope around all the folded lines, the sides in their order; else one around the last folded line
  const around = (start: (side: number) => number) =>
    labels.order.reduce<Scope | undefined>((found, side) => found ?? innermostScope(labels.scopes[side] ?? [], start(side), lines[side]![1]), undefined)
  const label = around(side => lines[side]![0]) ?? around(side => lines[side]![1] - 1)
  return { kind: "fold", gap: index, level, lines, label: label?.label ?? null, ...foldedOf(gap, from, to) }
}

/**
 * What rows `[from, to)` of a gap hide: its unchanged lines, the blocks other sections show, the lines of the blocks
 * the policy ignores (an ignored deletion has no row in Unified: it is in the fold whose rows surround it)
 */
function foldedOf(gap: Gap, from: number, to: number): Pick<FoldRow, "unchanged" | "elsewhere" | "ignored"> {
  let unchanged = 0
  let ignored = 0
  const elsewhere: { block: number; label: string }[] = []
  let offset = 0
  for (const item of gap.items) {
    const rows = itemRowCount(item)
    const overlap = Math.min(to, offset + rows) - Math.max(from, offset)
    if (overlap > 0 || (rows === 0 && offset >= from && offset < to)) {
      if (item.kind === "equal") {
        if (item.ignored === true) {
          ignored += overlap
        } else {
          unchanged += overlap
        }
      } else if (item.kind === "elsewhere") {
        elsewhere.push({ block: item.index, label: item.label })
      } else if (item.kind === "block") {
        ignored += Math.max(item.block.oldEnd - item.block.oldStart, item.block.newEnd - item.block.newStart)
      }
    }
    offset += rows
  }
  return { unchanged, elsewhere, ignored }
}

/** "1 line", "62 lines" */
function lineCount(count: number): string {
  return `${count} ${count === 1 ? "line" : "lines"}`
}

/**
 * The words of a fold (openspec/specs/branch/review-tour/spec.md, "In the Review Window: A fold names the changes it
 * hides"): what it hides of the lines, "62 unchanged lines · 1 line hidden by Whitespace", empty for a fold of blocks
 * another section shows alone; then each place that shows blocks it hides, with the first of them, "1 change in part 2"
 * from the words of their row ("Shown in part 2")
 */
export function foldWords(fold: FoldRow): { lines: string; places: { block: number; words: string }[] } {
  const lines = [
    fold.unchanged > 0 || (fold.elsewhere.length === 0 && fold.ignored === 0) ? `${fold.unchanged} unchanged ${fold.unchanged === 1 ? "line" : "lines"}` : null,
    fold.ignored > 0 ? `${lineCount(fold.ignored)} hidden by Whitespace` : null,
  ]
  const blocks = new Map<string, number[]>()
  for (const { block, label } of fold.elsewhere) {
    blocks.set(label, [...(blocks.get(label) ?? []), block])
  }
  const places = [...blocks].map(([label, of]) => {
    const where = label.startsWith("Shown in ") ? label.slice("Shown ".length) : label.charAt(0).toLowerCase() + label.slice(1)
    return { block: of[0]!, words: `${of.length === 1 ? "1 change" : `${of.length} changes`} ${where}` }
  })
  return { lines: lines.filter(part => part !== null).join(" · "), places }
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
  return segment.kind === "lines" ? itemRow(segment.item, segment.from + row - rows.segmentStarts[index]!) : segment.row
}

/**
 * The row of an item where `line` of a side, or a later line of it, shows; the row count when no row does. An old line
 * of a block that has no row of its own gives the row that stands for it (`partsRowOfLine`)
 */
function itemRowOfLine(item: Item, side: number, line: number): number {
  if (item.kind === "equal") {
    return Math.max(line - item.starts[side]!, 0)
  }
  if (item.kind === "elsewhere") {
    return line < blockLines(item.block)[side]![1] ? 0 : 1
  }
  if (item.kind === "merge") {
    const start = item.starts[side]!
    return line >= start + item.counts[side]! ? itemRowCount(item) : Math.max(line - start, 0)
  }
  return partsRowOfLine(item.parts, side, line)
}

/**
 * The row that shows `line` of a side, or the first row after it: the anchor of the scroll position across a new
 * layout. A folded line gives its fold, an old line that Inline shows once by its new line the note that stands for it.
 */
export function rowOfLine(rows: FileRows, side: number, line: number): number {
  for (const [index, segment] of rows.segments.entries()) {
    const start = rows.segmentStarts[index]!
    if (segment.kind === "fold") {
      if (line < segment.row.lines[side]![1]) {
        return start
      }
    } else {
      const offset = itemRowOfLine(segment.item, side, line)
      if (offset < segment.from + segment.count) {
        return start + Math.max(offset - segment.from, 0)
      }
    }
  }
  return Math.max(rows.rowCount - 1, 0)
}

/**
 * The row a reveal of `line` of a side lands on, the line itself and no name in it (an end of a move): the row of the
 * line (`rowOfLine`), or the row of words right above it that says what the lines from it are, the note at the end of
 * a move whose lines start there. A line behind a note is that note
 */
export function rowOfPlace(rows: FileRows, side: number, line: number): number {
  const row = rowOfLine(rows, side, line)
  if (row === 0) {
    return row
  }
  const above = rowAt(rows, row - 1)
  return above.kind === "note" && above.remark?.move != null && above.remark.lines[side]![0] === line && isLineRow(rowAt(rows, row)) ? row - 1 : row
}

/**
 * The note that hides `line` of the old side, or would once closed, by its key (the first old line it hides); null for
 * a line no note of the rows stands for
 */
export function noteOfLine(rows: FileRows, line: number): number | null {
  return rows.notes.find(([start, end]) => start <= line && line < end)?.[0] ?? null
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

/**
 * What the levels of the folds are kept by: a diff or a merge, or a section of a review that shows some blocks of a
 * diff (`sectionFoldKey`), whose gaps are its own
 */
export type FoldKey = TextDiff | TextMerge | { readonly section: string }

// the levels of the folds of a diff or a merge while the store keeps it, so the lines it expanded stay expanded
const foldLevels = new WeakMap<FoldKey, readonly number[]>()
// the key of the folds of each section of a review of a diff, while the store keeps the diff
const sectionKeys = new WeakMap<TextDiff | TextMerge, Map<string, { readonly section: string }>>()

/** The key of the folds of the section `section` of `diff`: the same object while the store keeps the diff */
export function sectionFoldKey(diff: TextDiff, section: string): FoldKey {
  let keys = sectionKeys.get(diff)
  if (keys === undefined) {
    keys = new Map()
    sectionKeys.set(diff, keys)
  }
  let key = keys.get(section)
  if (key === undefined) {
    key = { section }
    keys.set(section, key)
  }
  return key
}

/** The levels of the folds of a diff, a merge or a section, by gap; a missing one is 0 */
export function getFoldLevels(model: FoldKey): readonly number[] {
  return foldLevels.get(model) ?? []
}

export function setFoldLevels(model: FoldKey, levels: readonly number[]): void {
  foldLevels.set(model, levels)
}

// the notes of a diff or a section that the user opened, beside the levels of its folds: the old lines they showed
// stay shown while the store keeps the diff
const openNotes = new WeakMap<FoldKey, ReadonlySet<number>>()

/** The notes of a diff or a section that are open, each by the first old line it hides (`Remark.open`) */
export function getOpenNotes(model: FoldKey): ReadonlySet<number> {
  return openNotes.get(model) ?? NO_NOTES
}

export function setOpenNotes(model: FoldKey, notes: ReadonlySet<number>): void {
  openNotes.set(model, notes)
}

/**
 * The same diff or merge as a new object (the store replaces the content of a file as its scopes come): what the
 * views opened of it goes with it. The levels of its folds, its open notes, and the keys of its sections
 * (`sectionFoldKey`), so the folds and the notes of each section of a review stay as they are too
 */
export function keepOpened(from: TextDiff | TextMerge, to: TextDiff | TextMerge): void {
  const levels = foldLevels.get(from)
  if (levels !== undefined) {
    foldLevels.set(to, levels)
  }
  const notes = openNotes.get(from)
  if (notes !== undefined) {
    openNotes.set(to, notes)
  }
  const keys = sectionKeys.get(from)
  if (keys !== undefined) {
    sectionKeys.set(to, keys)
  }
}
