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
  /** Inside a changed range of the engine */
  changed: boolean
  /** A deleted part of the old line in an Inline row: not selectable, not copied */
  deleted: boolean
  /** Inside a hit of the find: 1, or 2 for the current hit */
  hit?: number
}

/** The hits of the find on a line: flat `[start, end, ...]`, and the index of the current one among them, or -1 */
export interface LineHits {
  ranges: readonly number[]
  current: number
}

/** A cursor over sorted flat ranges: the index of the range that holds an offset, or -1; offsets must not decrease */
function rangeCursor(ranges: readonly number[] | null): (offset: number) => number {
  let index = 0
  return offset => {
    if (ranges === null) {
      return -1
    }
    while (index + 1 < ranges.length && ranges[index + 1]! <= offset) {
      index += 2
    }
    return index + 1 < ranges.length && ranges[index]! <= offset ? index / 2 : -1
  }
}

/** The runs of line `line` of a page: pairs of an offset and a style */
export function runsOf(page: TokenPage | undefined, lineInPage: number): Uint32Array | null {
  if (page === undefined) {
    return null
  }
  return page.runs.subarray(page.lineStarts[lineInPage]! * 2, page.lineStarts[lineInPage + 1]! * 2)
}

/**
 * The parts of a line of `length` UTF-16 units: a part ends at each run start, each bound of a changed range and each
 * bound of a hit
 */
export function lineParts(length: number, runs: Uint32Array | null, ranges: readonly number[] | null, hits: LineHits | null = null): LinePart[] {
  const bounds = [0, length]
  for (let index = 0; runs !== null && index < runs.length; index += 2) {
    bounds.push(runs[index]!)
  }
  bounds.push(...(ranges ?? []), ...(hits?.ranges ?? []))
  bounds.sort((a, b) => a - b)
  const parts: LinePart[] = []
  let run = -2
  const changedAt = rangeCursor(ranges)
  const hitAt = rangeCursor(hits?.ranges ?? null)
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
    const part: LinePart = { start, end, style: run >= 0 ? runs![run + 1]! : 0, changed: changedAt(start) >= 0, deleted: false }
    if (hit >= 0) {
      part.hit = hit === hits!.current ? 2 : 1
    }
    parts.push(part)
  }
  return parts
}

/** The parts of `parts` that cover `[start, end)`, cut at the bounds, with `extra` set */
function sliceParts(parts: readonly LinePart[], start: number, end: number, extra: Partial<LinePart> = {}): LinePart[] {
  const result: LinePart[] = []
  for (const part of parts) {
    if (part.end <= start || part.start >= end) {
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

/**
 * The ranges of the two lines of a similar pair, aligned by index: the engine gives a line without a range no range at
 * all, so a pure insertion has ranges on the new line only. Such a side gets an empty range where the text of the
 * other side is inserted or deleted. Null when both sides have ranges that do not align.
 */
export function alignInlineRanges(oldRanges: readonly number[], newRanges: readonly number[]): [readonly number[], readonly number[]] | null {
  if (oldRanges.length === newRanges.length) {
    return [oldRanges, newRanges]
  }
  const emptyAt = (ranges: readonly number[]) => {
    const empty: number[] = []
    // the text between two ranges is the same on both sides: an offset of the other side less what its ranges hold
    let shift = 0
    for (let index = 0; index + 1 < ranges.length; index += 2) {
      empty.push(ranges[index]! - shift, ranges[index]! - shift)
      shift += ranges[index + 1]! - ranges[index]!
    }
    return empty
  }
  if (oldRanges.length === 0) {
    return [emptyAt(newRanges), newRanges]
  }
  if (newRanges.length === 0) {
    return [oldRanges, emptyAt(oldRanges)]
  }
  return null
}

/**
 * The parts of an Inline row: the new line with the deleted parts of the old line put in before the inserted parts,
 * each part with the runs of its own side. The ranges of the two lines align by index, so the text between two ranges
 * is the same on both sides and comes from the new line. Ranges that do not align give the new line as it is.
 */
export function inlineParts(
  oldLength: number,
  oldRuns: Uint32Array | null,
  oldRanges: readonly number[],
  newLength: number,
  newRuns: Uint32Array | null,
  newRanges: readonly number[],
  newHits: LineHits | null = null,
): InlinePart[] {
  const aligned = alignInlineRanges(oldRanges, newRanges)
  const fromNew = (parts: LinePart[]) => parts.map((part): InlinePart => ({ ...part, side: 1 }))
  if (aligned === null) {
    return fromNew(lineParts(newLength, newRuns, newRanges, newHits))
  }
  const [deletions, insertions] = aligned
  const newParts = lineParts(newLength, newRuns, insertions, newHits)
  const oldParts = lineParts(oldLength, oldRuns, null)
  const result: InlinePart[] = []
  let position = 0
  for (let index = 0; index + 1 < insertions.length; index += 2) {
    const newStart = insertions[index]!
    const newEnd = insertions[index + 1]!
    result.push(...fromNew(sliceParts(newParts, position, newStart)))
    const deleted = sliceParts(oldParts, deletions[index]!, deletions[index + 1]!, { changed: true, deleted: true })
    result.push(...deleted.map((part): InlinePart => ({ ...part, side: 0 })))
    result.push(...fromNew(sliceParts(newParts, newStart, newEnd)))
    position = Math.max(position, newEnd)
  }
  result.push(...fromNew(sliceParts(newParts, position, newLength)))
  return result
}
