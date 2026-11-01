import { isLineRow, NEW, rowAt as fileRowAt, rowOfLine } from "@/lib/diff/rows"
import type { FileRows } from "@/lib/diff/rows"

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

/**
 * Where a y of the document is: the prologue above the first section, the opening of a step above its card (the
 * divider of a group too), or the card of a file with the space below it
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
}

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
  /** The key of the file, or `PROLOGUE_KEY` for a top in the prologue, whose offset is from the top of the document */
  key: string
  /** A line of a side whose row is at the top; absent when the top is in the header or the notice */
  line?: { side: number; line: number }
  /**
   * The offset is from the start of the section (`sectionStart`, the top of its opening), not from its card: the top
   * is in its opening or in the space above it. So an opening that grows grows downwards, below the top
   */
  opening?: true
  /** The pixels from the top of the anchor (the row, the start of the section, or the file) to the top of the viewport */
  offset: number
}

export class DiffLayout {
  readonly files: readonly LayoutFile[]
  readonly metrics: LayoutMetrics
  /** The top of the card of each file (below its opening), and the height of the document at the end */
  readonly offsets: Float64Array
  /** The height of the prologue above the first section, as the view measured it (cards only); 0 for none */
  readonly prologue: number
  private readonly byKey: Map<string, number>

  constructor(files: readonly LayoutFile[], metrics: LayoutMetrics, prologue = 0) {
    this.files = files
    this.metrics = metrics
    this.prologue = metrics.cards === undefined ? 0 : prologue
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
  }

  /** The height of the document: the prologue, the files, the gaps between them and the space around them */
  get height(): number {
    const padding = this.metrics.cards?.padding ?? 0
    if (this.files.length === 0) {
      return this.prologue > 0 ? padding + this.prologue + padding : 0
    }
    return this.offsets[this.files.length]! - (this.metrics.cards?.gap ?? 0) + padding
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

  /** The header, the notice and the rows of a file, the line at the end of its card and the bottom border of the card */
  fileHeight(file: number): number {
    return this.rowsOffset(file) + this.rowsHeight(file) + this.footerHeight(file) + (this.metrics.cards?.borderBottom ?? 0)
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

  /** The file whose header is at the top of a view scrolled to `scrollTop`: the one under the middle of a stuck header */
  fileAtTop(scrollTop: number): number {
    return this.fileAt(scrollTop + this.stickyTop + this.metrics.headerHeight / 2)
  }

  /**
   * The file a user's scroll to `scrollTop` shows, the one the tree and the map follow: the one at the top
   * (`fileAtTop`), except at the end of the document, where the last file is in view entire and its header can never
   * reach the top
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

  /** The part of the document at `y`: the prologue above the first section, the opening of the file at `y`, else its card */
  partAt(y: number): LayoutPart {
    if (this.prologue > 0 && (this.files.length === 0 || y < this.sectionStart(0))) {
      return "prologue"
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
   * The anchor at `scrollTop`: the file there, and the line of its row at the top. A top in the prologue (or the space
   * above it) keeps its offset from the top of the document; one in an opening, from the start of its section, so an
   * opening that grows grows below it. Below the prologue (the gap, the space above the cards) and below the rows of a
   * file (the line at the end of its card, its bottom border, the gap) it is the start of the next section: a scroll
   * to a file (`fileTop`) stays where it is when the prologue, the file above it or its opening changes, and a row of
   * the file above never holds it
   */
  anchorAt(scrollTop: number): ScrollAnchor | null {
    if (this.prologue > 0 && (this.files.length === 0 || scrollTop < (this.metrics.cards?.padding ?? 0) + this.prologue)) {
      return { key: PROLOGUE_KEY, offset: scrollTop }
    }
    if (this.files.length === 0) {
      return null
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
    const index = this.rowAt(file, scrollTop)
    const row = fileRowAt(rows, index)
    // the new side (the middle of a merge) first, else the first side with a line
    const side =
      !isLineRow(row) || row.lines[NEW] != null
        ? NEW
        : Math.max(
            row.lines.findIndex(line => line != null),
            0,
          )
    const line = { side, line: (isLineRow(row) ? row.lines[side] : row.lines[side]![0]) ?? 0 }
    return { key, line, offset: scrollTop - this.rowTop(file, index) }
  }

  /**
   * The `scrollTop` that puts the anchor where it was; null when its file is gone. An offset into a prologue or an
   * opening that became lower is clamped to its end: the top of what follows it
   */
  scrollTopOf(anchor: ScrollAnchor): number | null {
    if (anchor.key === PROLOGUE_KEY) {
      return Math.min(anchor.offset, this.files.length === 0 ? this.height : this.sectionStart(0))
    }
    const file = this.indexOf(anchor.key)
    if (file < 0) {
      return null
    }
    if (anchor.opening === true) {
      return this.sectionStart(file) + Math.min(anchor.offset, this.openingHeight(file))
    }
    const rows = this.files[file]!.rows
    if (anchor.line === undefined || rows === null || rows.rowCount === 0) {
      return this.offsets[file]! + Math.min(anchor.offset, this.fileHeight(file))
    }
    const row = rowOfLine(rows, anchor.line.side, anchor.line.line)
    // a row that shows fewer visual lines keeps the anchor inside it
    return this.rowTop(file, row) + Math.min(anchor.offset, Math.max(this.rowHeight(file, row) - 1, 0))
  }
}

/** The visual lines before each row, from the visual lines of each row: the `visualLines` of a `LayoutFile` */
export function visualLinePrefix(rowCount: number, linesOf: (row: number) => number): Uint32Array {
  const prefix = new Uint32Array(rowCount + 1)
  for (let row = 0; row < rowCount; row++) {
    prefix[row + 1] = prefix[row]! + Math.max(linesOf(row), 1)
  }
  return prefix
}
