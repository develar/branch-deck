import { NEW, rowAt as fileRowAt, rowOfLine } from "@/lib/diff/rows"
import type { FileRows } from "@/lib/diff/rows"

/*
 * The layout of a document of file diffs (docs/design/diff-view.md, "Layout and virtualization"): every item has a known
 * height, so each offset is arithmetic. With soft wraps off every row has the row height; with them on a row has its
 * count of visual lines, which the view computes before the paint. No height is measured from the DOM.
 */

export interface LayoutMetrics {
  /** The height of a row, in CSS px (`rowHeightOf`) */
  rowHeight: number
  /** The height of a file header, the top border of its card included */
  headerHeight: number
  /**
   * Each file a card (the review windows, docs/design/diff-view.md "File cards"): the gap between two cards, the space
   * above the first and below the last, and the bottom border of a card. Absent: the files follow each other
   */
  cards?: { gap: number; padding: number; borderBottom: number }
}

export interface LayoutFile {
  /** The key of the file across layouts: its path */
  key: string
  /** The rows the file shows; null when it is collapsed or shows none */
  rows: FileRows | null
  /** One row under the header says why the file shows no lines, or how its line ends changed */
  notice: boolean
  /** With soft wraps: the visual lines before each row, `rowCount + 1` entries; absent, one per row */
  visualLines?: Uint32Array
}

/** Where the view stands in a document: kept across a change of heights, restored after it */
export interface ScrollAnchor {
  key: string
  /** A line of a side whose row is at the top; absent when the top is in the header or the notice */
  line?: { side: number; line: number }
  /** The pixels from the top of the anchor (the row, or the file) to the top of the viewport */
  offset: number
}

export class DiffLayout {
  readonly files: readonly LayoutFile[]
  readonly metrics: LayoutMetrics
  /** The top of each file, and the height of the document at the end */
  readonly offsets: Float64Array
  private readonly byKey: Map<string, number>

  constructor(files: readonly LayoutFile[], metrics: LayoutMetrics) {
    this.files = files
    this.metrics = metrics
    this.offsets = new Float64Array(files.length + 1)
    this.byKey = new Map()
    const gap = metrics.cards?.gap ?? 0
    this.offsets[0] = metrics.cards?.padding ?? 0
    files.forEach((file, index) => {
      this.byKey.set(file.key, index)
      this.offsets[index + 1] = this.offsets[index]! + this.fileHeight(index) + gap
    })
  }

  /** The height of the document: the files, the gaps between them and the space around them */
  get height(): number {
    if (this.files.length === 0) {
      return 0
    }
    const cards = this.metrics.cards
    return this.offsets[this.files.length]! - (cards?.gap ?? 0) + (cards?.padding ?? 0)
  }

  indexOf(key: string): number {
    return this.byKey.get(key) ?? -1
  }

  /** The header, the notice and the rows of a file, and the bottom border of its card */
  fileHeight(file: number): number {
    return this.rowsOffset(file) + this.rowsHeight(file) + (this.metrics.cards?.borderBottom ?? 0)
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

  /** The `scrollTop` that shows a file at the top: its card in its place, the space above the cards over it */
  fileTop(file: number): number {
    return Math.max(this.offsets[file]! - this.stickyTop, 0)
  }

  /** The file whose header is at the top of a view scrolled to `scrollTop`: the one under the middle of a stuck header */
  fileAtTop(scrollTop: number): number {
    return this.fileAt(scrollTop + this.stickyTop + this.metrics.headerHeight / 2)
  }

  /** The file at `y`: the last one whose top is at or above it */
  fileAt(y: number): number {
    let low = 0
    let high = Math.max(this.files.length - 1, 0)
    while (low < high) {
      const middle = (low + high + 1) >> 1
      if (this.offsets[middle]! <= y) {
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

  /** The anchor at `scrollTop`: the file there, and the line of its row at the top */
  anchorAt(scrollTop: number): ScrollAnchor | null {
    if (this.files.length === 0) {
      return null
    }
    const file = this.fileAt(scrollTop)
    const { key, rows } = this.files[file]!
    const rowsTop = this.offsets[file]! + this.rowsOffset(file)
    if (rows === null || rows.rowCount === 0 || scrollTop < rowsTop) {
      return { key, offset: scrollTop - this.offsets[file]! }
    }
    const index = this.rowAt(file, scrollTop)
    const row = fileRowAt(rows, index)
    // the new side (the middle of a merge) first, else the first side with a line
    const side =
      row.kind === "fold" || row.lines[NEW] != null
        ? NEW
        : Math.max(
            row.lines.findIndex(line => line != null),
            0,
          )
    const line = { side, line: (row.kind === "fold" ? row.lines[side]![0] : row.lines[side]) ?? 0 }
    return { key, line, offset: scrollTop - this.rowTop(file, index) }
  }

  /** The `scrollTop` that puts the anchor where it was; null when its file is gone */
  scrollTopOf(anchor: ScrollAnchor): number | null {
    const file = this.indexOf(anchor.key)
    if (file < 0) {
      return null
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
