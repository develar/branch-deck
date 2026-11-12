import type { LineMove, Rename, RenameProof, RunKind } from "@/lib/bindings"
import { cutList } from "@/lib/text"

/*
 * The words of the marks of a diff (openspec/specs/commit/quiet-changes/spec.md, "Views"): Rust decides which lines of
 * a block changed in one way alone, which lines moved within the file, which names the change renamed and where the
 * reading of each block starts (`TextDiff.marks`), and what the other files of the subject and the index of the
 * repository say of the file (`TextDiff.marks.subject`: the lines that moved to or from another file, the name
 * another file declares, how far a rename reaches in the subject and how far it is proven), this says it: in the note before such lines and beside the
 * counts of a file. What a line of a quiet run or of a move is to a screen reader is generated content of diff-view.css, as
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
 * Another file of the subject, as a note names it: the file the lines of a move went to or came from, the file that
 * declares a renamed name
 */
export interface OtherFile {
  /** Its path in the repository */
  readonly path: string
  /** Its name among the files of the subject (`fileNameAmong`): "Report.kt", "api/Report.kt" */
  readonly name: string
  /** The view can show a place of it: the words that name it are a button that shows the place */
  readonly shown: boolean
}

/**
 * The name of file `index` of `paths` in a few words: its last part, with as many of the directories before it as
 * tell it from every other path of the list that ends the same ("Report.kt"; "api/Report.kt" beside
 * "impl/Report.kt")
 */
export function fileNameAmong(paths: readonly string[], index: number): string {
  const parts = (paths[index] ?? "").split("/")
  const others = paths.filter((_, at) => at !== index)
  for (let count = 1; count < parts.length; count++) {
    const name = parts.slice(-count).join("/")
    if (!others.some(other => other === name || other.endsWith(`/${name}`))) {
      return name
    }
  }
  return parts.join("/")
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
   * lines of its other end, [start, end), the blank lines that moved with the move among them; of a move between two
   * files of the subject the file of that other end (`file`); null for a run
   */
  readonly move: { readonly at: "source" | "destination"; readonly move: LineMove; readonly other: readonly [number, number]; readonly file?: OtherFile | null } | null
  /**
   * Of the note of the renames a view names at a block: the renames, worded as a list of the app is (the first, two of
   * two, the others counted from three on), and the old lines of the quiet runs the note opens, [start, end) each;
   * null or absent for any other note
   */
  readonly renames?: { readonly renames: readonly RenamedName[]; readonly hides: readonly (readonly [number, number])[] } | null
}

/**
 * A rename as a note words it: the two spellings of the name and how many places of the file hold it (`Rename`), and
 * what the subject says of it: what the index of the repository proved of its uses (`proof`; null or absent where it
 * was not asked), of a name another file of the subject declares that file and the line of the declaration there,
 * from 0 (`declared`; null or absent for a name the file declares), and of a name the file declares that is not local
 * the places that hold it in the files of the subject, this one among them, and the files they are in (`across`;
 * null or absent else)
 */
export type RenamedName = Pick<Rename, "old" | "new" | "sites"> & {
  readonly proof?: RenameProof | null
  readonly declared?: { readonly file: OtherFile; readonly line: number } | null
  readonly across?: { readonly sites: number; readonly files: number } | null
}

/**
 * The words of one rename of a note: its two spellings, set in the font of the code, and the places that hold it, "12
 * sites"; of a name the file declares and other files of the subject use, those of all of them: "30 sites in 4 files"
 */
export interface RenamedWords {
  readonly old: string
  readonly new: string
  readonly sites: string
}

/**
 * What the subject says of one rename of a note, after the words of the note: "declared in Store.kt · all uses in
 * this change"
 */
export interface ReachWords {
  /** The rename, by its number among the renames of the note */
  readonly rename: number
  /**
   * The new name of the rename, set in the font of the code before the words, where the note names more than one
   * rename, so each is told from the others; null in a note of one
   */
  readonly of: string | null
  /**
   * Of a name another file declares: "declared in Store.kt", whether the view can show the declaration (`jump`: the
   * words are a button, else plain), and what the words say in full, the name of that button ("Go to the declaration
   * of connectStore in src/Store.kt") or the hover of the plain words ("connectStore is declared in src/Store.kt");
   * null for a name the file declares
   */
  readonly declared: { readonly words: string; readonly name: string; readonly jump: boolean } | null
  /** What the index proved of its uses (`RenameProof`), for a test and a style; null where it was not asked */
  readonly proof: RenameProof | null
  /** What the index said, in words (`PROOF_WORDS`); empty where it said nothing to tell */
  readonly said: string
}

/**
 * What the index of the repository said of the uses of a renamed name, after the words of its note; empty where there
 * is nothing to tell: it is still to be asked, or no index answers. Only `confirmed` is a proof: a reader passes the
 * uses of such a name; under any other word they are read
 */
const PROOF_WORDS: Readonly<Record<RenameProof, string>> = {
  asked: "",
  confirmed: "all uses in this change",
  usesOutside: "uses outside this change",
  otherMeaning: "changes what another name means",
  otherDeclaration: "more than the name changed",
  incomplete: "uses not all checked",
  unavailable: "",
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
   * Of a move, where its other end is, the words of the button that shows it ("to line 210", "from line 120"; of a
   * move between two files "to Report.kt:12", "from Ledger.kt:40"), whether the view can show that end (`jump`: false
   * for an end in a file the view does not show, whose words are plain), and what the words say in full: the name of
   * the button ("Go to line 210, where the lines stand now") or the hover of the plain words ("Line 12 of
   * src/Report.kt, where the lines stand now"); null for a run
   */
  readonly target: { readonly words: string; readonly name: string; readonly jump: boolean } | null
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
   * Of a note of renames, what the subject says of each rename it words that it says something of, after the words of
   * the note, in their order: where another file declares it, what the index proved of its uses. Empty for a note
   * whose renames the subject says nothing of, and for any other note
   */
  readonly reach: readonly ReachWords[]
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
 * The places that hold a renamed name, as its note counts them: those of the file, "12 sites"; where the file declares
 * the name and other files of the subject hold it too, those of the subject, which a proof beside them speaks of:
 * "30 sites in 4 files"
 */
function sitesOf(rename: RenamedName): string {
  const across = rename.across
  return across != null && across.files > 1 ? `${siteCount(across.sites)} in ${across.files} files` : siteCount(rename.sites)
}

/** What the subject says of a rename, in the words of a note; null where it says nothing to tell */
function reachOf(rename: RenamedName, at: number, several: boolean): ReachWords | null {
  const file = rename.declared?.file
  const said = rename.proof == null ? "" : PROOF_WORDS[rename.proof]
  if (file === undefined && said === "") {
    return null
  }
  return {
    rename: at,
    of: several ? rename.new : null,
    declared:
      file === undefined
        ? null
        : {
            words: `declared in ${file.name}`,
            name: file.shown ? `Go to the declaration of ${rename.new} in ${file.path}` : `${rename.new} is declared in ${file.path}`,
            jump: file.shown,
          },
    proof: rename.proof ?? null,
    said,
  }
}

/** What a reach says as a part of a sentence: "declared in Store.kt, all uses in this change" */
function reachSentence(reach: ReachWords): string {
  return [reach.declared?.words, reach.said].filter(part => part !== undefined && part !== "").join(", ")
}

/**
 * The words of the note before a run of lines, "Formatting only · 3 lines", at an end of a move: where the lines
 * left "14 lines moved to line 210", where they stand now "Moved from line 120 · re-indented · 1 line edited", of a
 * move between two files "Moved to Report.kt:12" and "Moved from Ledger.kt:40", or of the renames named at a block:
 * "Renamed total → sum · 12 sites", two of them both ("Renamed total → sum · 12 sites · count → size · 3 sites"), of
 * three or more the first and the count of the others, "and 2 more" (`cutList`), then what the subject says of each
 * ("declared in Store.kt · all uses in this change"); a name the file declares and other files of the subject use
 * counts the places of all of them ("Renamed openStore → connectStore · 30 sites in 4 files"); and what its buttons do. A line is named by its number in its
 * text, from 1: the first line of the other end, the blank lines that moved with the move among them
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
    // each rename with what the subject says of it: "total to sum, 12 sites, declared in Store.kt, all uses in this change"
    const told = renames.map((it, at) => ({ rename: it, reach: reachOf(it, at, renames.length > 1) }))
    const sentence = ({ rename: it, reach }: (typeof told)[number]) =>
      [`${it.old} to ${it.new}`, sitesOf(it), ...(reach === null ? [] : [reachSentence(reach)])].filter(part => part !== "").join(", ")
    const line = ({ rename: it, reach }: (typeof told)[number]) =>
      [`${it.old} → ${it.new}`, sitesOf(it), ...(reach === null ? [] : [reach.declared?.words ?? "", reach.said])].filter(part => part !== "").join(" · ")
    const counted = told.slice(shown.length)
    const said = counted.map(sentence).join("; ")
    const more = rest === 0 ? null : { words: `and ${rest} more`, names: counted.map(line).join("\n"), said: `: ${said}` }
    const name = [`Renamed ${sentence(told[0]!)}`, ...told.slice(1, shown.length).map(it => `and ${sentence(it)}`), ...(more === null ? [] : [`${more.words}: ${said}`])].join(", ")
    return {
      label: "Renamed",
      renamed: shown.map(it => ({ old: it.old, new: it.new, sites: sitesOf(it) })),
      target: null,
      detail: "",
      more,
      reach: told.slice(0, shown.length).flatMap(it => it.reach ?? []),
      name,
      toggle,
    }
  }
  if (remark.kind !== "moved" || remark.move === null) {
    const label = remark.kind === "moved" ? "Moved" : RUN_WORDS[remark.kind]
    const detail = lineCount(newEnd - newStart)
    return { label, renamed: null, target: null, detail, more: null, reach: [], name: `${label}, ${detail}`, toggle }
  }
  const { at, move, other, file } = remark.move
  // the first line of the other end, as a reader counts
  const line = other[0] + 1
  if (file != null) {
    // a move between two files: every line of it shows at both ends, so the note counts none; the place is the file and the line
    const words = `${at === "source" ? "to" : "from"} ${file.name}:${line}`
    const where = `ine ${line} of ${file.path}, where the lines ${at === "source" ? "stand now" : "were"}`
    return {
      label: "Moved",
      renamed: null,
      target: { words, name: file.shown ? `Go to l${where}` : `L${where}`, jump: file.shown },
      detail: "",
      more: null,
      reach: [],
      name: `Moved ${at === "source" ? "to" : "from"} line ${line} of ${file.path}`,
      toggle,
    }
  }
  if (at === "source") {
    const label = `${lineCount(old)} moved`
    return {
      label,
      renamed: null,
      target: { words: `to line ${line}`, name: `Go to line ${line}, where the lines stand now`, jump: true },
      detail: "",
      more: null,
      reach: [],
      name: `${label} to line ${line}`,
      toggle,
    }
  }
  const detail = [move.indent === 0 ? null : "re-indented", move.edited.length === 0 ? null : `${lineCount(move.edited.length)} edited`].filter(part => part !== null).join(" · ")
  return {
    label: "Moved",
    renamed: null,
    target: { words: `from line ${line}`, name: `Go to line ${line}, where the lines were`, jump: true },
    detail,
    more: null,
    reach: [],
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

/**
 * What that action does from an end of a move, for its hover: where the lines stand now, or where they were; of a
 * move between two files with the name of the other one (`file`: "Show where the moved lines stand now, in Report.kt")
 */
export function describeMoveEnd(at: "source" | "destination", file: string | null = null): string {
  const where = at === "source" ? "Show where the moved lines stand now" : "Show where the moved lines were"
  return file === null ? where : `${where}, in ${file}`
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
