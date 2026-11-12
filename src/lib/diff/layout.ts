import type { MoveEnd } from "@/lib/diff/block-parts"
import { isLineRow, NEW, OLD, rowAt as fileRowAt, rowOfLine } from "@/lib/diff/rows"
import type { FileRows, Row } from "@/lib/diff/rows"

/*
 * The layout of a document of file diffs (docs/design/diff-view.md, "Layout and virtualization"): every item has a known
 * height, so each offset is arithmetic. With soft wraps off every row has the row height; with them on a row has its
 * count of visual lines, which the view computes before the paint. The rows are never measured from the DOM; the few
 * blocks of words of a review are (its prologue, the opening of each step): the view hands their heights in as numbers.
 */

export interface LayoutMetrics {
  /** The height of a row, in CSS px (`rowHeightOf`) */
  rowHeight: number
  /** The height of a file header, the top border of its card included */
  headerHeight: number
  /**
   * Each file a card (the review windows, docs/design/diff-view.md "File cards"): the gap between two cards, the space
   * above the first and below the last, the bottom border of a card, and the height of the divider above the card of a
   * file that starts a group (`LayoutFile.divider`). Absent: the files follow each other
   */
  cards?: { gap: number; padding: number; borderBottom: number; divider: number }
}

/** The key of the anchor of a top in the prologue (`ScrollAnchor.key`): no file has it */
export const PROLOGUE_KEY = "\u0000prologue"

/** The key of the anchor of a top in the epilogue (`ScrollAnchor.key`): no file has it */
export const EPILOGUE_KEY = "\u0000epilogue"

/**
 * Where a y of the document is: the prologue above the first section, the opening of a step above its card (the
 * divider of a group too) or in the epilogue below the last card, or the card of a file with the space below it
 */
export type LayoutPart = "prologue" | "opening" | "card"

export interface LayoutFile {
  /** The key of the file across layouts: its path */
  key: string
  /** The rows the file shows; null when it is collapsed or shows none */
  rows: FileRows | null
  /** One row under the header says why the file shows no lines, or how its line ends changed */
  notice: boolean
  /** With soft wraps: the visual lines before each row, `rowCount + 1` entries; absent, one per row */
  visualLines?: Uint32Array
  /**
   * The opening of a step of the review plan on the canvas above its card (cards only): its height in px, as the view
   * measured it; absent, none. It outranks the divider of a group
   */
  opening?: number
  /** A line on the canvas above its card, of a fixed height (cards only): the file starts a group ("Same edit in 3 files") */
  divider?: boolean
  /**
   * A line at the end of its card (cards only, `endsWithFooter`), one row high below the rows: the next section, or the
   * end of the review; absent, none
   */
  footer?: boolean
  /**
   * A side of its rows is wider than its column: the sticky horizontal scrollbar takes a strip of its own below the
   * rows (`SCROLLBAR_HEIGHT`), so at the end of the file it lies under the last row, never over it; absent, none
   */
  scrollbar?: boolean
}

/** The strip of the sticky horizontal scrollbar of a file below its rows (`LayoutFile.scrollbar`, `.dv-hscrolls`) */
export const SCROLLBAR_HEIGHT = 10

/** A card of more rows than this ends with a line naming the next section (`LayoutFile.footer`) */
export const FOOTER_MIN_ROWS = 30

/**
 * Whether the card of a file ends with a line naming what comes after it: a long card, by its rows alone (never by the
 * viewport, so the layout stays arithmetic), whose end is far from its header, and the last card, the end of the review
 */
export function endsWithFooter(rows: FileRows | null, last: boolean): boolean {
  return last || (rows?.rowCount ?? 0) > FOOTER_MIN_ROWS
}

/** Where the view stands in a document: kept across a change of heights, restored after it */
export interface ScrollAnchor {
  /**
   * The key of the file, or `PROLOGUE_KEY` for a top in the prologue, whose offset is from the top of the document, or
   * `EPILOGUE_KEY` for a top in the epilogue, whose offset is from its top
   */
  key: string
  /**
   * A line of a side whose row is at the top; absent when the top is in the header or the notice. Of a row of words
   * that stands for no line (`standsForNoLine`: a heading, the note of lines that all show) it is the line of the row
   * after it, and the offset is negative: the top is that far above the row of the line
   */
  line?: { side: number; line: number }
  /**
   * The offset is from the start of the section (`sectionStart`, the top of its opening), not from its card: the top
   * is in its opening or in the space above it. So an opening that grows grows downwards, below the top
   */
  opening?: true
  /**
   * The offset is from the end of the rows of the file, the last one: the top is below them (a view that lies above
   * the viewport of the scroller it is a block of, the space past the end of a document). So rows that come or go
   * above keep what is below the file where it is
   */
  below?: true
  /**
   * The pixels from the top of the anchor (the row, the start of the section, the file, or the end of its rows) to the
   * top of the viewport
   */
  offset: number
}

/** The mark of a card `DiffLayout.passedCards` reported */
const PASSED = 4

export class DiffLayout {
  readonly files: readonly LayoutFile[]
  readonly metrics: LayoutMetrics
  /** The top of the card of each file (below its opening), and the height of the document at the end */
  readonly offsets: Float64Array
  /** The height of the prologue above the first section, as the view measured it (cards only); 0 for none */
  readonly prologue: number
  /**
   * The height of the epilogue below the last card, a gap above it, as the view measured it (cards and files only): the
   * openings of the steps of no card told after the last card; 0 for none
   */
  readonly epilogue: number
  /**
   * The empty space past the end of the document (the review windows, `view` of the constructor), as IntelliJ's editor
   * scrolls past the end of a file: as much as the view needs to show its last stop at its top (`lastStopTop`); 0 for
   * none, and where the last section is as high as the view
   */
  readonly pastEnd: number
  private readonly byKey: Map<string, number>

  /**
   * `view`: the height of the view the document scrolls past its end for, so every stop of it reaches the top (the
   * review windows); 0 for a document that ends at its end (a block of another scroller, a view of a file)
   */
  constructor(files: readonly LayoutFile[], metrics: LayoutMetrics, prologue = 0, epilogue = 0, view = 0) {
    this.files = files
    this.metrics = metrics
    this.prologue = metrics.cards === undefined ? 0 : prologue
    this.epilogue = metrics.cards === undefined || files.length === 0 ? 0 : epilogue
    this.offsets = new Float64Array(files.length + 1)
    this.byKey = new Map()
    const gap = metrics.cards?.gap ?? 0
    // the prologue below the space above the cards, a gap after it; the opening of a file lies between the gap above
    // its card and the card
    this.offsets[0] = (metrics.cards?.padding ?? 0) + (this.prologue > 0 ? this.prologue + gap : 0) + this.openingHeight(0)
    files.forEach((file, index) => {
      this.byKey.set(file.key, index)
      this.offsets[index + 1] = this.offsets[index]! + this.fileHeight(index) + gap + this.openingHeight(index + 1)
    })
    this.pastEnd = view > 0 && files.length > 0 ? Math.max(view - (this.contentHeight - this.lastStopTop), 0) : 0
  }

  /**
   * The height of the document: the prologue, the files, the epilogue, the gaps between them and the space around them,
   * then the space past its end (`pastEnd`)
   */
  get height(): number {
    return this.contentHeight + this.pastEnd
  }

  /** The prologue, the files, the epilogue, the gaps between them and the space around them: the end of what shows */
  get contentHeight(): number {
    const padding = this.metrics.cards?.padding ?? 0
    if (this.files.length === 0) {
      return this.prologue > 0 ? padding + this.prologue + padding : 0
    }
    return this.epilogue > 0 ? this.epilogueTop + this.epilogue + padding : this.offsets[this.files.length]! - (this.metrics.cards?.gap ?? 0) + padding
  }

  /**
   * The lowest `scrollTop` a stop of the document needs to be at the top: the stop at the card of the last file
   * (`stopTop`, below the heading of its group the heading; a change N shows in it lands there at most), else, below an
   * epilogue, its end less a row, as the top of its last opening is at most
   */
  get lastStopTop(): number {
    const last = this.files.length - 1
    if (last < 0) {
      return 0
    }
    return this.epilogue > 0 ? Math.max(this.epilogueTop + this.epilogue - this.metrics.rowHeight - this.stickyTop, 0) : this.stopTop(last)
  }

  /** The top of the epilogue: a gap below the last card */
  get epilogueTop(): number {
    return this.offsets[this.files.length]!
  }

  indexOf(key: string): number {
    return this.byKey.get(key) ?? -1
  }

  /**
   * The space above the card of a file that belongs to it: the opening of its step, else the divider of its group; 0
   * without either, without cards, and past the last file
   */
  openingHeight(file: number): number {
    const cards = this.metrics.cards
    const { opening, divider } = this.files[file] ?? {}
    if (cards === undefined) {
      return 0
    }
    return opening ?? (divider === true ? cards.divider : 0)
  }

  /** Where the section of a file starts: the top of its opening, else of its card; `fileAt` of a y below it is the file */
  sectionStart(file: number): number {
    return this.offsets[file]! - this.openingHeight(file)
  }

  /**
   * The header, the notice and the rows of a file, the strip of its scrollbar, the line at the end of its card and the
   * bottom border of the card
   */
  fileHeight(file: number): number {
    return this.rowsOffset(file) + this.rowsHeight(file) + this.scrollbarHeight(file) + this.footerHeight(file) + (this.metrics.cards?.borderBottom ?? 0)
  }

  /** The strip of the horizontal scrollbar of a file below its rows: 0 without one, and without rows */
  scrollbarHeight(file: number): number {
    const { scrollbar, rows } = this.files[file]!
    return scrollbar === true && rows !== null ? SCROLLBAR_HEIGHT : 0
  }

  /** The line at the end of the card of a file, below its rows: one row high; 0 without one, and without cards */
  footerHeight(file: number): number {
    return this.metrics.cards !== undefined && this.files[file]!.footer === true ? this.metrics.rowHeight : 0
  }

  /** The rows of a file */
  rowsHeight(file: number): number {
    return this.visualLines(file, this.files[file]!.rows?.rowCount ?? 0) * this.metrics.rowHeight
  }

  /** The offset of the first row in its file: below the header and the notice */
  rowsOffset(file: number): number {
    return this.metrics.headerHeight + (this.files[file]!.notice ? this.metrics.rowHeight : 0)
  }

  /** The visual lines before row `row` of a file */
  private visualLines(file: number, row: number): number {
    return this.files[file]!.visualLines?.[row] ?? row
  }

  /** The top of a row in the document */
  rowTop(file: number, row: number): number {
    return this.offsets[file]! + this.rowsOffset(file) + this.visualLines(file, row) * this.metrics.rowHeight
  }

  rowHeight(file: number, row: number): number {
    return (this.visualLines(file, row + 1) - this.visualLines(file, row)) * this.metrics.rowHeight
  }

  /**
   * Whether a row of a file is in view at `scrollTop` of a view `viewHeight` high: whole below the header stuck at
   * the top, and above the end of the view. What a key that acts on the rows in view takes (the notes that open, the
   * note of a move)
   */
  rowInView(file: number, row: number, scrollTop: number, viewHeight: number): boolean {
    const top = this.rowTop(file, row)
    return top >= scrollTop + this.stickyTop + this.metrics.headerHeight && top + this.rowHeight(file, row) <= scrollTop + viewHeight
  }

  /** The rows of a file that are in view (`rowInView`) at `scrollTop` of a view `viewHeight` high: [from, to); empty when none is */
  rowsInView(file: number, scrollTop: number, viewHeight: number): readonly [number, number] {
    const count = this.files[file]!.rows?.rowCount ?? 0
    // the row at the edge below the stuck header, which may cut it
    let from = Math.max(this.rowAt(file, scrollTop + this.stickyTop + this.metrics.headerHeight), 0)
    while (from < count && !this.rowInView(file, from, scrollTop, viewHeight) && this.rowTop(file, from) < scrollTop + viewHeight) {
      from++
    }
    let to = from
    while (to < count && this.rowInView(file, to, scrollTop, viewHeight)) {
      to++
    }
    return [from, to]
  }

  /**
   * Where a file header sticks below the top of the view: the space above the first card, so a card at the top keeps
   * the space it has at the top of the document (`.dv-card > .dv-header` in diff-view.css)
   */
  get stickyTop(): number {
    return this.metrics.cards?.padding ?? 0
  }

  /**
   * The `scrollTop` that shows a file at the top: its opening and the space above the cards over its card, so a step
   * shown starts with its opening
   */
  fileTop(file: number): number {
    return Math.max(this.sectionStart(file) - this.stickyTop, 0)
  }

  /**
   * The `scrollTop` that shows the card of a file at the top, its opening above the view: the least top of a change,
   * a hit of the find or a declaration shown in it, so their place on screen never depends on the height of the opening
   */
  cardTop(file: number): number {
    return Math.max(this.offsets[file]! - this.stickyTop, 0)
  }

  /**
   * The `scrollTop` a stop at the card of a file goes to (J and K at a card, a row of the tree, the least top of a
   * change, a hit of the find or a declaration shown): its card at the top, an opening above the view whole, as the gap
   * at the end of its layer keeps the opening off the top; the heading of a group (`divider`), which stands right above
   * the card, at the top with it, so it shows whole and never a cut edge of it under the top of the view
   */
  stopTop(file: number): number {
    const { opening, divider } = this.files[file] ?? {}
    return opening === undefined && divider === true && this.metrics.cards !== undefined ? this.fileTop(file) : this.cardTop(file)
  }

  /** The file whose header is at the top of a view scrolled to `scrollTop`: the one under the middle of a stuck header */
  fileAtTop(scrollTop: number): number {
    return this.fileAt(scrollTop + this.stickyTop + this.metrics.headerHeight / 2)
  }

  /**
   * The file a user's scroll to `scrollTop` shows, the one the tree and the map follow: the one at the top
   * (`fileAtTop`), except at the end of a document that does not scroll past its end (`pastEnd`), where the last file is
   * in view entire and its header can never reach the top; past the end it is the one at the top there too
   */
  fileShownAt(scrollTop: number, viewHeight: number): number {
    return this.files.length > 0 && scrollTop + viewHeight >= this.height - 1 ? this.files.length - 1 : this.fileAtTop(scrollTop)
  }

  /**
   * The part of the document a user's scroll to `scrollTop` shows, of the file `fileShownAt` names: where the stuck
   * header would be (`fileAtTop`); of the last file at the end of the document its card
   */
  partShownAt(scrollTop: number, viewHeight: number): LayoutPart {
    const part = this.partAt(scrollTop + this.stickyTop + this.metrics.headerHeight / 2)
    return part === "prologue" || this.fileShownAt(scrollTop, viewHeight) === this.fileAtTop(scrollTop) ? part : "card"
  }

  /**
   * The part of the document at `y`: the prologue above the first section, the opening of the file at `y`, else its
   * card; the epilogue is an opening (of the last file, which `fileAt` names there)
   */
  partAt(y: number): LayoutPart {
    if (this.prologue > 0 && (this.files.length === 0 || y < this.sectionStart(0))) {
      return "prologue"
    }
    if (this.epilogue > 0 && y >= this.epilogueTop) {
      return "opening"
    }
    const file = this.fileAt(y)
    return this.files.length > 0 && y < this.offsets[file]! && this.files[file]!.opening !== undefined && this.metrics.cards !== undefined ? "opening" : "card"
  }

  /**
   * The file at `y`: the last one whose section (its opening, else its card) starts at or above it; a gap is the file
   * above it, the prologue the first
   */
  fileAt(y: number): number {
    let low = 0
    let high = Math.max(this.files.length - 1, 0)
    while (low < high) {
      const middle = (low + high + 1) >> 1
      if (this.sectionStart(middle) <= y) {
        low = middle
      } else {
        high = middle - 1
      }
    }
    return low
  }

  /**
   * The cards that passed entirely through a view scrolled to `scrollTop`, `viewHeight` high, since `edges` started:
   * the top and the bottom of each card in view now are marked in `edges` by its key (1 the top, 2 the bottom, 4
   * reported), a card both of whose edges were in view, at once or one after the other, is reported once. A card
   * `ready` refuses (its diff not read yet, so its height is not its own) marks nothing
   */
  passedCards(scrollTop: number, viewHeight: number, edges: Map<string, number>, ready: (file: number) => boolean): number[] {
    if (this.files.length === 0) {
      return []
    }
    const bottom = scrollTop + viewHeight
    const passed: number[] = []
    for (let file = this.fileAt(scrollTop), last = this.fileAt(bottom); file <= last; file++) {
      const key = this.files[file]!.key
      const marks = edges.get(key) ?? 0
      if ((marks & PASSED) !== 0 || !ready(file)) {
        continue
      }
      const top = this.offsets[file]!
      const end = top + this.fileHeight(file)
      const now = marks | (top >= scrollTop - 1 && top <= bottom + 1 ? 1 : 0) | (end >= scrollTop - 1 && end <= bottom + 1 ? 2 : 0)
      if (now === 3) {
        passed.push(file)
        edges.set(key, now | PASSED)
      } else if (now !== marks) {
        edges.set(key, now)
      }
    }
    return passed
  }

  /** The row of a file at `y` of the document, clamped to its rows; -1 when the file has none */
  rowAt(file: number, y: number): number {
    const rows = this.files[file]!.rows
    if (rows === null || rows.rowCount === 0) {
      return -1
    }
    const line = Math.floor((y - this.offsets[file]! - this.rowsOffset(file)) / this.metrics.rowHeight)
    const visualLines = this.files[file]!.visualLines
    if (visualLines === undefined) {
      return Math.min(Math.max(line, 0), rows.rowCount - 1)
    }
    let low = 0
    let high = rows.rowCount - 1
    while (low < high) {
      const middle = (low + high + 1) >> 1
      if (visualLines[middle]! <= line) {
        low = middle
      } else {
        high = middle - 1
      }
    }
    return low
  }

  /**
   * The anchor at `scrollTop`: the file there, and the line of its row at the top; of a row of words that stands for
   * no line the line of the first row after it that has one, the offset counted from that row (negative), so the row
   * of words stays at the top across a new layout: `rowOfLine` never gives such a row. A top in the prologue (or the space
   * above it) keeps its offset from the top of the document; one in an opening, from the start of its section, so an
   * opening that grows grows below it. Below the prologue (the gap, the space above the cards) and below the rows of a
   * file (the line at the end of its card, its bottom border, the gap) it is the start of the next section: a scroll
   * to a file (`fileTop`) stays where it is when the prologue, the file above it or its opening changes, and a row of
   * the file above never holds it. A top in the epilogue keeps its offset from the top of the epilogue
   */
  anchorAt(scrollTop: number): ScrollAnchor | null {
    if (this.prologue > 0 && (this.files.length === 0 || scrollTop < (this.metrics.cards?.padding ?? 0) + this.prologue)) {
      return { key: PROLOGUE_KEY, offset: scrollTop }
    }
    if (this.files.length === 0) {
      return null
    }
    if (this.epilogue > 0 && scrollTop >= this.epilogueTop) {
      return { key: EPILOGUE_KEY, offset: scrollTop - this.epilogueTop }
    }
    const file = this.fileAt(scrollTop)
    const { key, rows } = this.files[file]!
    const rowsTop = this.offsets[file]! + this.rowsOffset(file)
    if (file + 1 < this.files.length && scrollTop >= rowsTop + this.rowsHeight(file)) {
      return { key: this.files[file + 1]!.key, opening: true, offset: scrollTop - this.sectionStart(file + 1) }
    }
    if (scrollTop < this.offsets[file]!) {
      return { key, opening: true, offset: scrollTop - this.sectionStart(file) }
    }
    if (rows === null || rows.rowCount === 0 || scrollTop < rowsTop) {
      return { key, offset: scrollTop - this.offsets[file]! }
    }
    // below the rows of the last file: held by their end, however many rows there are
    if (scrollTop >= rowsTop + this.rowsHeight(file)) {
      return { key, below: true, offset: scrollTop - rowsTop - this.rowsHeight(file) }
    }
    let index = this.rowAt(file, scrollTop)
    let row = fileRowAt(rows, index)
    // a row of words that stands for no line is held by the row after it: the lines its words are about
    while (standsForNoLine(row) && index + 1 < rows.rowCount) {
      index++
      row = fileRowAt(rows, index)
    }
    // the new side (the middle of a merge) first, else the first side with a line; of a row of words the first side it
    // stands for lines of (a note that stands for old lines alone: `rowOfLine` of its first one is the note)
    const holds = (side: number) => (isLineRow(row) ? row.lines[side] != null : row.lines[side]![0] < row.lines[side]![1])
    const first = row.lines.findIndex((_, at) => holds(at))
    const side = holds(NEW) || first < 0 ? (isLineRow(row) && first < 0 ? 0 : NEW) : first
    const line = { side, line: (isLineRow(row) ? row.lines[side] : row.lines[side]![0]) ?? 0 }
    return { key, line, offset: scrollTop - this.rowTop(file, index) }
  }

  /**
   * The `scrollTop` that puts the anchor where it was; null when its file is gone. An offset into a prologue or an
   * opening that became lower is clamped to its end: the top of what follows it. A negative offset, of a top in the
   * rows of words above the row of the line, reaches no further up than those rows: where they are gone (another
   * layout, a note that stands for the line again) the top is the row of the line
   */
  scrollTopOf(anchor: ScrollAnchor): number | null {
    if (anchor.key === PROLOGUE_KEY) {
      return Math.min(anchor.offset, this.files.length === 0 ? this.height : this.sectionStart(0))
    }
    if (anchor.key === EPILOGUE_KEY) {
      return this.files.length === 0 ? null : this.epilogueTop + Math.min(anchor.offset, this.epilogue)
    }
    const file = this.indexOf(anchor.key)
    if (file < 0) {
      return null
    }
    if (anchor.opening === true) {
      return this.sectionStart(file) + Math.min(anchor.offset, this.openingHeight(file))
    }
    const rows = this.files[file]!.rows
    if (anchor.below === true) {
      return this.offsets[file]! + this.rowsOffset(file) + this.rowsHeight(file) + anchor.offset
    }
    if (anchor.line === undefined || rows === null || rows.rowCount === 0) {
      return this.offsets[file]! + Math.min(anchor.offset, this.fileHeight(file))
    }
    const row = rowOfLine(rows, anchor.line.side, anchor.line.line)
    let above = row
    while (anchor.offset < 0 && above > 0 && standsForNoLine(fileRowAt(rows, above - 1))) {
      above--
    }
    // a row that shows fewer visual lines keeps the anchor inside it
    return this.rowTop(file, row) + Math.max(Math.min(anchor.offset, Math.max(this.rowHeight(file, row) - 1, 0)), this.rowTop(file, above) - this.rowTop(file, row))
  }
}

/**
 * An end of a move a view stands at: the section that shows it (`LayoutFile.key`), its side, and its first line, the
 * blank lines that moved with the move among them (`MoveEnd.lines`)
 */
export interface MoveHand {
  readonly key: string
  readonly side: number
  readonly line: number
}

/** The end of a move as the place a view stands at, in the section `key` */
export function moveHandOf(key: string, end: MoveEnd): MoveHand {
  return { key, side: end.at === "source" ? OLD : NEW, line: end.lines[0] }
}

/**
 * The note of a move the key of the moves takes in a view at `scrollTop`, `viewHeight` high: the one at the end the
 * view stands at (`hand`: where a jump, Back or Forward landed, or the note N or P stopped under) while that note is
 * in view (`rowInView`), else the first note of a move in view. So one note answers the key at a time, the one that
 * shows its hint, and two presses go to the other end and back also where both ends are in view. `takes`: whether the
 * view can show the other end of a note (an end in another file of the subject that the view does not show is plain
 * words: the key passes it). Null with no such note of a move in view
 */
export function moveNoteOfKey(
  layout: DiffLayout,
  scrollTop: number,
  viewHeight: number,
  hand: MoveHand | null,
  takes: (file: number, end: MoveEnd) => boolean = () => true,
): { file: number; end: MoveEnd; row: number } | null {
  if (layout.files.length === 0) {
    return null
  }
  const at = hand === null ? -1 : layout.indexOf(hand.key)
  const held =
    at < 0 ? undefined : layout.files[at]!.rows?.moveNotes.find(({ end }) => hand !== null && (end.at === "source" ? OLD : NEW) === hand.side && end.lines[0] === hand.line)
  if (held !== undefined && takes(at, held.end) && layout.rowInView(at, held.row, scrollTop, viewHeight)) {
    return { file: at, ...held }
  }
  const top = scrollTop + layout.stickyTop + layout.metrics.headerHeight
  for (let file = layout.fileAt(top), last = layout.fileAt(scrollTop + viewHeight); file <= last; file++) {
    const notes = layout.files[file]!.rows?.moveNotes ?? []
    // the notes are in the order of the rows: the first one at or below the top of the view
    let low = 0
    let high = notes.length
    while (low < high) {
      const middle = (low + high) >> 1
      if (layout.rowTop(file, notes[middle]!.row) < top) {
        low = middle + 1
      } else {
        high = middle
      }
    }
    // the first of them in view that the key can take
    for (let next = low; next < notes.length && layout.rowInView(file, notes[next]!.row, scrollTop, viewHeight); next++) {
      if (takes(file, notes[next]!.end)) {
        return { file, ...notes[next]! }
      }
    }
  }
  return null
}

/**
 * A row of words that stands for no line of any side (the heading of a change, the note of a run whose lines all
 * show): no line finds it (`rowOfLine`), so the scroll position holds it by the row after it
 */
function standsForNoLine(row: Row): boolean {
  return !isLineRow(row) && row.lines.every(([start, end]) => start >= end)
}

/** The visual lines before each row, from the visual lines of each row: the `visualLines` of a `LayoutFile` */
export function visualLinePrefix(rowCount: number, linesOf: (row: number) => number): Uint32Array {
  const prefix = new Uint32Array(rowCount + 1)
  for (let row = 0; row < rowCount; row++) {
    prefix[row + 1] = prefix[row]! + Math.max(linesOf(row), 1)
  }
  return prefix
}
