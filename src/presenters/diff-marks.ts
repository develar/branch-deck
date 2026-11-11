import type { LineMove, Rename, RunKind } from "@/lib/bindings"
import { cutList } from "@/lib/text"

/*
 * The words of the marks of a diff (openspec/specs/commit/quiet-changes/spec.md, "Views"): Rust decides which lines of
 * a block changed in one way alone, which lines moved within the file, which names the change renamed and where the
 * reading of each block starts (`TextDiff.marks`), this says it: in the note before such lines and beside the counts
 * of a file. What a line of a quiet run or of a move is to a screen reader is generated content of diff-view.css, as
 * what a changed line is.
 */

/**
 * What a note says of a run of lines, by its kind (`LineRun.kind`); of a run of the kind `renamed` only where the
 * marks name no rename with a site in it, which no engine gives: its rename is worded else (`RemarkFacts.renames`)
 */
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
  /**
   * Of the note of the renames a view names at a block: the renames, worded as a list of the app is (the first, two of
   * two, the others counted from three on), and the old lines of the quiet runs the note opens, [start, end) each;
   * null or absent for any other note
   */
  readonly renames?: { readonly renames: readonly RenamedName[]; readonly hides: readonly (readonly [number, number])[] } | null
}

/** A rename as a note words it: the two spellings of the name and how many places hold it (`Rename`) */
export type RenamedName = Pick<Rename, "old" | "new" | "sites">

/** The words of one rename of a note: its two spellings, set in the font of the code, and the places that hold it, "12 sites" */
export interface RenamedWords {
  readonly old: string
  readonly new: string
  readonly sites: string
}

/** The renames a note words before it counts the rest; one more is worded too (`cutList`: never "and 1 more") */
const WORDED_RENAMES = 1

export interface RemarkWords {
  /** What the lines changed in: "Formatting only"; of a move what happened to them: "14 lines moved", "Moved"; of renames "Renamed" */
  readonly label: string
  /**
   * Of a note of renames, the renames it words after the label, each with its two spellings and its sites: `total` →
   * `sum` · "12 sites"; the one of a note of one, both of a note of two, the first of a longer one; null for any other
   * note
   */
  readonly renamed: readonly RenamedWords[] | null
  /**
   * Of a move, where its other end is, the words of the button that shows it ("to line 210", "from line 120"), and the
   * name of that button ("Go to line 210, where the lines stand now"); null for a run
   */
  readonly target: { readonly words: string; readonly name: string } | null
  /**
   * How far it reaches, by the new lines: "3 lines"; of a move what else happened to its lines on the way
   * ("re-indented · 1 line edited"), empty when nothing did and where the lines left, and for renames, each of which
   * says its sites (`renamed`)
   */
  readonly detail: string
  /**
   * Of a note that names more renames than it words (three or more): "and 2 more", those renames a line each for its
   * hover ("count → size · 3 sites"), and as words a screen reader hears after the count (": count to size, 3 sites;
   * flag to on, 1 site"); null for a note of one or two renames and for any other note
   */
  readonly more: { readonly words: string; readonly names: string; readonly said: string } | null
  /**
   * The note as one sentence, the name of its button: "Formatting only, 3 lines", "14 lines moved to line 210",
   * "Renamed total to sum, 12 sites, and count to size, 3 sites", "Renamed total to sum, 12 sites, and 2 more: count
   * to size, 3 sites; flag to on, 1 site"
   */
  readonly name: string
  /** What a press of the note does, after the name of its button: "Show the old line"; null for a note that opens nothing */
  readonly toggle: string | null
}

/** "1 line", "3 lines" */
function lineCount(count: number): string {
  return `${count} ${count === 1 ? "line" : "lines"}`
}

/** "1 site", "12 sites": the places that hold a renamed name */
function siteCount(count: number): string {
  return `${count} ${count === 1 ? "site" : "sites"}`
}

/**
 * The words of the note before a run of lines, "Formatting only · 3 lines", at an end of a move: where the lines
 * left "14 lines moved to line 210", where they stand now "Moved from line 120 · re-indented · 1 line edited", or of
 * the renames named at a block: "Renamed total → sum · 12 sites", two of them both ("Renamed total → sum · 12 sites ·
 * count → size · 3 sites"), of three or more the first and the count of the others, "and 2 more" (`cutList`); and
 * what its buttons do. A line is named by its number in its text, from 1: the first line of the
 * other end, the blank lines that moved with the move among them
 */
export function presentRemark(remark: RemarkFacts): RemarkWords {
  const [oldStart, oldEnd] = remark.lines[0]!
  const [newStart, newEnd] = remark.lines[1]!
  const first = remark.renames?.renames[0]
  // the old lines the note opens: of its run or its move, or of the quiet runs of its renames that are its to open
  const old = first === undefined ? oldEnd - oldStart : remark.renames!.hides.reduce((count, [start, end]) => count + end - start, 0)
  const lines = old === 1 ? "the old line" : `the ${old} old lines`
  const toggle = remark.open === null ? null : `${remark.open ? "Hide" : "Show"} ${lines}`
  if (first !== undefined) {
    const { renames } = remark.renames!
    const { shown, more: rest } = cutList(renames, WORDED_RENAMES)
    const others = renames.slice(shown.length)
    const sentence = (it: RenamedName) => `${it.old} to ${it.new}, ${siteCount(it.sites)}`
    const said = others.map(sentence).join("; ")
    const more = rest === 0 ? null : { words: `and ${rest} more`, names: others.map(it => `${it.old} → ${it.new} · ${siteCount(it.sites)}`).join("\n"), said: `: ${said}` }
    const name = [`Renamed ${sentence(first)}`, ...shown.slice(1).map(it => `and ${sentence(it)}`), ...(more === null ? [] : [`${more.words}: ${said}`])].join(", ")
    return {
      label: "Renamed",
      renamed: shown.map(it => ({ old: it.old, new: it.new, sites: siteCount(it.sites) })),
      target: null,
      detail: "",
      more,
      name,
      toggle,
    }
  }
  if (remark.kind !== "moved" || remark.move === null) {
    const label = remark.kind === "moved" ? "Moved" : RUN_WORDS[remark.kind]
    const detail = lineCount(newEnd - newStart)
    return { label, renamed: null, target: null, detail, more: null, name: `${label}, ${detail}`, toggle }
  }
  const { at, move, other } = remark.move
  // the first line of the other end, as a reader counts
  const line = other[0] + 1
  if (at === "source") {
    const label = `${lineCount(old)} moved`
    return {
      label,
      renamed: null,
      target: { words: `to line ${line}`, name: `Go to line ${line}, where the lines stand now` },
      detail: "",
      more: null,
      name: `${label} to line ${line}`,
      toggle,
    }
  }
  const detail = [move.indent === 0 ? null : "re-indented", move.edited.length === 0 ? null : `${lineCount(move.edited.length)} edited`].filter(part => part !== null).join(" · ")
  return {
    label: "Moved",
    renamed: null,
    target: { words: `from line ${line}`, name: `Go to line ${line}, where the lines were` },
    detail,
    more: null,
    name: [`Moved from line ${line}`, ...(detail === "" ? [] : [detail.replaceAll(" · ", ", ")])].join(", "),
    toggle,
  }
}

/**
 * What a renamed name is, for the hover of its mark in a line: on the new side what it was, "Renamed from total", on
 * the old side what it became, "Renamed to sum"
 */
export function describeRenamedName(side: "old" | "new", rename: Pick<Rename, "old" | "new">): string {
  return side === "old" ? `Renamed to ${rename.new}` : `Renamed from ${rename.old}`
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
