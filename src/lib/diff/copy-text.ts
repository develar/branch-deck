import { lineText } from "@/lib/diff/line-table"
import type { LineTable } from "@/lib/diff/line-table"
import { NEW, OLD, rowAt } from "@/lib/diff/rows"
import type { FileRows, LineRow } from "@/lib/diff/rows"

/*
 * The text a copy of the diffs gives (docs/design/diff-view.md, "Selection and copy"): built from the model, not from
 * the DOM, so a selection over rows that are no longer painted copies all its lines.
 */

export interface CopyFile {
  rows: FileRows | null
  /** The text of each side; null while a text is not read */
  tables: readonly (LineTable | null)[]
}

/** A point of a selection: a row of a file, and an offset in the text of the line it shows */
export interface CopyPoint {
  file: number
  row: number
  offset: number
}

function compare(a: CopyPoint, b: CopyPoint): number {
  return a.file - b.file || a.row - b.row || a.offset - b.offset
}

/**
 * The side whose line a row shows in a copy: with a column per side (Split, a merge) the side the selection is on; in
 * Unified and Inline the old line of a deleted row and the new line of the others (the deleted parts of an Inline row
 * are not copied)
 */
export function copySide(row: LineRow, sideBySide: boolean, selectionSide: number): number {
  if (sideBySide) {
    return selectionSide
  }
  return row.kind === "deleted" ? OLD : NEW
}

/**
 * The text between two points of a selection, in the order of the diffs: the lines the rows show, one per row, the
 * first and the last one cut at the points. A fold, a gap of Split and a line whose text is not read give nothing.
 */
export function copyText(files: readonly CopyFile[], anchor: CopyPoint, focus: CopyPoint, sideBySide: boolean, selectionSide: number): string {
  const [start, end] = compare(anchor, focus) <= 0 ? [anchor, focus] : [focus, anchor]
  const lines: string[] = []
  for (let file = start.file; file <= end.file; file++) {
    const { rows, tables } = files[file] ?? { rows: null, tables: [] }
    if (rows === null) {
      continue
    }
    const first = file === start.file ? start.row : 0
    const last = file === end.file ? end.row : rows.rowCount - 1
    for (let index = first; index <= last && index < rows.rowCount; index++) {
      const row = rowAt(rows, index)
      if (row.kind === "fold") {
        continue
      }
      const side = copySide(row, sideBySide, selectionSide)
      const line = row.lines[side]
      const table = tables[side]
      if (line === null || line === undefined || table === null || table === undefined) {
        continue
      }
      const text = lineText(table, line)
      const from = file === start.file && index === start.row ? start.offset : 0
      const to = file === end.file && index === end.row ? end.offset : text.length
      lines.push(text.slice(from, Math.max(from, to)))
    }
  }
  return lines.join("\n")
}

/** The whole side of a file: ⌘A in the diffs */
export function copyFile(file: CopyFile, sideBySide: boolean, selectionSide: number): string {
  if (file.rows === null || file.rows.rowCount === 0) {
    return ""
  }
  const last = file.rows.rowCount - 1
  return copyText([file], { file: 0, row: 0, offset: 0 }, { file: 0, row: last, offset: Number.MAX_SAFE_INTEGER }, sideBySide, selectionSide)
}
