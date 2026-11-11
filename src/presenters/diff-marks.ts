import type { LineMove, RunKind } from "@/lib/bindings"

/*
 * The words of the marks of a diff (openspec/specs/commit/quiet-changes/spec.md, "Views"): Rust decides which lines of
 * a block changed in one way alone, which lines moved within the file and where the reading of each block starts
 * (`TextDiff.marks`), this says it: in the note before such lines and beside the counts of a file. What a line of a
 * quiet run or of a move is to a screen reader is generated content of diff-view.css, as what a changed line is.
 */

/** What a note says of a run of lines, by its kind (`LineRun.kind`) */
const RUN_WORDS: Readonly<Record<RunKind, string>> = {
  formatting: "Formatting only",
  comments: "Comments only",
  renamed: "Renamed only",
}

/**
 * What a note of the diff is asked to word: a run of the marks, or an end of a move, as a view shows it (`Remark` of
 * block-parts.ts)
 */
export interface RemarkFacts {
  readonly kind: RunKind | "moved"
  /** The lines of each side it is about: [start, end); of an end of a move the lines of its side */
  readonly lines: readonly (readonly [number, number])[]
  /** Its old lines show; null where the layout hides none of them */
  readonly open: boolean | null
  /**
   * Of a move (`kind: "moved"`), the end the note stands at: where the lines left, or where they stand now, and the
   * lines of its other end, [start, end), the blank lines that moved with the move among them; null for a run
   */
  readonly move: { readonly at: "source" | "destination"; readonly move: LineMove; readonly other: readonly [number, number] } | null
}

export interface RemarkWords {
  /** What the lines changed in: "Formatting only"; of a move what happened to them: "14 lines moved", "Moved" */
  readonly label: string
  /**
   * Of a move, where its other end is, the words of the button that shows it ("to line 210", "from line 120"), and the
   * name of that button ("Go to line 210, where the lines stand now"); null for a run
   */
  readonly target: { readonly words: string; readonly name: string } | null
  /**
   * How far it reaches, by the new lines: "3 lines"; of a move what else happened to its lines on the way
   * ("re-indented · 1 line edited"), empty when nothing did and where the lines left
   */
  readonly detail: string
  /** The note as one sentence, the name of its button: "Formatting only, 3 lines", "14 lines moved to line 210" */
  readonly name: string
  /** What a press of the note does, after the name of its button: "Show the old line"; null for a note that opens nothing */
  readonly toggle: string | null
}

/** "1 line", "3 lines" */
function lineCount(count: number): string {
  return `${count} ${count === 1 ? "line" : "lines"}`
}

/**
 * The words of the note before a run of lines, "Formatting only · 3 lines", or at an end of a move: where the lines
 * left "14 lines moved to line 210", where they stand now "Moved from line 120 · re-indented · 1 line edited"; and
 * what its buttons do. A line is named by its number in its text, from 1: the first line of the other end, the blank
 * lines that moved with the move among them
 */
export function presentRemark(remark: RemarkFacts): RemarkWords {
  const [oldStart, oldEnd] = remark.lines[0]!
  const [newStart, newEnd] = remark.lines[1]!
  const old = oldEnd - oldStart
  const lines = old === 1 ? "the old line" : `the ${old} old lines`
  const toggle = remark.open === null ? null : `${remark.open ? "Hide" : "Show"} ${lines}`
  if (remark.kind !== "moved" || remark.move === null) {
    const label = remark.kind === "moved" ? "Moved" : RUN_WORDS[remark.kind]
    const detail = lineCount(newEnd - newStart)
    return { label, target: null, detail, name: `${label}, ${detail}`, toggle }
  }
  const { at, move, other } = remark.move
  // the first line of the other end, as a reader counts
  const line = other[0] + 1
  if (at === "source") {
    const label = `${lineCount(old)} moved`
    return { label, target: { words: `to line ${line}`, name: `Go to line ${line}, where the lines stand now` }, detail: "", name: `${label} to line ${line}`, toggle }
  }
  const detail = [move.indent === 0 ? null : "re-indented", move.edited.length === 0 ? null : `${lineCount(move.edited.length)} edited`].filter(part => part !== null).join(" · ")
  return {
    label: "Moved",
    target: { words: `from line ${line}`, name: `Go to line ${line}, where the lines were` },
    detail,
    name: [`Moved from line ${line}`, ...(detail === "" ? [] : [detail.replaceAll(" · ", ", ")])].join(", "),
    toggle,
  }
}

/** The name of the action that shows the other end of a move, as the menu of the code names it */
export const MOVE_END_LABEL = "Go to Other End of Move"

/** What that action does from an end of a move, for its hover: where the lines stand now, or where they were */
export function describeMoveEnd(at: "source" | "destination"): string {
  return at === "source" ? "Show where the moved lines stand now" : "Show where the moved lines were"
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
