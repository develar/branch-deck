import type { RunKind } from "@/lib/bindings"

/*
 * The words of the marks of a diff (openspec/specs/commit/quiet-changes/spec.md, "Views"): Rust decides which lines of
 * a block changed in one way alone and where the reading of each block starts (`TextDiff.marks`), this says it: in the
 * note before such lines and beside the counts of a file. What a line of a quiet run is to a screen reader is generated
 * content of diff-view.css, as what a changed line is.
 */

/** What a note says of a run of lines, by its kind (`LineRun.kind`) */
const RUN_WORDS: Readonly<Record<RunKind, string>> = {
  formatting: "Formatting only",
  comments: "Comments only",
  renamed: "Renamed only",
}

/** What a note of the diff is asked to word: a run of the marks as a view shows it (`Remark` of block-parts.ts) */
export interface RemarkFacts {
  readonly kind: RunKind
  /** The lines of each side it is about: [start, end) */
  readonly lines: readonly (readonly [number, number])[]
  /** Its old lines show; null where the layout hides none of them */
  readonly open: boolean | null
}

export interface RemarkWords {
  /** What the lines changed in: "Formatting only" */
  readonly label: string
  /** How far it reaches, by the new lines: "3 lines" */
  readonly detail: string
  /** What a press of the note does, the name of its button: "Show the old line"; null for a note that opens nothing */
  readonly toggle: string | null
}

/** "1 line", "3 lines" */
function lineCount(count: number): string {
  return `${count} ${count === 1 ? "line" : "lines"}`
}

/** The words of the note before a run of lines: "Formatting only · 3 lines", and what its button does */
export function presentRemark(remark: RemarkFacts): RemarkWords {
  const [oldStart, oldEnd] = remark.lines[0]!
  const [newStart, newEnd] = remark.lines[1]!
  const old = oldEnd - oldStart
  const lines = old === 1 ? "the old line" : `the ${old} old lines`
  return {
    label: RUN_WORDS[remark.kind],
    detail: lineCount(newEnd - newStart),
    toggle: remark.open === null ? null : `${remark.open ? "Hide" : "Show"} ${lines}`,
  }
}

/** The hits of the find in the old lines a note stands for, `lines` of them: "1 match in the old line", "3 matches in the old lines" */
export function formatHiddenMatches(count: number, lines: number): string {
  return `${count} ${count === 1 ? "match" : "matches"} in the old ${lines === 1 ? "line" : "lines"}`
}

/** What is left to read of a file, beside its counts: "2 to read", "Nothing to read" */
export function formatToRead(toRead: number): string {
  return toRead === 0 ? "Nothing to read" : `${toRead} to read`
}

/**
 * The count to read in full, for a hover and a screen reader: "2 of 3 changes to read", "None of the 3 changes needs
 * reading"; of one change "The change needs no reading"
 */
export function describeToRead(toRead: number, changes: number): string {
  if (toRead > 0) {
    return `${toRead} of ${changes} changes to read`
  }
  return changes === 1 ? "The change needs no reading" : `None of the ${changes} changes needs reading`
}
