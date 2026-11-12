import type { LineRun, Rename, TextDiff } from "@/lib/diff/file-diff-model"
import { RANGE_STRIDE, RENAMED_RANGE } from "@/lib/diff/line-parts"

/*
 * The sites of the renames of a diff (docs/design/diff-view.md, "The sites of a rename"): Rust says which names a
 * change renamed and where each stands on both sides (`TextDiff.marks.renames`, place `i` of the old text is place
 * `i` of the new one). A view draws the name at a site in a mark of its own and never as a change: the ranges of the
 * engine inside it are left out, and a row that shows a pair of lines as one strikes the old name through before the
 * new one, each as a whole name. What such a line is to a reader (a run of the kind `renamed`, quiet or a label, or a
 * line with another change, in no run) is Rust's to say and is read from the runs, never from here. On a line of such
 * a run the names are all that changed (the same tokens but for them), so nothing else of it is marked: what the
 * engine marked beside a name there is the same code, joined to the name by its word diff or spaced otherwise.
 *
 * A name that another file of the subject declares and this file uses at every place (`SubjectMarks.used`) is a
 * rename of the file as any other, without a declaration in it: its sites are drawn the same, and its note stands at
 * its first site (`namedRenames`).
 */

const OLD = 0
const NEW = 1

/** A site of a rename on a line of a side: where the name stands, and the same site on the other side */
export interface RenameSite {
  readonly rename: Rename
  /** The place of the rename among the renames a view names in the file (`namedRenames`) */
  readonly index: number
  /** The number of the site among the places of its rename, from 0 */
  readonly place: number
  readonly side: number
  /** The line of its side, from 0, and the UTF-16 columns of the name in it: [start, end) */
  readonly line: number
  readonly start: number
  readonly end: number
  /** The same place on the other side: the name before the change for a site of the new side, after it for one of the old */
  readonly twin: RenameSite
}

export interface RenameSites {
  /** The sites of each side by line, each line's in the order of its columns */
  readonly byLine: readonly ReadonlyMap<number, readonly RenameSite[]>[]
  /** The sites of each rename of `namedRenames` on the new side, by their number */
  readonly ofRename: readonly (readonly RenameSite[])[]
}

// one list per diff: every reader of the sites of a file with a used name asks
const renamesOfDiffs = new WeakMap<TextDiff, readonly Rename[]>()

/**
 * The renames a view names in a file, in one order: those the file declares (`TextDiff.marks.renames`, each by its
 * number there), then the names another file of the subject declares and this one uses at every place
 * (`SubjectMarks.used`), which declare none here
 */
export function namedRenames(diff: TextDiff): readonly Rename[] {
  const used = diff.marks.subject?.used ?? []
  if (used.length === 0) {
    return diff.marks.renames
  }
  let renames = renamesOfDiffs.get(diff)
  if (renames === undefined) {
    renames = [...diff.marks.renames, ...used.map(it => it.rename)]
    renamesOfDiffs.set(diff, renames)
  }
  return renames
}

const NO_SITES: readonly RenameSite[] = []
const NONE: RenameSites = { byLine: [new Map(), new Map()], ofRename: [] }

// read once per diff: every row of a line with a site asks
const sitesOfDiffs = new WeakMap<TextDiff, RenameSites>()

/** The sites of the renames of a diff; the places a rename names on one side alone, which no engine gives, are left out */
export function renameSites(diff: TextDiff): RenameSites {
  const renames = namedRenames(diff)
  if (renames.length === 0) {
    return NONE
  }
  const known = sitesOfDiffs.get(diff)
  if (known !== undefined) {
    return known
  }
  const byLine: Map<number, RenameSite[]>[] = [new Map(), new Map()]
  const add = (site: RenameSite) => {
    const sites = byLine[site.side]!.get(site.line)
    if (sites === undefined) {
      byLine[site.side]!.set(site.line, [site])
    } else {
      sites.push(site)
    }
  }
  const ofRename = renames.map((rename, index) => {
    const sites: RenameSite[] = []
    const count = Math.floor(Math.min(rename.oldSites.length, rename.newSites.length) / RANGE_STRIDE)
    for (let place = 0; place < count; place++) {
      const at = place * RANGE_STRIDE
      const before = { rename, index, place, side: OLD, line: rename.oldSites[at]!, start: rename.oldSites[at + 1]!, end: rename.oldSites[at + 2]! } as {
        -readonly [key in keyof RenameSite]: RenameSite[key]
      }
      const after: RenameSite = { rename, index, place, side: NEW, line: rename.newSites[at]!, start: rename.newSites[at + 1]!, end: rename.newSites[at + 2]!, twin: before }
      before.twin = after
      add(before)
      add(after)
      sites.push(after)
    }
    return sites
  })
  for (const lines of byLine) {
    for (const sites of lines.values()) {
      sites.sort((a, b) => a.start - b.start)
    }
  }
  const sites: RenameSites = { byLine, ofRename }
  sitesOfDiffs.set(diff, sites)
  return sites
}

/** The sites on a line of a side, in the order of their columns; none for most lines */
export function sitesOfLine(sites: RenameSites, side: number, line: number): readonly RenameSite[] {
  return sites.byLine[side]?.get(line) ?? NO_SITES
}

/**
 * The ranges of a line (`[start, end, kind, ...]`) with its sites: each name a range of the kind `RENAMED_RANGE`, and
 * of the ranges of the engine what lies outside the names. A marker of an empty side that touches a name goes with
 * it: the name as a whole is what changed there
 */
export function withSites(ranges: readonly number[] | null, sites: readonly RenameSite[]): readonly number[] | null {
  if (sites.length === 0) {
    return ranges
  }
  const pieces: [number, number, number][] = sites.map(site => [site.start, site.end, RENAMED_RANGE])
  for (let at = 0; ranges !== null && at + 2 < ranges.length; at += RANGE_STRIDE) {
    const start = ranges[at]!
    const end = ranges[at + 1]!
    const kind = ranges[at + 2]!
    if (start === end) {
      if (!sites.some(site => site.start <= start && start <= site.end)) {
        pieces.push([start, end, kind])
      }
      continue
    }
    let from = start
    for (const site of sites) {
      if (site.end <= from) {
        continue
      }
      if (site.start >= end) {
        break
      }
      if (site.start > from) {
        pieces.push([from, site.start, kind])
      }
      from = site.end
    }
    if (from < end) {
      pieces.push([from, end, kind])
    }
  }
  return pieces.toSorted((one, other) => one[0] - other[0] || one[1] - other[1]).flat()
}

/**
 * The ranges of a pair of lines that one row shows (`inlinePair`: range `i` of the old line is range `i` of the new
 * one) with the sites of the pair, `sites` those of the new line whose twin is on the old one: each name is one range
 * of the kind `RENAMED_RANGE` on both sides, which takes the ranges of the engine inside it; a range that reaches out
 * of a name is cut there, and what is left of it is a range of its own, inserted or deleted where one side of it is
 * empty. Both lists stay as long as each other. Ranges that are no pair (lists of two lengths) are left as they are
 */
export function pairedWithSites(oldRanges: readonly number[], newRanges: readonly number[], sites: readonly RenameSite[]): readonly [readonly number[], readonly number[]] {
  if (sites.length === 0 || oldRanges.length !== newRanges.length) {
    return [oldRanges, newRanges]
  }
  const before: number[] = []
  const after: number[] = []
  const emit = (oldStart: number, oldEnd: number, newStart: number, newEnd: number, given?: number) => {
    if (oldStart >= oldEnd && newStart >= newEnd && given === undefined) {
      return
    }
    const kind = given ?? (oldStart >= oldEnd ? 0 : newStart >= newEnd ? 1 : 2)
    before.push(oldStart, Math.max(oldStart, oldEnd), kind)
    after.push(newStart, Math.max(newStart, newEnd), kind)
  }
  type Pending = { oldStart: number; oldEnd: number; newStart: number; newEnd: number; kind: number | undefined }
  let at = 0
  const read = (): Pending | null =>
    at + 2 < oldRanges.length ? { oldStart: oldRanges[at]!, oldEnd: oldRanges[at + 1]!, newStart: newRanges[at]!, newEnd: newRanges[at + 1]!, kind: oldRanges[at + 2]! } : null
  let range = read()
  const next = () => {
    at += RANGE_STRIDE
    range = read()
  }
  for (const site of sites) {
    const { start: newStart, end: newEnd } = site
    const { start: oldStart, end: oldEnd } = site.twin
    // the ranges before the name on both sides
    while (range !== null && range.oldEnd <= oldStart && range.newEnd <= newStart) {
      emit(range.oldStart, range.oldEnd, range.newStart, range.newEnd, range.kind)
      next()
    }
    let first = true
    // the ranges that hold a part of the name, until one starts after it on both sides
    while (range !== null && !(range.oldStart >= oldEnd && range.newStart >= newEnd)) {
      if (first) {
        // what the first of them holds before the name
        emit(Math.min(range.oldStart, oldStart), oldStart, Math.min(range.newStart, newStart), newStart)
        first = false
      }
      if (range.oldEnd > oldEnd || range.newEnd > newEnd) {
        // it reaches past the name: what is left of it goes on, maybe into the next name
        range = {
          oldStart: Math.max(range.oldStart, oldEnd),
          oldEnd: Math.max(range.oldEnd, oldEnd),
          newStart: Math.max(range.newStart, newEnd),
          newEnd: Math.max(range.newEnd, newEnd),
          kind: undefined,
        }
        break
      }
      next()
    }
    emit(oldStart, oldEnd, newStart, newEnd, RENAMED_RANGE)
  }
  while (range !== null) {
    emit(range.oldStart, range.oldEnd, range.newStart, range.newEnd, range.kind)
    next()
  }
  return [before, after]
}

/**
 * The ranges of a line of a side (`[start, end, kind, ...]`, the ranges of the engine) as a view draws them: with its
 * renamed names (`withSites`), and on a line of a run of a renamed name the names alone
 */
export function lineRanges(diff: TextDiff, side: number, line: number, ranges: readonly number[] | null): readonly number[] | null {
  const sites = sitesOfLine(renameSites(diff), side, line)
  return sites.length === 0 ? ranges : withSites(renamedRunOf(diff, line, side) === null ? ranges : null, sites)
}

/**
 * The ranges of the pair of an old and a new line that one row shows, with the sites both lines hold at one place
 * (`pairedWithSites`): the ranges as they are for a pair without such a site, and the names alone for a pair of a run
 * of a renamed name
 */
export function pairRanges(
  diff: TextDiff,
  [oldLine, newLine]: readonly [number, number],
  oldRanges: readonly number[],
  newRanges: readonly number[],
): readonly [readonly number[], readonly number[]] {
  const paired = sitesOfLine(renameSites(diff), NEW, newLine).filter(site => site.twin.line === oldLine)
  if (paired.length === 0) {
    return [oldRanges, newRanges]
  }
  return renamedRunOf(diff, newLine) === null ? pairedWithSites(oldRanges, newRanges, paired) : pairedWithSites([], [], paired)
}

/**
 * The run of the kind `renamed` (`TextDiff.marks.runs`) that holds `line` of a side (the new one by default): lines
 * that changed in a renamed name alone, however they were wrapped and spaced, quiet or a label; null for a line of none
 */
export function renamedRunOf(diff: TextDiff, line: number, side = NEW): LineRun | null {
  const { runs } = diff.marks
  const start = (run: LineRun) => (side === OLD ? run.oldStart : run.newStart)
  // the runs come in the order of both sides: the last one that starts at or before the line
  let low = 0
  let high = runs.length - 1
  while (low < high) {
    const middle = (low + high + 1) >> 1
    if (start(runs[middle]!) <= line) {
      low = middle
    } else {
      high = middle - 1
    }
  }
  const run = runs[low]
  return run !== undefined && run.kind === "renamed" && start(run) <= line && line < (side === OLD ? run.oldEnd : run.newEnd) ? run : null
}

/**
 * The text of the lines of one side of a run of a renamed name, unit by unit: each character that is no whitespace
 * outside the renamed names, and each renamed name as one unit. The two sides of such a run are the same tokens but
 * for the name (Rust says so), so they are the same units in the same order however the lines are wrapped and spaced:
 * unit `i` of the old side stands at unit `i` of the new one. What the find reads the hidden old lines of the run by
 */
export interface RunUnits {
  readonly count: number
  /** The line of each unit, and its UTF-16 columns in it: [start, end) */
  readonly lines: readonly number[]
  readonly starts: readonly number[]
  readonly ends: readonly number[]
  /** The character of each unit; -1 for a renamed name */
  readonly codes: readonly number[]
  /** The first line of the run on its side, and the first unit of each of its lines from there, the count at the end */
  readonly firstLine: number
  readonly offsets: readonly number[]
}

/** A space, a tab, a form feed or what is left of a line end: the whitespace between the tokens of a line */
function isBlank(code: number): boolean {
  return code === 32 || code === 9 || code === 12 || code === 11 || code === 13
}

/** The units of the lines `[start, end)` of a side: `textOf` the text of a line, `sitesOf` its sites in the order of their columns */
export function runUnits(textOf: (line: number) => string, sitesOf: (line: number) => readonly RenameSite[], start: number, end: number): RunUnits {
  const lines: number[] = []
  const starts: number[] = []
  const ends: number[] = []
  const codes: number[] = []
  const offsets: number[] = []
  for (let line = start; line < end; line++) {
    offsets.push(lines.length)
    const text = textOf(line)
    const sites = sitesOf(line)
    let next = 0
    for (let column = 0; column < text.length;) {
      while (next < sites.length && sites[next]!.end <= column) {
        next++
      }
      const site = sites[next]
      if (site !== undefined && site.start <= column) {
        lines.push(line)
        starts.push(site.start)
        ends.push(site.end)
        codes.push(-1)
        column = site.end
        continue
      }
      const code = text.charCodeAt(column)
      if (!isBlank(code)) {
        lines.push(line)
        starts.push(column)
        ends.push(column + 1)
        codes.push(code)
      }
      column++
    }
  }
  offsets.push(lines.length)
  return { count: lines.length, lines, starts, ends, codes, firstLine: start, offsets }
}

/** Whether two sides are the same units in the same order: the same characters, a renamed name where the other has one */
export function sameUnits(one: RunUnits, other: RunUnits): boolean {
  return one.count === other.count && one.codes.every((code, at) => code === other.codes[at])
}

/** The first and the last unit that `[start, end)` of `line` holds a part of; null where it holds whitespace alone */
export function unitsIn(units: RunUnits, line: number, start: number, end: number): readonly [number, number] | null {
  const from = units.offsets[line - units.firstLine]
  const to = units.offsets[line - units.firstLine + 1]
  if (from === undefined || to === undefined) {
    return null
  }
  let first = -1
  let last = -1
  for (let at = from; at < to && units.starts[at]! < end; at++) {
    if (units.ends[at]! > start) {
      first = first < 0 ? at : first
      last = at
    }
  }
  return first < 0 ? null : [first, last]
}

/** The first unit at or after column `column` of `line`, on that line or a later one; the count of the units when none follows */
export function unitFrom(units: RunUnits, line: number, column: number): number {
  const from = units.offsets[line - units.firstLine]
  const to = units.offsets[line - units.firstLine + 1]
  if (from === undefined || to === undefined) {
    return units.count
  }
  let at = from
  while (at < to && units.starts[at]! < column) {
    at++
  }
  return at
}
