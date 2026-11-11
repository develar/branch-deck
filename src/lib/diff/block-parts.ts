import type { DiffBlock, LineRun, RunKind, TextDiff } from "@/lib/diff/file-diff-model"
import type { Heading, LineRow, LineRowKind, LineTint, NoteRow } from "@/lib/diff/rows"
import type { DiffViewMode } from "@/stores/settings/diff-view-settings"

/*
 * The parts of a block of a diff by its marks (docs/design/diff-view.md, "The parts of a block"): Rust says which rows
 * of lines of a block changed in one way alone (`TextDiff.marks.runs`), and a block shows part by part, in the order
 * of both sides. A part is a row of words (the heading of the block in a section of a review, what the diff says of
 * the lines after it) or rows of lines in one shape. Every function of rows.ts that maps the rows of a block to its
 * lines goes through here, so a view never counts the rows of a block by its line counts.
 */

/** The sides of a row of a diff, as `OLD` and `NEW` of rows.ts */
const OLD = 0
const NEW = 1

/**
 * What the diff says of a row of lines of a block, which a note row words (`presentRemark`): a run of
 * `TextDiff.marks.runs`
 */
export interface Remark {
  readonly kind: RunKind
  /** Nothing of the lines to read (`LineRun.quiet`): they are played down, and N and P pass them */
  readonly quiet: boolean
  /** The lines of each side it is about: [start, end) */
  readonly lines: readonly (readonly [number, number])[]
  /**
   * Its old lines show: false while the layout shows the lines once, by the new ones (Inline, a quiet run), true once
   * the note is opened; null where the layout hides no line of it, so the note has nothing to open
   */
  readonly open: boolean | null
}

/**
 * How the rows of a part show its lines: `sides` a row per line of the longer side, the shorter one filled (Split);
 * `pairs` a row per pair of lines (Inline); `stacked` the old lines, then the new ones (Unified); `new` the new lines
 * alone, which stand for the old ones (an ignored block in one column, a quiet run of Inline)
 */
type PartShape = "sides" | "pairs" | "stacked" | "new"

/** Rows of lines of a block: the old lines `[oldStart, oldStart + oldCount)` and the new ones, in one shape */
interface LinesPart {
  readonly kind: "lines"
  readonly shape: PartShape
  readonly oldStart: number
  readonly oldCount: number
  readonly newStart: number
  readonly newCount: number
  readonly tint: LineTint
  /** The kind of the quiet run the lines are of; null for lines to read */
  readonly run: RunKind | null
  /** The lines of a block the whitespace policy ignores: drawn unchanged */
  readonly ignored: boolean
}

/** A row of words: the heading of the change the block starts, what the diff says of the lines after it, or both */
interface NotePart {
  readonly kind: "note"
  readonly heading: Heading | null
  /** The words of the heading as one line (`headingText`); empty without one */
  readonly label: string
  readonly remark: Remark | null
}

type BlockPart = LinesPart | NotePart

/** The parts of a block in a layout, with the first row of each */
export interface BlockParts {
  readonly block: DiffBlock
  /** The index of the block in the blocks of its diff */
  readonly index: number
  readonly parts: readonly BlockPart[]
  /** The first row of each part, and the row count at the end */
  readonly starts: Uint32Array
  readonly rowCount: number
}

/** What the parts of a block depend on beside its lines */
export interface PartsOptions {
  readonly mode: DiffViewMode
  /** The tint of the lines of the block that are in no quiet run */
  readonly tint: LineTint
  /** The notes the user opened, each by the first old line it hides (`getOpenNotes`) */
  readonly open: ReadonlySet<number>
  /** The heading of the change the block starts in a section of a review, its first row, and its words as one line (`headingText`) */
  readonly heading: { readonly parts: Heading; readonly label: string } | null
}

function rowsOf(part: BlockPart): number {
  if (part.kind === "note") {
    return 1
  }
  switch (part.shape) {
    case "sides":
      return Math.max(part.oldCount, part.newCount)
    case "pairs":
    case "new":
      return part.newCount
    case "stacked":
      return part.oldCount + part.newCount
  }
}

/**
 * The runs of each block of a diff, in their order: a run lies inside one block (`TextDiff.marks.runs`); one that lies
 * in none, which no engine gives, is left out, so a view shows every line of every block whatever the marks say
 */
export function runsByBlock(diff: TextDiff): readonly (readonly LineRun[])[] {
  const known = runsOfDiffs.get(diff)
  if (known !== undefined) {
    return known
  }
  const { runs } = diff.marks
  let next = 0
  const byBlock = diff.blocks.map(block => {
    const own: LineRun[] = []
    let oldLine = block.oldStart
    let newLine = block.newStart
    // the runs before the block are of no block
    while (next < runs.length && (runs[next]!.oldStart < block.oldStart || runs[next]!.newStart < block.newStart)) {
      next++
    }
    for (; next < runs.length; next++) {
      const run = runs[next]!
      if (run.oldStart >= block.oldEnd || run.newStart >= block.newEnd) {
        break
      }
      // inside the block, after the run before it, with a line on both sides
      if (
        run.oldStart >= oldLine &&
        run.newStart >= newLine &&
        run.oldEnd <= block.oldEnd &&
        run.newEnd <= block.newEnd &&
        run.oldStart < run.oldEnd &&
        run.newStart < run.newEnd
      ) {
        own.push(run)
        oldLine = run.oldEnd
        newLine = run.newEnd
      }
    }
    return own
  })
  runsOfDiffs.set(diff, byBlock)
  return byBlock
}

// read once per diff: every layout of a file asks
const runsOfDiffs = new WeakMap<TextDiff, readonly (readonly LineRun[])[]>()

/**
 * The parts of a block in a layout. The runs cut it in the order of both sides: the lines between two runs are a part
 * that shows as the block did; a run is a note that says what it is, then its lines. Split and Unified show every line
 * of a run, a quiet one played down (`tint: "quiet"`); Inline shows a quiet run once, by its new lines, until its note
 * is opened, then as Unified. A run that is not quiet is a label: its lines show as those of the block. A heading
 * (`PartsOptions.heading`) is the first row of the block; a run that starts the block says its words in that row.
 */
export function blockParts(block: DiffBlock, index: number, runs: readonly LineRun[], { mode, tint, open, heading }: PartsOptions): BlockParts {
  const parts: BlockPart[] = []
  const ignored = block.kind === "ignored"
  /** The shape of lines that show as the block does: a pair per row only where the engine pairs line i of both sides */
  const plain = (oldStart: number, oldCount: number, newStart: number, newCount: number): PartShape => {
    if (mode === "split") {
      return "sides"
    }
    if (ignored) {
      return "new"
    }
    const paired = block.inlinePairs && oldCount === newCount && oldStart - block.oldStart === newStart - block.newStart
    return mode === "inline" && paired ? "pairs" : "stacked"
  }
  const lines = (oldStart: number, oldEnd: number, newStart: number, newEnd: number, run: LineRun | null, shown: boolean | null) => {
    const oldCount = oldEnd - oldStart
    const newCount = newEnd - newStart
    if (oldCount === 0 && newCount === 0) {
      return
    }
    const quiet = run !== null && run.quiet
    // Inline shows a quiet run once, by its new lines, until its note is opened
    const shape = quiet && mode === "inline" ? (shown === true ? "stacked" : "new") : plain(oldStart, oldCount, newStart, newCount)
    parts.push({ kind: "lines", shape, oldStart, oldCount, newStart, newCount, tint: quiet ? "quiet" : tint, run: quiet ? run.kind : null, ignored })
  }
  if (heading !== null) {
    parts.push({ kind: "note", heading: heading.parts, label: heading.label, remark: null })
  }
  let oldLine = block.oldStart
  let newLine = block.newStart
  for (const run of runs) {
    lines(oldLine, run.oldStart, newLine, run.newStart, null, null)
    // only Inline hides lines: the old ones of a quiet run
    const shown = mode === "inline" && run.quiet ? open.has(run.oldStart) : null
    const remark: Remark = {
      kind: run.kind,
      quiet: run.quiet,
      lines: [
        [run.oldStart, run.oldEnd],
        [run.newStart, run.newEnd],
      ],
      open: shown,
    }
    const last = parts.at(-1)
    if (last?.kind === "note" && last.remark === null) {
      // the run starts the block: its words join the heading instead of a second row of words
      parts[parts.length - 1] = { ...last, remark }
    } else {
      parts.push({ kind: "note", heading: null, label: "", remark })
    }
    lines(run.oldStart, run.oldEnd, run.newStart, run.newEnd, run, shown)
    oldLine = run.oldEnd
    newLine = run.newEnd
  }
  lines(oldLine, block.oldEnd, newLine, block.newEnd, null, null)
  const starts = new Uint32Array(parts.length + 1)
  parts.forEach((part, at) => {
    starts[at + 1] = starts[at]! + rowsOf(part)
  })
  return { block, index, parts, starts, rowCount: starts[parts.length]! }
}

/** The index of the part that holds row `row` */
function partAt(parts: BlockParts, row: number): number {
  let low = 0
  let high = parts.parts.length - 1
  while (low < high) {
    const middle = (low + high + 1) >> 1
    if (parts.starts[middle]! <= row) {
      low = middle
    } else {
      high = middle - 1
    }
  }
  return low
}

/** The lines a note stands for: the old lines of its run that have no row (`Remark.open` false), none else */
function noteLines(remark: Remark | null, block: DiffBlock): [number, number][] {
  if (remark === null) {
    return [
      [block.oldStart, block.oldStart],
      [block.newStart, block.newStart],
    ]
  }
  const [oldStart, oldEnd] = remark.lines[OLD]!
  const newStart = remark.lines[NEW]![0]
  return [
    [oldStart, remark.open === false ? oldEnd : oldStart],
    [newStart, newStart],
  ]
}

/** Row `row` of the parts of a block */
export function partsRow(parts: BlockParts, row: number): LineRow | NoteRow {
  const at = partAt(parts, row)
  const part = parts.parts[at]!
  const { block, index } = parts
  if (part.kind === "note") {
    const { heading, label, remark } = part
    return { kind: "note", tone: heading === null ? "remark" : "unit", label, heading, block: index, lines: noteLines(remark, block), remark }
  }
  const offset = row - parts.starts[at]!
  const line = (kind: LineRowKind, oldLine: number | null, newLine: number | null): LineRow => ({
    kind,
    lines: [oldLine, newLine],
    block: index,
    blockRow: row,
    blockRows: parts.rowCount,
    tint: part.tint,
    run: part.run,
  })
  switch (part.shape) {
    case "sides":
      return line(part.ignored ? "ignored" : "changed", offset < part.oldCount ? part.oldStart + offset : null, offset < part.newCount ? part.newStart + offset : null)
    case "pairs":
      return line("inline", part.oldStart + offset, part.newStart + offset)
    case "stacked":
      return offset < part.oldCount ? line("deleted", part.oldStart + offset, null) : line("added", null, part.newStart + offset - part.oldCount)
    case "new":
      return line(part.ignored ? "ignored" : "once", null, part.newStart + offset)
  }
}

/**
 * The row of the parts where `line` of a side, or a later line of it, shows; the row count when no row does. An old
 * line without a row of its own gives the row that stands for it: the note of its quiet run in Inline, the first new
 * line of an ignored block in one column
 */
export function partsRowOfLine(parts: BlockParts, side: number, line: number): number {
  for (const [at, part] of parts.parts.entries()) {
    const start = parts.starts[at]!
    if (part.kind === "note") {
      if (side === OLD && part.remark?.open === false && line < part.remark.lines[OLD]![1]) {
        return start
      }
      continue
    }
    const first = side === OLD ? part.oldStart : part.newStart
    const count = side === OLD ? part.oldCount : part.newCount
    if (line >= first + count) {
      continue
    }
    const offset = Math.max(line - first, 0)
    switch (part.shape) {
      case "sides":
      case "pairs":
        return start + offset
      case "stacked":
        return start + (side === OLD ? offset : part.oldCount + offset)
      case "new":
        return start + (side === NEW ? offset : 0)
    }
  }
  return parts.rowCount
}

/**
 * The row the reading of a block starts at in its layout, from `line` of a side, the line Rust names
 * (`TextDiff.marks.starts`). Rust cuts a block by its quiet runs alone and names the first new line of what is read
 * first, else its first old one (`text_diff::model::starts_of`); a view cuts the block at every run, a label too, and
 * shows the old lines of a part before its new ones. So the stop is the first row of lines of what Rust reads there:
 * from the part that shows the line back over the parts of no quiet run (lines to read, the note of a label) to the
 * one after the last quiet run. Lines deleted before a comment worded again are read from their first row, though
 * Rust names the comment, the first new line. The row count when no part shows the line
 */
export function partsStopRow(parts: BlockParts, side: number, line: number): number {
  // the first part of lines to read since the last quiet run
  let first = -1
  for (const [at, part] of parts.parts.entries()) {
    if (part.kind !== "lines") {
      continue
    }
    if (part.run !== null) {
      first = -1
    } else if (first < 0) {
      first = at
    }
    const start = side === OLD ? part.oldStart : part.newStart
    if (start <= line && line < start + (side === OLD ? part.oldCount : part.newCount)) {
      // a line of a quiet run, which Rust never names, is where it shows
      return parts.starts[first < 0 ? at : first]!
    }
  }
  return parts.rowCount
}

/** The lines of each side that rows `[from, from + count)` of the parts cover, the lines they stand for with them */
export function partsLines(parts: BlockParts, from: number, count: number): [number, number][] {
  const { block } = parts
  if (from <= 0 && from + count >= parts.rowCount) {
    return [
      [block.oldStart, block.oldEnd],
      [block.newStart, block.newEnd],
    ]
  }
  let covered: [number, number][] | undefined
  for (const [at, part] of parts.parts.entries()) {
    const rows = parts.starts[at + 1]! - parts.starts[at]!
    const start = Math.max(from - parts.starts[at]!, 0)
    const end = Math.min(from + count - parts.starts[at]!, rows)
    if (start >= end) {
      continue
    }
    const lines = part.kind === "note" ? noteLines(part.remark, block) : linesOf(part, start, end)
    covered = covered === undefined ? lines : covered.map((range, side) => [range[0], lines[side]![1]])
  }
  return (
    covered ?? [
      [block.oldStart, block.oldStart],
      [block.newStart, block.newStart],
    ]
  )
}

/** The lines of each side that rows `[from, to)` of a part of lines cover */
function linesOf(part: LinesPart, from: number, to: number): [number, number][] {
  const clip = (start: number, length: number, first: number, last: number): [number, number] => [
    start + Math.min(Math.max(first, 0), length),
    start + Math.min(Math.max(last, 0), length),
  ]
  switch (part.shape) {
    case "sides":
      return [clip(part.oldStart, part.oldCount, from, to), clip(part.newStart, part.newCount, from, to)]
    case "pairs":
      return [clip(part.oldStart, part.oldCount, from, to), clip(part.newStart, part.newCount, from, to)]
    case "stacked":
      return [clip(part.oldStart, part.oldCount, from, to), clip(part.newStart, part.newCount, from - part.oldCount, to - part.oldCount)]
    case "new":
      // the old lines have no row: the first row stands for them from their start, the last one to their end
      return [clip(part.oldStart, part.oldCount, from === 0 ? 0 : from, to === part.newCount ? part.oldCount : to), clip(part.newStart, part.newCount, from, to)]
  }
}

/**
 * The notes of the parts that open (`Remark.open` not null), each with its row and the old lines it hides or would
 * hide: [start, end), the start the key of the note
 */
export function partsNotes(parts: BlockParts): { readonly lines: readonly [number, number]; readonly row: number }[] {
  return parts.parts.flatMap((part, at) =>
    part.kind === "note" && part.remark !== null && part.remark.open !== null ? [{ lines: part.remark.lines[OLD]!, row: parts.starts[at]! }] : [],
  )
}

/**
 * The pairs of lines of a block that Inline shows as one row each, `[old line, new line]`: the rows of its `pairs`
 * parts, whatever notes are open
 */
export function inlinePairsOf(block: DiffBlock, index: number, runs: readonly LineRun[]): [number, number][] {
  const pairs: [number, number][] = []
  for (const part of blockParts(block, index, runs, { mode: "inline", tint: "full", open: NO_NOTES, heading: null }).parts) {
    if (part.kind === "lines" && part.shape === "pairs") {
      for (let row = 0; row < part.newCount; row++) {
        pairs.push([part.oldStart + row, part.newStart + row])
      }
    }
  }
  return pairs
}

const NO_NOTES: ReadonlySet<number> = new Set()

/**
 * The stops of a diff by block, in the order of its blocks: the line the reading of each block starts at, as Rust
 * says it (`TextDiff.marks.starts`, flat `[block, side, line]`). A block without one has nothing to read; with
 * `quietImports` (a review with its plan) a block of the imports alone is no stop either. The one source of where N
 * and P stop and of the count of what is left to read
 */
export function stopsOf(diff: TextDiff, quietImports = false): Map<number, { readonly side: number; readonly line: number }> {
  const stops = new Map<number, { side: number; line: number }>()
  const { starts } = diff.marks
  for (let at = 0; at + 2 < starts.length; at += 3) {
    if (!(quietImports && diff.importBlocks.includes(starts[at]!))) {
      stops.set(starts[at]!, { side: starts[at + 1]!, line: starts[at + 2]! })
    }
  }
  return stops
}

/** A block of changed lines: one the whitespace policy does not ignore, with a line (the stand-in of a use left outside has none) */
export function isChangeBlock(block: DiffBlock): boolean {
  return block.kind !== "ignored" && (block.oldStart < block.oldEnd || block.newStart < block.newEnd)
}

/**
 * What is left to read of a diff: its stops (`stopsOf`) among its blocks of changed lines. The words are the
 * presenter's (`formatToRead`)
 */
export function readingOf(diff: TextDiff, quietImports = false): { readonly toRead: number; readonly changes: number } {
  return { toRead: stopsOf(diff, quietImports).size, changes: diff.blocks.filter(isChangeBlock).length }
}
