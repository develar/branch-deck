import { copySide } from "@/lib/diff/copy-text"
import type { CopyFile } from "@/lib/diff/copy-text"
import type { LineHits } from "@/lib/diff/line-parts"
import { lineText } from "@/lib/diff/line-table"
import { NEW, rowAt } from "@/lib/diff/rows"

/*
 * The find in the diffs (docs/design/diff-view.md, "Find in the diffs"): it searches the model, the lines the rows show
 * and, on request, the folded lines, not the rows in the DOM.
 */

export interface FindHit {
  file: number
  side: number
  line: number
  start: number
  end: number
  /** The fold that holds the hit, by its gap; -1 for a line a row shows */
  gap: number
}

/**
 * The hits of `query` in the files, in the order of the rows, case-insensitive. A row shows the lines a copy takes;
 * with a column per side (Split, a merge) a change is searched on every side and an unchanged line on the new side
 * (the middle of a merge). With `folded` the folded lines of that side are searched too.
 */
export function findHits(files: readonly CopyFile[], query: string, { sideBySide, folded }: { sideBySide: boolean; folded: boolean }): FindHit[] {
  const needle = query.toLowerCase()
  const hits: FindHit[] = []
  if (needle === "") {
    return hits
  }
  const search = (file: number, side: number, line: number, gap: number) => {
    const table = files[file]!.tables[side]
    if (table === null || table === undefined || line >= table.lineCount) {
      return
    }
    const text = lineText(table, line).toLowerCase()
    for (let start = text.indexOf(needle); start >= 0; start = text.indexOf(needle, start + needle.length)) {
      hits.push({ file, side, line, start, end: start + needle.length, gap })
    }
  }
  files.forEach(({ rows }, file) => {
    if (rows === null) {
      return
    }
    for (let index = 0; index < rows.rowCount; index++) {
      const row = rowAt(rows, index)
      if (row.kind === "fold") {
        const [start, end] = row.lines[NEW]!
        for (let line = start; folded && line < end; line++) {
          search(file, NEW, line, row.gap)
        }
        continue
      }
      if (row.kind === "note") {
        continue
      }
      const sides = sideBySide ? (row.kind === "context" ? [NEW] : row.lines.map((_, side) => side)) : [copySide(row, false, NEW)]
      for (const side of sides) {
        const line = row.lines[side]
        if (line !== null && line !== undefined) {
          search(file, side, line, -1)
        }
      }
    }
  })
  return hits
}

/** The key of a line of a file in `hitsByLine` */
export function lineKey(side: number, line: number): string {
  return `${side}:${line}`
}

/** The hits by file and line, for the painter: the current one marked */
export function hitsByLine(hits: readonly FindHit[], current: number): Map<number, Map<string, LineHits>> {
  const byFile = new Map<number, Map<string, { ranges: number[]; current: number }>>()
  hits.forEach((hit, index) => {
    let lines = byFile.get(hit.file)
    if (lines === undefined) {
      lines = new Map()
      byFile.set(hit.file, lines)
    }
    const key = lineKey(hit.side, hit.line)
    let entry = lines.get(key)
    if (entry === undefined) {
      entry = { ranges: [], current: -1 }
      lines.set(key, entry)
    }
    if (index === current) {
      entry.current = entry.ranges.length / 2
    }
    entry.ranges.push(hit.start, hit.end)
  })
  return byFile
}
