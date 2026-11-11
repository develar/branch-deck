import { copySide } from "@/lib/diff/copy-text"
import type { CopyFile } from "@/lib/diff/copy-text"
import type { LineRun } from "@/lib/diff/file-diff-model"
import { inlineOffset, inlinePair, isOwnOldHit } from "@/lib/diff/line-parts"
import type { LineHits } from "@/lib/diff/line-parts"
import { lineText } from "@/lib/diff/line-table"
import { runUnits, sameUnits, unitFrom, unitsIn } from "@/lib/diff/rename-sites"
import type { RenameSite, RunUnits } from "@/lib/diff/rename-sites"
import { NEW, OLD, rowAt, rowOfLine } from "@/lib/diff/rows"
import type { FileRows, LineRow } from "@/lib/diff/rows"

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
  /**
   * Of a hit of an old line that no row and no note shows (a line of a quiet run of a renamed name, which Inline shows
   * once, by its new lines): the text of the new side that stands for it, its line and its columns, [start, end). The
   * hit is revealed at the row of that line (`rowOfHit`) and marked there (`LineHits.stands`); as many columns as none
   * where nothing of the line stands for it (whitespace the new lines have not)
   */
  stand?: { readonly line: number; readonly start: number; readonly end: number }
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
  /** The sites of the renames on a line of a side (`RowCells.renamed`); absent for a file that renames no name */
  renamed?: (side: number, line: number) => readonly RenameSite[]
  /** The run of a renamed name that holds a line of the new side (`RowCells.renamedRun`); absent as `renamed` */
  renamedRun?: (line: number) => LineRun | null
}

/** A hit with its place in the text of its row */
interface PlacedHit {
  readonly hit: DiffFindHit
  readonly place: number
}

/**
 * The hits of `query` in the files, in the order of the rows, case-insensitive. A row shows the lines a copy takes;
 * with a column per side (Split, a merge) a change is searched on every side and an unchanged line on the new side
 * (the middle of a merge). One column shows a deleted row by its old line, and an Inline row by both its lines: the new
 * one, and of the old one the hits of its own (`isOwnOldHit`: one that holds a deleted part, struck through in the
 * row, or a place where the new line inserts text), in the order of the text of the row. The text the two lines of
 * the row share is the new line's: where a whitespace policy pairs lines of another indent, the row shows and the
 * find reads the whitespace of the new line. The old lines of a quiet run that Inline shows once are searched at the
 * note that stands for them (`NoteRow.lines`), before the new lines: their hits are of the old side, and
 * `rowOfLine` gives the note. So are the lines a move took, where they left; the row of an edited line of a move,
 * the pair of its old line and its new one where the lines stand now, is searched by its new line alone, so a hit of
 * the old line is one hit, at the row `rowOfLine` gives it. The old lines of a quiet run of a renamed name have no
 * note of their own: every one of them is searched, and a hit is found at the text of the new lines that stands for
 * it (`DiffFindHit.stand`, `onceRunHits`): the new name for a hit that holds a part of an old name, the same code of
 * the new lines for any other hit, unless the new lines have that hit themselves. A row of a pair strikes the old
 * name of a rename through, so its hits are hits of its own. With `folded` the folded lines of that side are searched
 * too.
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
  /** The hits of one row in the order of its text: by their place in it, the old line first where two stand at one place */
  const addPlaced = (placed: PlacedHit[]) => {
    placed.sort((a, b) => a.place - b.place || a.hit.side - b.hit.side)
    for (const { hit } of placed) {
      hits.push(hit)
    }
  }
  /**
   * The hits of a quiet run of a renamed name that Inline shows once, by the new line of the row each is found at: the
   * hits of its new lines, and of every old line of it the hits the new lines do not show. The two sides are the same
   * code but for the name, wrapped and spaced as each side has it (`RunUnits`). A hit of an old line that holds a part
   * of an old name stands at the new name of that site, one hit at the first name it holds; any other one stands at the
   * same code of the new lines, as far as the line it starts on holds it, and is no hit where a hit of the new lines
   * starts at that code (the new line shows the same text there). A hit of whitespace alone stands where the code
   * after it does, with nothing to mark, and is no hit where a hit of whitespace alone of the new lines stands before
   * that code
   */
  const onceRunHits = (file: number, run: LineRun, renamed: (side: number, line: number) => readonly RenameSite[]): Map<number, PlacedHit[]> => {
    const byLine = new Map<number, PlacedHit[]>()
    const put = (line: number, hit: DiffFindHit, place: number) => {
      const placed = byLine.get(line)
      if (placed === undefined) {
        byLine.set(line, [{ hit, place }])
      } else {
        placed.push({ hit, place })
      }
    }
    const shown: DiffFindHit[] = []
    for (let line = run.newStart; line < run.newEnd; line++) {
      for (const hit of hitsIn(file, NEW, line, -1)) {
        put(line, hit, hit.start)
        shown.push(hit)
      }
    }
    const nowhere = { line: run.newStart, start: 0, end: 0 }
    const { tables } = files[file]!
    // read when a hit of an old line lies outside the names: the units of both sides, the units the hits of the new
    // lines start at, and the units their hits of whitespace alone stand before
    let code: { readonly old: RunUnits; readonly now: RunUnits; readonly taken: ReadonlySet<number>; readonly blanks: ReadonlySet<number> } | null | undefined
    const codeOf = () => {
      if (code === undefined) {
        const old = runUnits(
          line => lineText(tables[OLD]!, line),
          line => renamed(OLD, line),
          run.oldStart,
          run.oldEnd,
        )
        const now = runUnits(
          line => lineText(tables[NEW]!, line),
          line => renamed(NEW, line),
          run.newStart,
          run.newEnd,
        )
        // two sides that are not the same code, which no engine gives: every hit of the old lines counts, at the first row
        if (sameUnits(old, now)) {
          const taken = new Set<number>()
          const blanks = new Set<number>()
          for (const hit of shown) {
            const first = unitsIn(now, hit.line, hit.start, hit.end)?.[0]
            if (first === undefined) {
              blanks.add(unitFrom(now, hit.line, hit.end))
            } else {
              taken.add(first)
            }
          }
          code = { old, now, taken, blanks }
        } else {
          code = null
        }
      }
      return code
    }
    const standOf = (hit: DiffFindHit, names: readonly RenameSite[]): NonNullable<DiffFindHit["stand"]> | null => {
      const name = names.find(site => hit.start < site.end && hit.end > site.start)
      if (name !== undefined) {
        const { line, start, end } = name.twin
        return run.newStart <= line && line < run.newEnd ? { line, start, end } : nowhere
      }
      const sides = codeOf()
      if (sides === null) {
        return nowhere
      }
      const { old, now, taken, blanks } = sides
      const units = unitsIn(old, hit.line, hit.start, hit.end)
      if (units === null) {
        const after = unitFrom(old, hit.line, hit.end)
        if (blanks.has(after)) {
          return null
        }
        const next = Math.min(after, now.count - 1)
        return next < 0 ? nowhere : { line: now.lines[next]!, start: now.starts[next]!, end: now.starts[next]! }
      }
      const [first, last] = units
      if (taken.has(first)) {
        return null
      }
      // as far as the line the hit starts on holds its code
      let end = first
      while (end < last && now.lines[end + 1] === now.lines[first]) {
        end++
      }
      return { line: now.lines[first]!, start: now.starts[first]!, end: now.ends[end]! }
    }
    for (let line = run.oldStart; line < run.oldEnd; line++) {
      const names = renamed(OLD, line)
      for (const hit of hitsIn(file, OLD, line, -1)) {
        const stand = standOf(hit, names)
        if (stand !== null) {
          put(stand.line, { ...hit, stand }, stand.start)
        }
      }
    }
    return byLine
  }
  files.forEach(({ rows, tables, ranges, renamed, renamedRun }, file) => {
    if (rows === null) {
      return
    }
    // the old lines the last closed note stands for, and the hits of the runs of a renamed name that are shown once
    let behind: readonly [number, number] = [0, 0]
    const ofRuns = new Map<LineRun, Map<number, PlacedHit[]>>()
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
        // the old lines a note stands for while Inline shows their run once: found there, the note is where they are
        const [start, end] = row.remark?.open === false ? row.lines[OLD]! : [0, 0]
        for (let line = start; line < end; line++) {
          add(hitsIn(file, OLD, line, -1))
        }
        if (start < end) {
          behind = [start, end]
        }
        continue
      }
      if (!sideBySide && row.kind === "inline") {
        const newHits = hitsIn(file, NEW, row.lines[NEW], -1)
        // the old line of an edited line of a move is a line of where the lines left (`rowOfLine`): found there once,
        // behind its note or in its own row, never a second time in the row of its pair
        const oldHits = row.move === null ? hitsIn(file, OLD, row.lines[OLD], -1) : []
        const newTable = tables[NEW]
        if (oldHits.length === 0 || newTable == null) {
          add(newHits)
          continue
        }
        const pair = inlinePair(lineText(tables[OLD]!, row.lines[OLD]!), lineText(newTable, row.lines[NEW]!), ranges(row, OLD) ?? [], ranges(row, NEW) ?? [])
        // in the order of the text of the row; a deleted part stands before the inserted one, so the old line goes first
        addPlaced([...oldHits.filter(hit => isOwnOldHit(pair, hit.start, hit.end)), ...newHits].map(hit => ({ hit, place: inlineOffset(pair, hit.side, hit.start) })))
        continue
      }
      const newLine = row.lines[NEW]
      const run = !sideBySide && row.kind === "once" && row.run === "renamed" && newLine != null && renamed !== undefined ? (renamedRun?.(newLine) ?? null) : null
      // a line of a run of a renamed name, shown once, whose old lines no note stands for (the note of its rename
      // stands elsewhere, for no line): the hits of the whole run, each at the row of its new line
      if (run !== null && tables[OLD] != null && tables[NEW] != null && !(behind[0] <= run.oldStart && run.oldStart < behind[1])) {
        let found = ofRuns.get(run)
        if (found === undefined) {
          found = onceRunHits(file, run, renamed!)
          ofRuns.set(run, found)
        }
        addPlaced(found.get(newLine!) ?? [])
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

/**
 * The row a hit is revealed at: the row of its line (`rowOfLine`: a line behind a note is that note), or the row of
 * the new line that stands for it (`DiffFindHit.stand`)
 */
export function rowOfHit(rows: FileRows, hit: DiffFindHit): number {
  return hit.stand === undefined ? rowOfLine(rows, hit.side, hit.line) : rowOfLine(rows, NEW, hit.stand.line)
}

/** The key of a line of a file in `hitsByLine` */
export function lineKey(side: number, line: number): string {
  return `${side}:${line}`
}

/**
 * The hits by file and line, for the painter: the current one marked. A hit of a hidden old line that a new line
 * stands for (`DiffFindHit.stand`) is a mark of that new line (`LineHits.stands`), where the row shows it: the marks
 * of a line in their order, two that overlap (two hits in one old name) as one, the current hit where either is
 */
export function hitsByLine(hits: readonly DiffFindHit[], current: number): Map<number, Map<string, LineHits>> {
  const byFile = new Map<number, Map<string, { ranges: number[]; current: number; stands?: number[] }>>()
  // the marks of the lines that stand for hidden hits, until all of a line are known: `[start, end, hit]` each
  const stood = new Map<{ stands?: number[] }, [number, number, number][]>()
  hits.forEach((hit, index) => {
    let lines = byFile.get(hit.file)
    if (lines === undefined) {
      lines = new Map()
      byFile.set(hit.file, lines)
    }
    const key = hit.stand === undefined ? lineKey(hit.side, hit.line) : lineKey(NEW, hit.stand.line)
    let entry = lines.get(key)
    if (entry === undefined) {
      entry = { ranges: [], current: -1 }
      lines.set(key, entry)
    }
    if (hit.stand !== undefined) {
      if (hit.stand.start < hit.stand.end) {
        const marks = stood.get(entry) ?? []
        marks.push([hit.stand.start, hit.stand.end, index === current ? 2 : 1])
        stood.set(entry, marks)
      }
      return
    }
    if (index === current) {
      entry.current = entry.ranges.length / 2
    }
    entry.ranges.push(hit.start, hit.end)
  })
  for (const [entry, marks] of stood) {
    marks.sort((one, other) => one[0] - other[0] || one[1] - other[1])
    const merged: [number, number, number][] = []
    for (const mark of marks) {
      const last = merged.at(-1)
      if (last !== undefined && mark[0] < last[1]) {
        last[1] = Math.max(last[1], mark[1])
        last[2] = Math.max(last[2], mark[2])
      } else {
        merged.push([...mark])
      }
    }
    entry.stands = merged.flat()
  }
  return byFile
}
