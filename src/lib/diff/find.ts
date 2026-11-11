import { copySide } from "@/lib/diff/copy-text"
import type { CopyFile } from "@/lib/diff/copy-text"
import { inlineOffset, inlinePair, isOwnOldHit } from "@/lib/diff/line-parts"
import type { LineHits } from "@/lib/diff/line-parts"
import { lineText } from "@/lib/diff/line-table"
import { NEW, OLD, rowAt } from "@/lib/diff/rows"
import type { LineRow } from "@/lib/diff/rows"

/*
 * The diff hits of the find in the review (docs/design/diff-view.md): it searches the model, the lines the rows show
 * and, on request, the folded lines, not the rows in the DOM.
 */

export interface DiffFindHit {
  kind: "diff"
  file: number
  side: number
  line: number
  start: number
  end: number
  /** The fold that holds the hit, by its gap; -1 for a line a row shows */
  gap: number
}

export interface TextFindHit {
  kind: "text"
  /** -1 the prologue, i the openings above file i, files.length the epilogue */
  file: number
  range: Range
  /** The hidden folds around the hit, outermost first; empty when shown */
  folds: readonly HTMLElement[]
}

export type FindHit = DiffFindHit | TextFindHit

/** A file the find searches: its rows, its texts, and the inner ranges of its lines, which say what an Inline row shows of its old line */
export interface FindFile extends CopyFile {
  /** The inner ranges of the line of a side of a row (`RowCells.ranges`) */
  ranges: (row: LineRow, side: number) => readonly number[] | null
}

/**
 * The hits of `query` in the files, in the order of the rows, case-insensitive. A row shows the lines a copy takes;
 * with a column per side (Split, a merge) a change is searched on every side and an unchanged line on the new side
 * (the middle of a merge). One column shows a deleted row by its old line, and an Inline row by both its lines: the new
 * one, and of the old one the hits of its own (`isOwnOldHit`: one that holds a deleted part, struck through in the
 * row, or a place where the new line inserts text), in the order of the text of the row. The text the two lines of
 * the row share is the new line's: where a whitespace policy pairs lines of another indent, the row shows and the
 * find reads the whitespace of the new line. With `folded` the folded lines of that side are searched too.
 */
export function findHits(files: readonly FindFile[], query: string, { sideBySide, folded }: { sideBySide: boolean; folded: boolean }): DiffFindHit[] {
  const needle = query.toLowerCase()
  const hits: DiffFindHit[] = []
  if (needle === "") {
    return hits
  }
  /** The hits in a line of a side; none while its text is not read */
  const hitsIn = (file: number, side: number, line: number | null | undefined, gap: number): DiffFindHit[] => {
    const table = files[file]!.tables[side]
    if (line === null || line === undefined || table === null || table === undefined || line >= table.lineCount) {
      return []
    }
    const text = lineText(table, line).toLowerCase()
    const found: DiffFindHit[] = []
    for (let start = text.indexOf(needle); start >= 0; start = text.indexOf(needle, start + needle.length)) {
      found.push({ kind: "diff", file, side, line, start, end: start + needle.length, gap })
    }
    return found
  }
  // a loop, not `push(...found)`: a line may hold more hits than a call takes arguments (a minified file, one letter)
  const add = (found: readonly DiffFindHit[]) => {
    for (const hit of found) {
      hits.push(hit)
    }
  }
  files.forEach(({ rows, tables, ranges }, file) => {
    if (rows === null) {
      return
    }
    for (let index = 0; index < rows.rowCount; index++) {
      const row = rowAt(rows, index)
      if (row.kind === "fold") {
        const [start, end] = row.lines[NEW]!
        for (let line = start; folded && line < end; line++) {
          add(hitsIn(file, NEW, line, row.gap))
        }
        continue
      }
      if (row.kind === "note") {
        continue
      }
      if (!sideBySide && row.kind === "inline") {
        const newHits = hitsIn(file, NEW, row.lines[NEW], -1)
        const oldHits = hitsIn(file, OLD, row.lines[OLD], -1)
        const newTable = tables[NEW]
        if (oldHits.length === 0 || newTable == null) {
          add(newHits)
          continue
        }
        const pair = inlinePair(lineText(tables[OLD]!, row.lines[OLD]!), lineText(newTable, row.lines[NEW]!), ranges(row, OLD) ?? [], ranges(row, NEW) ?? [])
        // in the order of the text of the row; a deleted part stands before the inserted one, so the old line goes first
        const placed = [...oldHits.filter(hit => isOwnOldHit(pair, hit.start, hit.end)), ...newHits].map(hit => ({ hit, place: inlineOffset(pair, hit.side, hit.start) }))
        placed.sort((a, b) => a.place - b.place || a.hit.side - b.hit.side)
        for (const { hit } of placed) {
          hits.push(hit)
        }
        continue
      }
      const sides = sideBySide ? (row.kind === "context" ? [NEW] : row.lines.map((_, side) => side)) : [copySide(row, false, NEW)]
      for (const side of sides) {
        add(hitsIn(file, side, row.lines[side], -1))
      }
    }
  })
  return hits
}

/** Hits in each segment independently, ignoring case; a hit never spans two blocks */
export function findInSegments(segments: readonly string[], query: string): Array<{ segment: number; start: number; end: number }> {
  const needle = query.toLowerCase()
  if (needle === "") {
    return []
  }
  return segments.flatMap((segment, index) => {
    const text = segment.toLowerCase()
    const hits: Array<{ segment: number; start: number; end: number }> = []
    for (let start = text.indexOf(needle); start >= 0; start = text.indexOf(needle, start + needle.length)) {
      hits.push({ segment: index, start, end: start + needle.length })
    }
    return hits
  })
}

/** Merges hits in document order: story text before the diff rows of its file */
export function mergeHits(text: readonly TextFindHit[], diff: readonly DiffFindHit[]): FindHit[] {
  const hits: FindHit[] = []
  let textIndex = 0
  let diffIndex = 0
  while (textIndex < text.length || diffIndex < diff.length) {
    const textHit = text[textIndex]
    const diffHit = diff[diffIndex]
    if (diffHit === undefined || (textHit !== undefined && textHit.file <= diffHit.file)) {
      hits.push(textHit!)
      textIndex++
    } else {
      hits.push(diffHit)
      diffIndex++
    }
  }
  return hits
}

/** The key of a line of a file in `hitsByLine` */
export function lineKey(side: number, line: number): string {
  return `${side}:${line}`
}

/** The hits by file and line, for the painter: the current one marked */
export function hitsByLine(hits: readonly DiffFindHit[], current: number): Map<number, Map<string, LineHits>> {
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
