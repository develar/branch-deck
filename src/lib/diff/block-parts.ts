import type { DiffBlock, LineMove, LineRun, RunKind, TextDiff } from "@/lib/diff/file-diff-model"
import type { Heading, LineRow, LineRowKind, LineTint, NoteRow } from "@/lib/diff/rows"
import type { DiffViewMode } from "@/stores/settings/diff-view-settings"

/*
 * The parts of a block of a diff by its marks (docs/design/diff-view.md, "The parts of a block"): Rust says which rows
 * of lines of a block changed in one way alone (`TextDiff.marks.runs`) and which lines moved within the file
 * (`TextDiff.marks.moves`: the old lines of a move in the block they left, its new lines in the block they stand in
 * now), and a block shows part by part, in the order of both sides. A part is a row of words (the heading of the block
 * in a section of a review, what the diff says of the lines after it) or rows of lines in one shape. Every function of
 * rows.ts that maps the rows of a block to its lines goes through here, so a view never counts the rows of a block by
 * its line counts.
 */

/** The sides of a row of a diff, as `OLD` and `NEW` of rows.ts */
const OLD = 0
const NEW = 1

/**
 * An end of a move (`TextDiff.marks.moves`) in the block that holds it: `source` where its lines left, the old lines
 * of the move, nothing to read there; `destination` where they stand now, its new lines, read there
 */
export interface MoveEnd {
  readonly at: "source" | "destination"
  readonly move: LineMove
  /**
   * The lines of the end on its side, the blank lines that moved with them (`blanksBefore`, `blanksAfter`) among
   * them: [start, end), inside its block
   */
  readonly lines: readonly [number, number]
  /**
   * The lines of the other end of the move on its side, as that end has them (`lines` there): what the note of this
   * end names ("to line 210") and where a jump from it lands. Read here alone from the blank lines of the move, as
   * Rust reads them once (`LineMove::old_lines`, `new_lines`)
   */
  readonly other: readonly [number, number]
  /** The index of the block that holds the other end: a view that does not draw the move there draws it nowhere (`drawnMarks`) */
  readonly otherBlock: number
}

/** The marks of one block of a diff: its runs, and the ends of the moves it holds */
export interface BlockMarks {
  /** In the order of both sides, without overlap, each with a line on both sides */
  readonly runs: readonly LineRun[]
  /** The moves whose old lines the block holds, by those lines */
  readonly sources: readonly MoveEnd[]
  /** The moves whose new lines the block holds, by those lines */
  readonly destinations: readonly MoveEnd[]
}

/** A block the marks say nothing of */
export const NO_MARKS: BlockMarks = { runs: [], sources: [], destinations: [] }

/**
 * What the diff says of a row of lines of a block, which a note row words (`presentRemark`): a run of
 * `TextDiff.marks.runs`, or an end of a move of `TextDiff.marks.moves` (`kind: "moved"`)
 */
export interface Remark {
  readonly kind: RunKind | "moved"
  /** Nothing of the lines to read (`LineRun.quiet`): they are played down, and N and P pass them */
  readonly quiet: boolean
  /** The lines of each side it is about: [start, end); of an end of a move the lines of its side alone */
  readonly lines: readonly (readonly [number, number])[]
  /**
   * Its old lines show: false while the layout shows the lines once (Inline: a quiet run by its new lines, the lines
   * of a move where they stand now), true once the note is opened; null where the layout hides no line of it, so the
   * note has nothing to open
   */
  readonly open: boolean | null
  /** The end of the move the note stands at, which its words name and a click on them shows the other end of; null for a run */
  readonly move: MoveEnd | null
}

/**
 * How the rows of a part show its lines: `sides` a row per line of the longer side, the shorter one filled (Split);
 * `pairs` a row per pair of lines (Inline); `stacked` the old lines, then the new ones (Unified); `new` the new lines
 * alone, which stand for the old ones (an ignored block in one column, a quiet run of Inline); `landed` the new lines
 * of a move where they stand now, in Inline: a row per line, an edited line as the pair of its old line and its new one
 */
type PartShape = "sides" | "pairs" | "stacked" | "new" | "landed"

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
  /** The end of the move the lines are of; null for lines of no move */
  readonly move: MoveEnd | null
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
    case "landed":
      return part.newCount
    case "stacked":
      return part.oldCount + part.newCount
  }
}

/**
 * The marks of each block of a diff: its runs in their order, and the ends of the moves it holds, each side in its
 * order. A run lies inside one block (`TextDiff.marks.runs`), the old lines of a move inside one block and its new
 * lines inside one (`TextDiff.marks.moves`), neither in a block the whitespace policy ignores. A run that lies in no
 * block and a move with an end in none, which no engine gives, are left out, so a view shows every line of every
 * block whatever the marks say
 */
export function marksByBlock(diff: TextDiff): readonly BlockMarks[] {
  const known = marksOfDiffs.get(diff)
  if (known !== undefined) {
    return known
  }
  const { runs, moves } = diff.marks
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
    return { runs: own, sources: [] as MoveEnd[], destinations: [] as MoveEnd[] }
  })
  let sorted = true
  for (const move of moves) {
    const from = blockOf(diff.blocks, OLD, move.oldStart, move.len)
    const to = blockOf(diff.blocks, NEW, move.newStart, move.len)
    if (from < 0 || to < 0) {
      continue
    }
    const { oldStart, oldEnd } = diff.blocks[from]!
    const { newStart, newEnd } = diff.blocks[to]!
    // the blank lines that moved with the lines stand before them on one side and after them on the other
    const left: readonly [number, number] = [Math.max(move.oldStart - move.blanksBefore, oldStart), Math.min(move.oldStart + move.len + move.blanksAfter, oldEnd)]
    const stands: readonly [number, number] = [Math.max(move.newStart - move.blanksAfter, newStart), Math.min(move.newStart + move.len + move.blanksBefore, newEnd)]
    const sources = byBlock[from]!.sources
    sorted &&= sources.length === 0 || sources.at(-1)!.lines[0] <= left[0]
    sources.push({ at: "source", move, lines: left, other: stands, otherBlock: to })
    byBlock[to]!.destinations.push({ at: "destination", move, lines: stands, other: left, otherBlock: from })
  }
  // the moves come by their new lines: the ends of the old side of a block are put in the order of its old lines
  if (!sorted) {
    for (const { sources } of byBlock) {
      sources.sort((a, b) => a.lines[0] - b.lines[0])
    }
  }
  marksOfDiffs.set(diff, byBlock)
  return byBlock
}

// read once per diff: every layout of a file asks
const marksOfDiffs = new WeakMap<TextDiff, readonly BlockMarks[]>()

/**
 * The marks of block `index` as a view draws them that plays some blocks down (`played`: the blocks of the imports
 * alone in a review with its plan): without the moves that have an end in such a block, at both their ends. A move is
 * drawn at both ends or at neither. The block that is played down shows its lines as it did, pale, with no note of a
 * move; the block of the other end shows the lines of the move as its other lines, each with its row in every layout:
 * no line behind a note whose other end the view does not draw, no edited line whose change shows nowhere. The same
 * marks where nothing is left out
 */
export function drawnMarks(marks: BlockMarks, index: number, played: ReadonlySet<number>): BlockMarks {
  if (played.size === 0 || (marks.sources.length === 0 && marks.destinations.length === 0)) {
    return marks
  }
  if (played.has(index)) {
    return { runs: marks.runs, sources: [], destinations: [] }
  }
  const drawn = (end: MoveEnd) => !played.has(end.otherBlock)
  return marks.sources.every(drawn) && marks.destinations.every(drawn)
    ? marks
    : { runs: marks.runs, sources: marks.sources.filter(drawn), destinations: marks.destinations.filter(drawn) }
}

/**
 * The index of the block that holds the `count` lines of a side from `start`, one the whitespace policy does not
 * ignore; -1 when no block holds them all, or for no line
 */
function blockOf(blocks: readonly DiffBlock[], side: number, start: number, count: number): number {
  let low = 0
  let high = blocks.length - 1
  // the last block that starts at or before the line
  while (low < high) {
    const middle = (low + high + 1) >> 1
    if ((side === OLD ? blocks[middle]!.oldStart : blocks[middle]!.newStart) <= start) {
      low = middle
    } else {
      high = middle - 1
    }
  }
  const block = blocks[low]
  if (block === undefined || count <= 0 || block.kind === "ignored") {
    return -1
  }
  const [first, end] = side === OLD ? [block.oldStart, block.oldEnd] : [block.newStart, block.newEnd]
  return first <= start && start + count <= end ? low : -1
}

/**
 * The next end of `ends` from the index `at` with a line in `[cursor, limit)`, cut to those lines; the ends before the
 * cursor are passed. Null when none starts before `limit`
 */
function nextEnd(ends: readonly MoveEnd[], at: number, cursor: number, limit: number): { readonly at: number; readonly start: number; readonly end: number } | null {
  for (; at < ends.length; at++) {
    const [first, last] = ends[at]!.lines
    if (first >= limit) {
      return null
    }
    const start = Math.max(first, cursor)
    const end = Math.min(last, limit)
    if (start < end) {
      return { at, start, end }
    }
  }
  return null
}

/**
 * The parts of a block in a layout. The runs cut it in the order of both sides: the lines between two runs are a part
 * that shows as the block did; a run is a note that says what it is, then its lines. Split and Unified show every line
 * of a run, a quiet one played down (`tint: "quiet"`); Inline shows a quiet run once, by its new lines, until its note
 * is opened, then as Unified. A run that is not quiet is a label: its lines show as those of the block. A heading
 * (`PartsOptions.heading`) is the first row of the block; a run that starts the block says its words in that row.
 *
 * The ends of the moves cut the lines between two runs once more, each on its own side: where lines left (`sources`)
 * a note says where they stand now, then the lines, which Inline leaves out until the note is opened (the note stands
 * for them); where lines stand now (`destinations`) a note says where they were, then the lines, which Inline shows
 * once, an edited one as the pair of its two lines. The lines of no move between them show as the block did. In lines
 * the engine pairs (`inlinePairs`), a cut of one side cuts the other at the same line, so the lines beside a move
 * keep their pairs and the lines whose pair moved show alone.
 */
export function blockParts(block: DiffBlock, index: number, marks: BlockMarks, { mode, tint, open, heading }: PartsOptions): BlockParts {
  const parts: BlockPart[] = []
  const ignored = block.kind === "ignored"
  /** The engine pairs line i of both sides in these lines: as many of each side, at the same place in the block */
  const paired = (oldStart: number, oldCount: number, newStart: number, newCount: number) =>
    block.inlinePairs && oldCount === newCount && oldStart - block.oldStart === newStart - block.newStart
  /** The shape of lines that show as the block does: a pair per row only where the engine pairs line i of both sides */
  const plain = (oldStart: number, oldCount: number, newStart: number, newCount: number): PartShape => {
    if (mode === "split") {
      return "sides"
    }
    if (ignored) {
      return "new"
    }
    return mode === "inline" && paired(oldStart, oldCount, newStart, newCount) ? "pairs" : "stacked"
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
    parts.push({ kind: "lines", shape, oldStart, oldCount, newStart, newCount, tint: quiet ? "quiet" : tint, run: quiet ? run.kind : null, ignored, move: null })
  }
  /** The note of a run or of an end of a move: in the row of the heading when it starts the block, else a row of its own */
  const note = (remark: Remark) => {
    const last = parts.at(-1)
    if (last?.kind === "note" && last.remark === null) {
      // the run starts the block: its words join the heading instead of a second row of words
      parts[parts.length - 1] = { ...last, remark }
    } else {
      parts.push({ kind: "note", heading: null, label: "", remark })
    }
  }
  let source = 0
  let destination = 0
  /** The lines of no run from the old line `oldLine` to `oldEnd` and from the new line `newLine` to `newEnd`: cut at the ends of the moves, each on its side */
  const between = (oldLine: number, oldEnd: number, newLine: number, newEnd: number) => {
    const aligned = paired(oldLine, oldEnd - oldLine, newLine, newEnd - newLine)
    const [oldFirst, newFirst] = [oldLine, newLine]
    for (;;) {
      const left = nextEnd(marks.sources, source, oldLine, oldEnd)
      const stands = nextEnd(marks.destinations, destination, newLine, newEnd)
      if (left !== null && left.start === oldLine) {
        // where lines left: only Inline hides them, behind the note, which opens
        const end = marks.sources[left.at]!
        const shown = mode === "inline" ? open.has(left.start) : null
        note({
          kind: "moved",
          quiet: false,
          lines: [
            [left.start, left.end],
            [newLine, newLine],
          ],
          open: shown,
          move: end,
        })
        if (shown !== false) {
          parts.push({
            kind: "lines",
            shape: mode === "split" ? "sides" : "stacked",
            oldStart: left.start,
            oldCount: left.end - left.start,
            newStart: newLine,
            newCount: 0,
            tint,
            run: null,
            ignored,
            move: end,
          })
        }
        oldLine = left.end
        source = left.at + 1
        continue
      }
      if (stands !== null && stands.start === newLine) {
        // where lines stand now: every layout shows them, Inline an edited one as the pair of its two lines
        const end = marks.destinations[stands.at]!
        note({
          kind: "moved",
          quiet: false,
          lines: [
            [oldLine, oldLine],
            [stands.start, stands.end],
          ],
          open: null,
          move: end,
        })
        parts.push({
          kind: "lines",
          shape: mode === "split" ? "sides" : mode === "inline" ? "landed" : "stacked",
          oldStart: oldLine,
          oldCount: 0,
          newStart: stands.start,
          newCount: stands.end - stands.start,
          tint,
          run: null,
          ignored,
          move: end,
        })
        newLine = stands.end
        destination = stands.at + 1
        continue
      }
      // the lines of no move up to the next end of each side
      let oldTo = left?.start ?? oldEnd
      let newTo = stands?.start ?? newEnd
      if (oldTo === oldLine && newTo === newLine) {
        return
      }
      if (aligned) {
        // line i of both sides is a pair: both sides are cut at the same line, and the lines whose pair a move took
        // show alone before the pairs go on
        const ahead = oldLine - oldFirst - (newLine - newFirst)
        if (ahead > 0 && newTo > newLine) {
          oldTo = oldLine
          newTo = Math.min(newTo, newLine + ahead)
        } else if (ahead < 0 && oldTo > oldLine) {
          newTo = newLine
          oldTo = Math.min(oldTo, oldLine - ahead)
        } else if (ahead === 0) {
          const count = Math.min(oldTo - oldLine, newTo - newLine)
          if (count > 0) {
            oldTo = oldLine + count
            newTo = newLine + count
          }
        }
      }
      lines(oldLine, oldTo, newLine, newTo, null, null)
      oldLine = oldTo
      newLine = newTo
    }
  }
  if (heading !== null) {
    parts.push({ kind: "note", heading: heading.parts, label: heading.label, remark: null })
  }
  let oldLine = block.oldStart
  let newLine = block.newStart
  for (const run of marks.runs) {
    between(oldLine, run.oldStart, newLine, run.newStart)
    // only Inline hides lines: the old ones of a quiet run
    const shown = mode === "inline" && run.quiet ? open.has(run.oldStart) : null
    note({
      kind: run.kind,
      quiet: run.quiet,
      lines: [
        [run.oldStart, run.oldEnd],
        [run.newStart, run.newEnd],
      ],
      open: shown,
      move: null,
    })
    lines(run.oldStart, run.oldEnd, run.newStart, run.newEnd, run, shown)
    oldLine = run.oldEnd
    newLine = run.newEnd
  }
  between(oldLine, block.oldEnd, newLine, block.newEnd)
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

/** The lines a note stands for: the old lines of its run, or of the move that left, that have no row (`Remark.open` false), none else */
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
  const { move } = part
  const line = (kind: LineRowKind, oldLine: number | null, newLine: number | null): LineRow => ({
    kind,
    lines: [oldLine, newLine],
    block: index,
    blockRow: row,
    blockRows: parts.rowCount,
    // a line of a move has the colour of the move: pale where the line was edited on the way, as a block with inner ranges
    tint: move === null ? part.tint : isEditedLine(move, (move.at === "source" ? oldLine : newLine) ?? -1) ? "pale" : "full",
    run: part.run,
    move,
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
    case "landed": {
      // an edited line of a move is the pair of its old line, which stands where the lines left, and its new one
      const newLine = part.newStart + offset
      return move !== null && isEditedLine(move, newLine) ? line("inline", move.move.oldStart + newLine - move.move.newStart, newLine) : line("added", null, newLine)
    }
  }
}

/**
 * Whether `line` of the side of an end of a move is a line the move says was edited on the way (`LineMove.edited`, by
 * its place in the move); never a blank line that moved beside it
 */
function isEditedLine({ at, move }: MoveEnd, line: number): boolean {
  const place = line - (at === "source" ? move.oldStart : move.newStart)
  if (place < 0 || place >= move.len) {
    return false
  }
  // in order: a move may hold thousands of lines
  let low = 0
  let high = move.edited.length - 1
  while (low <= high) {
    const middle = (low + high) >> 1
    const edited = move.edited[middle]!
    if (edited === place) {
      return true
    }
    if (edited < place) {
      low = middle + 1
    } else {
      high = middle - 1
    }
  }
  return false
}

/**
 * The row of the parts where `line` of a side, or a later line of it, shows; the row count when no row does. An old
 * line without a row of its own gives the row that stands for it: the note of its quiet run or of the move it left
 * with in Inline, the first new line of an ignored block in one column. The old line of an edited line of a move is
 * found where the lines left, never in the row of its pair where they stand now
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
      case "landed":
        return start + offset
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
 * Rust names the comment, the first new line. The lines a move took are nothing to read either (Rust starts a block
 * at a line no move took): they end what is read before them as a quiet run does. The row count when no part shows
 * the line
 */
export function partsStopRow(parts: BlockParts, side: number, line: number): number {
  // the first part of lines to read since the last quiet run
  let first = -1
  for (const [at, part] of parts.parts.entries()) {
    if (part.kind !== "lines") {
      continue
    }
    if (part.run !== null || part.move?.at === "source") {
      first = -1
    } else if (first < 0) {
      first = at
    }
    const start = side === OLD ? part.oldStart : part.newStart
    if (start <= line && line < start + (side === OLD ? part.oldCount : part.newCount)) {
      // a line of a quiet run or one a move took, which Rust never names, is where it shows
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
    case "landed":
      // the old line of an edited pair is a line of where the lines left
      return [clip(part.oldStart, 0, 0, 0), clip(part.newStart, part.newCount, from, to)]
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

/** The notes of the parts that stand at an end of a move, each with its row */
export function partsMoveNotes(parts: BlockParts): { readonly end: MoveEnd; readonly row: number }[] {
  return parts.parts.flatMap((part, at) => (part.kind === "note" && part.remark !== null && part.remark.move !== null ? [{ end: part.remark.move, row: parts.starts[at]! }] : []))
}

/** A pair of lines that Inline shows as one row, with the inner ranges of each line (`[start, end, kind, ...]`) */
export interface InlineRowPair {
  readonly lines: readonly [number, number]
  readonly oldRanges: readonly number[]
  readonly newRanges: readonly number[]
}

/**
 * The pairs of lines of a block that Inline shows as one row each, whatever notes are open: the rows of its `pairs`
 * parts with the ranges of the block, and the edited lines of the moves that stand in it with the ranges of their
 * move (`landed`)
 */
export function inlinePairsOf(block: DiffBlock, index: number, marks: BlockMarks): InlineRowPair[] {
  const pairs: InlineRowPair[] = []
  // no pair of the engine and no edited line of a move: no row of two lines
  if (block.kind === "ignored" || !(block.inlinePairs || marks.destinations.some(end => end.move.edited.length > 0))) {
    return pairs
  }
  for (const part of blockParts(block, index, marks, { mode: "inline", tint: "full", open: NO_NOTES, heading: null }).parts) {
    if (part.kind !== "lines") {
      continue
    }
    if (part.shape === "pairs") {
      for (let row = 0; row < part.newCount; row++) {
        const [oldLine, newLine] = [part.oldStart + row, part.newStart + row]
        pairs.push({ lines: [oldLine, newLine], oldRanges: block.oldRanges[oldLine - block.oldStart] ?? [], newRanges: block.newRanges[newLine - block.newStart] ?? [] })
      }
    } else if (part.shape === "landed" && part.move !== null) {
      const { move } = part.move
      for (const place of move.edited) {
        const newLine = move.newStart + place
        if (newLine >= part.newStart && newLine < part.newStart + part.newCount) {
          pairs.push({ lines: [move.oldStart + place, newLine], oldRanges: move.oldRanges[place] ?? [], newRanges: move.newRanges[place] ?? [] })
        }
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
