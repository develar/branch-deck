import type { TokenPage } from "@/lib/diff/highlight/token-cache"

/*
 * The parts of a painted line (docs/design/diff-view.md, "Painting the layers of a line"): the syntax runs, the changed
 * ranges of the engine, the hits of the find and, in Inline, the deleted parts of the old line, merged into one list
 * per line.
 */

export interface LinePart {
  start: number
  end: number
  /** The style of the syntax runs: the role in the low 4 bits, the font style bits above; 0 the default foreground */
  style: number
  /**
   * The `RangeKind` of the changed range of the engine that holds the part (0 inserted, 1 deleted, 2 modified), or
   * `RENAMED_RANGE` for a part of a renamed name; -1 outside one
   */
  change: number
  /** A deleted part of the old line in an Inline row: not selectable, not copied */
  deleted: boolean
  /** Inside a hit of the find: 1, or 2 for the current hit */
  hit?: number
  /** The marker of the empty side of an inserted or a deleted change: a part without text at `start` */
  marker?: boolean
}

/** The values of one changed range of the engine: `[start, end, kind]` (`DiffBlock.oldRanges`) */
export const RANGE_STRIDE = 3

/**
 * The kind of the range of a renamed name (a site of `TextDiff.marks.renames`, rename-sites.ts), beside the
 * `RangeKind` of the engine: the page's own. The name is drawn in a mark of its own, never as a change; the row of a
 * pair of lines (Inline) strikes the old name through before the new one, as any deleted part, both in that mark
 */
export const RENAMED_RANGE = 3

/** The hits of the find on a line: flat `[start, end, ...]`, and the index of the current one among them, or -1 */
export interface LineHits {
  ranges: readonly number[]
  current: number
  /**
   * The hits of hidden old lines that the line stands for (`DiffFindHit.stand`: a line of a quiet run of a renamed name
   * that Inline shows once), by its own columns: `[start, end, hit, ...]` in their order, without overlap, `hit` as
   * `LinePart.hit` (2 for the current hit). Absent where it stands for none
   */
  stands?: readonly number[]
}

/**
 * A cursor over sorted flat ranges of `stride` values each: the index of the range that holds an offset, or -1; offsets
 * must not decrease. A range without text holds no offset
 */
function rangeCursor(ranges: readonly number[] | null, stride: number): (offset: number) => number {
  let index = 0
  return offset => {
    if (ranges === null) {
      return -1
    }
    while (index + 1 < ranges.length && ranges[index + 1]! <= offset) {
      index += stride
    }
    return index + 1 < ranges.length && ranges[index]! <= offset ? index / stride : -1
  }
}

/** The runs of line `line` of a page: pairs of an offset and a style */
export function runsOf(page: TokenPage | undefined, lineInPage: number): Uint32Array | null {
  if (page === undefined) {
    return null
  }
  return page.runs.subarray(page.lineStarts[lineInPage]! * 2, page.lineStarts[lineInPage + 1]! * 2)
}

/** The values of one hit of the old line in the text the new line shows: `[start, end, hit]`, `hit` as `LinePart.hit` */
const SHARED_HIT_STRIDE = 3

/**
 * The parts of a line of `length` UTF-16 units: a part ends at each run start, each bound of a changed range (`[start,
 * end, kind, ...]`) and each bound of a hit. The empty side of a change is a marker part where it stands. `sharedHits`:
 * the hits of the old line of an Inline row in the text it shares with this line (`sharedHitsOf`)
 */
export function lineParts(
  length: number,
  runs: Uint32Array | null,
  ranges: readonly number[] | null,
  hits: LineHits | null = null,
  sharedHits: readonly number[] | null = null,
): LinePart[] {
  const bounds = [0, length]
  for (let index = 0; runs !== null && index < runs.length; index += 2) {
    bounds.push(runs[index]!)
  }
  for (let index = 0; ranges !== null && index + 1 < ranges.length; index += RANGE_STRIDE) {
    bounds.push(ranges[index]!, ranges[index + 1]!)
  }
  // a loop, not a spread: a line may hold more hits than a call takes arguments
  for (const bound of hits?.ranges ?? []) {
    bounds.push(bound)
  }
  for (let index = 0; sharedHits !== null && index + 1 < sharedHits.length; index += SHARED_HIT_STRIDE) {
    bounds.push(sharedHits[index]!, sharedHits[index + 1]!)
  }
  bounds.sort((a, b) => a - b)
  const parts: LinePart[] = []
  let run = -2
  const changedAt = rangeCursor(ranges, RANGE_STRIDE)
  const hitAt = rangeCursor(hits?.ranges ?? null, 2)
  const sharedHitAt = rangeCursor(sharedHits, SHARED_HIT_STRIDE)
  for (let index = 0; index + 1 < bounds.length; index++) {
    const start = bounds[index]!
    const end = Math.min(bounds[index + 1]!, length)
    if (start >= end) {
      continue
    }
    while (runs !== null && run + 2 < runs.length && runs[run + 2]! <= start) {
      run += 2
    }
    const hit = hitAt(start)
    const sharedHit = sharedHitAt(start)
    const range = changedAt(start)
    const part: LinePart = { start, end, style: run >= 0 ? runs![run + 1]! : 0, change: range >= 0 ? ranges![range * RANGE_STRIDE + 2]! : -1, deleted: false }
    // the current hit wins where a hit of each line holds the part
    const marked = Math.max(hit < 0 ? 0 : hit === hits!.current ? 2 : 1, sharedHit < 0 ? 0 : sharedHits![sharedHit * SHARED_HIT_STRIDE + 2]!)
    if (marked > 0) {
      part.hit = marked
    }
    parts.push(part)
  }
  return withMarkers(parts, ranges)
}

/** The parts with a marker part at each range without text, before the part that starts there */
function withMarkers(parts: LinePart[], ranges: readonly number[] | null): LinePart[] {
  if (ranges === null) {
    return parts
  }
  let result = parts
  for (let index = 0; index + 2 < ranges.length; index += RANGE_STRIDE) {
    const offset = ranges[index]!
    if (offset !== ranges[index + 1]!) {
      continue
    }
    if (result === parts) {
      result = [...parts]
    }
    const at = result.findIndex(part => part.start >= offset)
    const style = result.find(part => part.start <= offset && offset < part.end)?.style ?? 0
    result.splice(at < 0 ? result.length : at, 0, { start: offset, end: offset, style, change: ranges[index + 2]!, deleted: false, marker: true })
  }
  return result
}

/** The parts of `parts` that cover `[start, end)`, cut at the bounds, with `extra` set; none for a range without text */
function sliceParts(parts: readonly LinePart[], start: number, end: number, extra: Partial<LinePart> = {}): LinePart[] {
  const result: LinePart[] = []
  for (const part of parts) {
    if (start >= end || part.end <= start || part.start >= end) {
      continue
    }
    result.push({ ...part, start: Math.max(part.start, start), end: Math.min(part.end, end), ...extra })
  }
  return result
}

/** A part of the Inline row, with the side its text comes from */
export interface InlinePart extends LinePart {
  side: 0 | 1
}

/** `RangeKind::Modified` of the engine */
const MODIFIED_RANGE = 2

/**
 * The two lines of a similar pair as the row of Inline shows them: the new line, with each deleted part of the old
 * line before the inserted part of its range. The engine pairs two lines only when their ranges align
 * (`DiffBlock.inlinePairs`, `pairs_align` of text-diff): line i of both sides has as many ranges, in the same order,
 * and between them the same text but for whitespace, which a whitespace policy ignores. The row shows that shared
 * text once, as the new line has it.
 */
export interface InlinePair {
  oldText: string
  newText: string
  /** The ranges of the old line, `[start, end, kind, ...]`: the parts the row strikes through */
  deletions: readonly number[]
  /** The ranges of the new line, as many as `deletions` and in their order */
  insertions: readonly number[]
}

/**
 * The pair of two lines with the ranges of the engine. Lines with as many ranges are a pair of the engine. Any other
 * two (no engine gives them) are the whole old line deleted before the whole new line, so a row never hides text of
 * its lines.
 */
export function inlinePair(oldText: string, newText: string, oldRanges: readonly number[], newRanges: readonly number[]): InlinePair {
  if (oldRanges.length === newRanges.length) {
    return { oldText, newText, deletions: oldRanges, insertions: newRanges }
  }
  return { oldText, newText, deletions: [0, oldText.length, MODIFIED_RANGE], insertions: [0, newText.length, MODIFIED_RANGE] }
}

/**
 * Whether the hit `[start, end)` of the old line of an Inline row is a hit of its own: it holds a deleted part, which
 * the row strikes through, or a place where the new line inserts text. Any other hit lies in text the two lines share,
 * where the new line has the same hit.
 */
export function isOwnOldHit({ deletions }: InlinePair, start: number, end: number): boolean {
  for (let index = 0; index + 2 < deletions.length; index += RANGE_STRIDE) {
    const from = deletions[index]!
    const to = deletions[index + 1]!
    if (from === to ? start < from && from < end : from < end && to > start) {
      return true
    }
  }
  return false
}

/**
 * The hits of the old line of a pair that are its own (`isOwnOldHit`), the current one kept: what the row of an edited
 * line of a move marks of its old line, whose every hit is found where the line left
 */
export function ownOldHits(pair: InlinePair, hits: LineHits): LineHits {
  const ranges: number[] = []
  let current = -1
  for (let at = 0; at + 1 < hits.ranges.length; at += 2) {
    if (isOwnOldHit(pair, hits.ranges[at]!, hits.ranges[at + 1]!)) {
      if (hits.current === at / 2) {
        current = ranges.length / 2
      }
      ranges.push(hits.ranges[at]!, hits.ranges[at + 1]!)
    }
  }
  return { ranges, current }
}

/** A space or a tab: the whitespace inside a line that a whitespace policy ignores (`is_space_enter_or_tab` of text-diff) */
function isSpace(code: number): boolean {
  return code === 32 || code === 9
}

/**
 * The offset in `to[toStart, toEnd)` of the offset `offset` of `from[fromStart, fromEnd)`, two texts that are the same
 * but for whitespace: the shared text of a pair before, between or after its ranges. Under a policy that ignores
 * whitespace the two differ in length (another indent), so an offset maps by the characters that are no whitespace
 * before it. `end`: the offset ends a range (the character before it counts) and does not start one; a run of
 * whitespace maps to the whole run of the other text.
 */
function sharedOffset(from: string, fromStart: number, fromEnd: number, to: string, toStart: number, toEnd: number, offset: number, end: boolean): number {
  if (fromEnd - fromStart === toEnd - toStart && from.startsWith(to.slice(toStart, toEnd), fromStart)) {
    return toStart + offset - fromStart
  }
  // the ends of the text are the ends of the other text, whatever whitespace stands there
  if (offset <= fromStart && end) {
    return toStart
  }
  if (offset >= fromEnd && !end) {
    return toEnd
  }
  let count = 0
  for (let index = fromStart; index < offset; index++) {
    if (!isSpace(from.charCodeAt(index))) {
      count++
    }
  }
  const edge = end ? offset - 1 : offset
  const onText = edge >= fromStart && edge < fromEnd && !isSpace(from.charCodeAt(edge))
  let index = toStart
  for (let seen = 0; index < toEnd && seen < count; index++) {
    if (!isSpace(to.charCodeAt(index))) {
      seen++
    }
  }
  // a start on text stands at that text, past the whitespace before it; an end after whitespace takes all of it
  if (onText !== end) {
    while (index < toEnd && isSpace(to.charCodeAt(index))) {
      index++
    }
  }
  return index
}

/**
 * The offset in `to` of the offset `offset` of `from`, a line that is the same text but for its whitespace: the old
 * and the new line of a row the whitespace policy ignores, which one column shows once, by the new line. `end`: the
 * offset ends a range. An unchanged line is the same text, so its offsets are the same
 */
export function sameTextOffset(from: string, to: string, offset: number, end = false): number {
  return sharedOffset(from, 0, from.length, to, 0, to.length, offset, end)
}

/**
 * Calls `visit` with the text the lines of a pair share: before each range and after the last one, as `[oldFrom,
 * oldTo)` of the old line and `[newFrom, newTo)` of the new one
 */
function forEachShared({ oldText, newText, deletions, insertions }: InlinePair, visit: (oldFrom: number, oldTo: number, newFrom: number, newTo: number) => void): void {
  let oldFrom = 0
  let newFrom = 0
  for (let index = 0; index + 2 < deletions.length; index += RANGE_STRIDE) {
    visit(oldFrom, deletions[index]!, newFrom, insertions[index]!)
    oldFrom = deletions[index + 1]!
    newFrom = insertions[index + 1]!
  }
  visit(oldFrom, oldText.length, newFrom, newText.length)
}

/**
 * The offset in the text of an Inline row (`inlineParts`) of the offset `offset` of its old (`side` 0) or new line.
 * `end`: the offset ends a range of the line, so it stands after the character before it: at the end of a deleted
 * part, not at the inserted part or the shared text that follows it.
 */
export function inlineOffset(pair: InlinePair, side: number, offset: number, end = false): number {
  const { oldText, newText, deletions, insertions } = pair
  if (side !== 0) {
    // the deleted parts before the offset: each stands before the inserted part of its range
    let struck = 0
    for (let index = 0; index + 2 < insertions.length; index += RANGE_STRIDE) {
      if (end ? offset <= insertions[index]! : offset < insertions[index]!) {
        break
      }
      struck += deletions[index + 1]! - deletions[index]!
    }
    return offset + struck
  }
  let struck = 0
  let oldFrom = 0
  let newFrom = 0
  let index = 0
  for (; index + 2 < deletions.length; index += RANGE_STRIDE) {
    const from = deletions[index]!
    const to = deletions[index + 1]!
    if (end ? offset <= from : offset < from) {
      break
    }
    if (end ? offset <= to : offset < to) {
      // in the deleted part, which stands where the inserted part of its range starts
      return insertions[index]! + struck + offset - from
    }
    struck += to - from
    oldFrom = to
    newFrom = insertions[index + 1]!
  }
  // in the shared text before the range `index`, or after the last one: where the new line has it
  const more = index + 2 < deletions.length
  return sharedOffset(oldText, oldFrom, more ? deletions[index]! : oldText.length, newText, newFrom, more ? insertions[index]! : newText.length, offset, end) + struck
}

/**
 * The hits of the old line of an Inline row in the text it shares with the new line, by the offsets of the new line:
 * `[start, end, hit, ...]`. The row shows that text once, from the new line, so a hit that reaches out of a deleted
 * part goes on there
 */
function sharedHitsOf(hits: LineHits | null, pair: InlinePair): number[] | null {
  if (hits === null) {
    return null
  }
  const { oldText, newText } = pair
  const shared: number[] = []
  for (let hit = 0; hit + 1 < hits.ranges.length; hit += 2) {
    const start = hits.ranges[hit]!
    const end = hits.ranges[hit + 1]!
    forEachShared(pair, (oldFrom, oldTo, newFrom, newTo) => {
      const cutStart = Math.max(start, oldFrom)
      const cutEnd = Math.min(end, oldTo)
      if (cutStart >= cutEnd) {
        return
      }
      const from = sharedOffset(oldText, oldFrom, oldTo, newText, newFrom, newTo, cutStart, false)
      const to = sharedOffset(oldText, oldFrom, oldTo, newText, newFrom, newTo, cutEnd, true)
      if (from < to) {
        shared.push(from, to, hit / 2 === hits.current ? 2 : 1)
      }
    })
  }
  return shared
}

/**
 * The parts of an Inline row: the new line with the deleted parts of the old line put in before the inserted parts,
 * each part with the runs of its own side. The text between two ranges is the text the lines share and comes from the
 * new line. The hits of the old line (`oldHits`, the ones of its own: `isOwnOldHit`) mark its deleted parts and the
 * shared text they reach into.
 */
export function inlineParts(
  pair: InlinePair,
  oldRuns: Uint32Array | null,
  newRuns: Uint32Array | null,
  newHits: LineHits | null = null,
  oldHits: LineHits | null = null,
): InlinePart[] {
  const { oldText, newText, deletions, insertions } = pair
  // the deleted text stands in the row, so an empty side needs no marker
  const newParts = lineParts(newText.length, newRuns, insertions, newHits, sharedHitsOf(oldHits, pair)).filter(part => part.marker !== true)
  const oldParts = lineParts(oldText.length, oldRuns, null, oldHits)
  const result: InlinePart[] = []
  // a loop, not a spread: a line may have more parts than a call takes arguments
  const add = (parts: readonly LinePart[], side: 0 | 1) => {
    for (const part of parts) {
      result.push({ ...part, side })
    }
  }
  let position = 0
  for (let index = 0; index + 2 < insertions.length; index += RANGE_STRIDE) {
    const newStart = insertions[index]!
    const newEnd = insertions[index + 1]!
    add(sliceParts(newParts, position, newStart), 1)
    add(sliceParts(oldParts, deletions[index]!, deletions[index + 1]!, { change: deletions[index + 2]!, deleted: true }), 0)
    add(sliceParts(newParts, newStart, newEnd), 1)
    position = Math.max(position, newEnd)
  }
  add(sliceParts(newParts, position, newText.length), 1)
  return result
}
