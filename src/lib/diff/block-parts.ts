import type { DiffBlock, FileMove, LineMove, LineRun, Rename, RenameProof, RunKind, TextDiff } from "@/lib/diff/file-diff-model"
import { namedRenames, renameSites, sitesOfLine } from "@/lib/diff/rename-sites"
import type { RenameSite } from "@/lib/diff/rename-sites"
import type { Heading, LineRow, LineRowKind, LineTint, NoteRow } from "@/lib/diff/rows"
import type { DiffViewMode } from "@/stores/settings/diff-view-settings"

/*
 * The parts of a block of a diff by its marks (docs/design/diff-view.md, "The parts of a block"): Rust says which rows
 * of lines of a block changed in one way alone (`TextDiff.marks.runs`), which lines moved within the file
 * (`TextDiff.marks.moves`: the old lines of a move in the block they left, its new lines in the block they stand in
 * now) and which names the change renamed (`TextDiff.marks.renames`), and what the other files of the subject say of
 * the file (`TextDiff.marks.subject`: the lines that moved to or from another file, the names another file declares,
 * what the index of the repository proved of a name), and a block shows part by part, in the order of both sides. A part is a row of words (the heading of the block in a section of a review, what the diff says of
 * the lines after it) or rows of lines in one shape. Every function of rows.ts that maps the rows of a block to its
 * lines goes through here, so a view never counts the rows of a block by its line counts.
 */

/** The sides of a row of a diff, as `OLD` and `NEW` of rows.ts */
const OLD = 0
const NEW = 1

/**
 * An end of a move in the block that holds it: `source` where its lines left, the old lines of the move;
 * `destination` where they stand now, its new lines, read there. Of a move within the file (`TextDiff.marks.moves`)
 * the lines that left are nothing to read. Of a move between two files of the subject (`SubjectMarks.movedOut`,
 * `movedIn`: `file`) the other end is in that file: both ends are read, and no layout leaves a line of either out
 */
export interface MoveEnd {
  readonly at: "source" | "destination"
  /**
   * The move. Of a move between files, as a move within a file is said: its old lines are lines of the file they
   * left and its new lines of the file they stand in, exact lines (none edited, no range, no blank line beside them)
   */
  readonly move: LineMove
  /**
   * The lines of the end on its side, the blank lines that moved with them (`blanksBefore`, `blanksAfter`) among
   * them: [start, end), inside its block
   */
  readonly lines: readonly [number, number]
  /**
   * The lines of the other end of the move on its side, as that end has them (`lines` there): what the note of this
   * end names ("to line 210") and where a jump from it lands. Read here alone from the blank lines of the move, as
   * Rust reads them once (`LineMove::old_lines`, `new_lines`)
   */
  readonly other: readonly [number, number]
  /**
   * The index of the block that holds the other end: a view that does not draw the move there draws it nowhere
   * (`drawnMarks`); -1 for an end in another file, whose blocks this diff does not know
   */
  readonly otherBlock: number
  /**
   * Of a move between two files of the subject, the file of the other end by its index among the files of the subject
   * (`FileDiff.subjectPaths`): `other` are lines of that file. Null for a move within the file
   */
  readonly file: number | null
  /**
   * Of a move between files that is a declaration the index says moved whole (`FileMove.declaration`), its name: the
   * lines of each end are the lines of the declaration there, their layout maybe another. Absent for a row of the
   * same lines
   */
  readonly declaration?: string
}

/** The marks of one block of a diff: its runs, and the ends of the moves it holds */
export interface BlockMarks {
  /** In the order of both sides, without overlap, each with a line on both sides */
  readonly runs: readonly LineRun[]
  /** The moves whose old lines the block holds, by those lines */
  readonly sources: readonly MoveEnd[]
  /** The moves whose new lines the block holds, by those lines */
  readonly destinations: readonly MoveEnd[]
  /**
   * The sites of the renames whose name stands on two lines the block does not pair, in a block whose lines the engine
   * pairs (`inlinePairs`: line i of both sides): `[old line, new line]` each. The lines that hold one show apart, the
   * old ones before the new ones, never as the row of a pair, which would draw the name as a deletion in one row and
   * an insertion in another
   */
  readonly apart: readonly (readonly [number, number])[]
  /**
   * The quiet runs of the diff that every layout keeps every line of (`SubjectMarks.confirmed`): runs of a renamed
   * name that the index of the repository proved after the rows of the file were shown. A reader passes them and they
   * are played down, but Inline shows them as it shows a label, never once: an answer that comes late takes no row
   */
  readonly kept: ReadonlySet<LineRun>
}

const NO_RUNS: ReadonlySet<LineRun> = new Set()

/** A block the marks say nothing of */
export const NO_MARKS: BlockMarks = { runs: [], sources: [], destinations: [], apart: [], kept: NO_RUNS }

/**
 * What the other files of the subject and the index of the repository say of a rename a note names
 * (`TextDiff.marks.subject`)
 */
export interface RenameReach {
  /**
   * What the index said of the uses of the name (`RenameProof`); null where it is not asked: a name bound inside a
   * body, a diff of no subject
   */
  readonly proof: RenameProof | null
  /**
   * Of a name another file of the subject declares (`UsedRename`): that file, by its index among the files of the
   * subject (`FileDiff.subjectPaths`), and the new line of the declaration there; of the owner of a declaration that
   * moved whole under another owner (`moved`, its name), the file and the line where that declaration stands now.
   * Null for a name this file declares
   */
  readonly declared: { readonly file: number; readonly line: number; readonly moved?: string | null } | null
  /**
   * Of a name this file declares that is not local (`RenameCheck`): the places that hold it in the files of the
   * subject, this one among them, and the files they are in. What the proof speaks of; null for a name another file
   * declares, a local, a diff of no subject
   */
  readonly across: { readonly sites: number; readonly files: number } | null
}

/**
 * The note of the renames a view names at a block (`renameNotesOf`): one note for a rename in the rows of a file, at
 * its declaration, or at its first site where the rows do not show the declaration; the renames whose notes would
 * stand in one block share one
 */
export interface RenameNote {
  /**
   * The key of the note among the open notes (`PartsOptions.open`): the old line of the site it stands at, which is a
   * line of no other note
   */
  readonly key: number
  /** The renames it names, in the order of their sites in the block: the note words them, a list too long cut (`presentRemark`) */
  readonly renames: readonly Rename[]
  /**
   * What the subject says of each of `renames`, in their order: where another file declares it, how far it reaches in
   * the files of the subject, what the index proved of it
   */
  readonly reach: readonly RenameReach[]
  /** The site it stands at: its line on each side */
  readonly lines: readonly [number, number]
  /**
   * The new line of the site each of `renames` is named at, in their order, the first the line of the note: the place
   * a jump from the note to the declaration of that rename left (`renameJumpOf`)
   */
  readonly sites: readonly number[]
  /**
   * The old lines of the quiet runs it opens (`RenameNotes.ofRun`), [start, end) each, in their order: what Inline
   * shows once, by the new lines, until the note is opened. Empty where it opens none: every line of its renames is
   * read or kept by every layout (`BlockMarks.kept`), or the note of another rename opens their runs
   */
  readonly hides: readonly (readonly [number, number])[]
}

/**
 * The jump from the note of renames to the declaration of rename `at` of it, a name another file of the subject
 * declares ("declared in Store.kt"): the file that declares it, by its index among the files of the subject, and the
 * new line of the declaration there; the new name, which the jump selects in that line; and the new line of the site
 * the note names the rename at, the place Back returns to: the site in the rows of the note, never an earlier one that
 * a section of a review does not show or plays down. Null for a rename the note does not hold, and for a name the
 * file declares itself
 */
export function renameJumpOf(note: RenameNote, at: number): { readonly file: number; readonly line: number; readonly name: string; readonly from: number } | null {
  const rename = note.renames[at]
  const declared = note.reach[at]?.declared
  if (rename === undefined || declared == null) {
    return null
  }
  return { file: declared.file, line: declared.line, name: rename.new, from: note.sites[at] ?? note.lines[NEW] }
}

/** The notes of the renames of a diff as the rows of a view draw them (`renameNotesOf`) */
export interface RenameNotes {
  /** The note that stands in a block, by the index of the block */
  readonly byBlock: ReadonlyMap<number, RenameNote>
  /**
   * The note of a run of the kind `renamed` that holds a site of a rename the rows name: the note of the first such
   * rename, in the order of the lines of the run. The run has no note of its own, its lines show the name in its mark,
   * and in Inline the old lines of a quiet one show while that one note is open: a run has one note that opens it, so
   * a note never says it hides lines that another one shows
   */
  readonly ofRun: ReadonlyMap<LineRun, RenameNote>
}

/** A diff that renames no name, or a view that names none */
export const NO_RENAME_NOTES: RenameNotes = { byBlock: new Map(), ofRun: new Map() }

/**
 * What the diff says of a row of lines of a block, which a note row words (`presentRemark`): a run of
 * `TextDiff.marks.runs`, an end of a move of `TextDiff.marks.moves` (`kind: "moved"`), or the renames of
 * `TextDiff.marks.renames` that a view names there (`kind: "renamed"` with `renames`)
 */
export interface Remark {
  readonly kind: RunKind | "moved"
  /** Nothing of the lines to read (`LineRun.quiet`): they are played down, and N and P pass them */
  readonly quiet: boolean
  /** The lines of each side it is about: [start, end); of an end of a move the lines of its side alone */
  readonly lines: readonly (readonly [number, number])[]
  /**
   * Its old lines show: false while the layout shows the lines once (Inline: a quiet run by its new lines, the lines
   * of a move where they stand now), true once the note is opened; null where the layout hides no line of it, so the
   * note has nothing to open
   */
  readonly open: boolean | null
  /** The end of the move the note stands at, which its words name and a click on them shows the other end of; null for a run */
  readonly move: MoveEnd | null
  /**
   * The renames the note names ("Renamed total → sum · 12 sites"); `lines` are then empty at the site it stands at,
   * and it opens the old lines of the quiet runs that are its to open (`RenameNote.hides`). Null for any other note
   */
  readonly renames: RenameNote | null
}

/** The key of a note that opens among the open notes (`PartsOptions.open`): the first old line a run or a move hides, or the key of the note of renames */
export function noteKey(remark: Remark): number {
  return remark.renames?.key ?? remark.lines[OLD]![0]
}

/**
 * How the rows of a part show its lines: `sides` a row per line of the longer side, the shorter one filled (Split);
 * `pairs` a row per pair of lines (Inline); `stacked` the old lines, then the new ones (Unified); `new` the new lines
 * alone, which stand for the old ones (an ignored block in one column, a quiet run of Inline); `landed` the new lines
 * of a move where they stand now, in Inline: a row per line, an edited line as the pair of its old line and its new one
 */
type PartShape = "sides" | "pairs" | "stacked" | "new" | "landed"

/** Rows of lines of a block: the old lines `[oldStart, oldStart + oldCount)` and the new ones, in one shape */
interface LinesPart {
  readonly kind: "lines"
  readonly shape: PartShape
  readonly oldStart: number
  readonly oldCount: number
  readonly newStart: number
  readonly newCount: number
  readonly tint: LineTint
  /** The kind of the quiet run the lines are of; null for lines to read */
  readonly run: RunKind | null
  /** The lines of a block the whitespace policy ignores: drawn unchanged */
  readonly ignored: boolean
  /** The end of the move the lines are of; null for lines of no move */
  readonly move: MoveEnd | null
  /**
   * The key of the note that hides or shows lines of the part (`noteKey`): the lines of a quiet run that Inline shows
   * once or, its note opened, with their old lines, and the lines a move took, shown under their opened note. Null
   * for lines no note opens. What the key of the notes takes for the rows of the part, as for the note itself
   */
  readonly note: number | null
}

/** A row of words: the heading of the change the block starts, what the diff says of the lines after it, or both */
interface NotePart {
  readonly kind: "note"
  readonly heading: Heading | null
  /** The words of the heading as one line (`headingText`); empty without one */
  readonly label: string
  readonly remark: Remark | null
}

type BlockPart = LinesPart | NotePart

/** The parts of a block in a layout, with the first row of each */
export interface BlockParts {
  readonly block: DiffBlock
  /** The index of the block in the blocks of its diff */
  readonly index: number
  readonly parts: readonly BlockPart[]
  /** The first row of each part, and the row count at the end */
  readonly starts: Uint32Array
  readonly rowCount: number
}

/** What the parts of a block depend on beside its lines */
export interface PartsOptions {
  readonly mode: DiffViewMode
  /** The tint of the lines of the block that are in no quiet run */
  readonly tint: LineTint
  /** The notes the user opened, each by the first old line it hides (`getOpenNotes`) */
  readonly open: ReadonlySet<number>
  /** The heading of the change the block starts in a section of a review, its first row, and its words as one line (`headingText`) */
  readonly heading: { readonly parts: Heading; readonly label: string } | null
  /** The notes of the renames of the diff as the rows name them (`renameNotesOf`); absent for rows that name none */
  readonly renames?: RenameNotes
}

function rowsOf(part: BlockPart): number {
  if (part.kind === "note") {
    return 1
  }
  switch (part.shape) {
    case "sides":
      return Math.max(part.oldCount, part.newCount)
    case "pairs":
    case "new":
    case "landed":
      return part.newCount
    case "stacked":
      return part.oldCount + part.newCount
  }
}

/**
 * The marks of each block of a diff: its runs in their order, the ends of the moves it holds, each side in its
 * order, and the renamed names that stand on two lines it does not pair (`BlockMarks.apart`). A run lies inside one block (`TextDiff.marks.runs`), the old lines of a move inside one block and its new
 * lines inside one (`TextDiff.marks.moves`), neither in a block the whitespace policy ignores. A run that lies in no
 * block and a move with an end in none, which no engine gives, are left out, so a view shows every line of every
 * block whatever the marks say
 */
export function marksByBlock(diff: TextDiff): readonly BlockMarks[] {
  const known = marksOfDiffs.get(diff)
  if (known !== undefined) {
    return known
  }
  const { runs, moves, subject } = diff.marks
  // the runs the index proved after the rows were shown: one set for the blocks of the diff
  const confirmed = subject?.confirmed ?? []
  const kept = confirmed.length === 0 ? NO_RUNS : new Set(confirmed.flatMap(number => runs[number] ?? []))
  let next = 0
  const byBlock = diff.blocks.map(block => {
    const own: LineRun[] = []
    let oldLine = block.oldStart
    let newLine = block.newStart
    // the runs before the block are of no block
    while (next < runs.length && (runs[next]!.oldStart < block.oldStart || runs[next]!.newStart < block.newStart)) {
      next++
    }
    for (; next < runs.length; next++) {
      const run = runs[next]!
      if (run.oldStart >= block.oldEnd || run.newStart >= block.newEnd) {
        break
      }
      // inside the block, after the run before it, with a line on both sides
      if (
        run.oldStart >= oldLine &&
        run.newStart >= newLine &&
        run.oldEnd <= block.oldEnd &&
        run.newEnd <= block.newEnd &&
        run.oldStart < run.oldEnd &&
        run.newStart < run.newEnd
      ) {
        own.push(run)
        oldLine = run.oldEnd
        newLine = run.newEnd
      }
    }
    return { runs: own, sources: [] as MoveEnd[], destinations: [] as MoveEnd[], apart: [] as (readonly [number, number])[], kept }
  })
  // the names that stand on two lines a block does not pair
  for (const places of renameSites(diff).ofRename) {
    for (const site of places) {
      const at = blockOf(diff.blocks, NEW, site.line, 1)
      const block = diff.blocks[at]
      if (block !== undefined && block.inlinePairs && site.twin.line - block.oldStart !== site.line - block.newStart) {
        byBlock[at]!.apart.push([site.twin.line, site.line])
      }
    }
  }
  let sorted = true
  for (const move of moves) {
    const from = blockOf(diff.blocks, OLD, move.oldStart, move.len)
    const to = blockOf(diff.blocks, NEW, move.newStart, move.len)
    if (from < 0 || to < 0) {
      continue
    }
    const { oldStart, oldEnd } = diff.blocks[from]!
    const { newStart, newEnd } = diff.blocks[to]!
    // the blank lines that moved with the lines stand before them on one side and after them on the other
    const left: readonly [number, number] = [Math.max(move.oldStart - move.blanksBefore, oldStart), Math.min(move.oldStart + move.len + move.blanksAfter, oldEnd)]
    const stands: readonly [number, number] = [Math.max(move.newStart - move.blanksAfter, newStart), Math.min(move.newStart + move.len + move.blanksBefore, newEnd)]
    const sources = byBlock[from]!.sources
    sorted &&= sources.length === 0 || sources.at(-1)!.lines[0] <= left[0]
    sources.push({ at: "source", move, lines: left, other: stands, otherBlock: to, file: null })
    byBlock[to]!.destinations.push({ at: "destination", move, lines: stands, other: left, otherBlock: from, file: null })
  }
  // the lines that moved to or from another file of the subject: an end each, in the block that holds its lines
  const across = (at: MoveEnd["at"], { start, len, file, line, declaration }: FileMove) => {
    const block = blockOf(diff.blocks, at === "source" ? OLD : NEW, start, len)
    if (block < 0) {
      return
    }
    const [oldStart, newStart] = at === "source" ? [start, line] : [line, start]
    const move: LineMove = { oldStart, newStart, len, indent: 0, blanksBefore: 0, blanksAfter: 0, edited: [], oldRanges: [], newRanges: [] }
    const ends = at === "source" ? byBlock[block]!.sources : byBlock[block]!.destinations
    sorted &&= ends.length === 0 || ends.at(-1)!.lines[0] <= start
    // a declaration that moved whole has the lines of its other end, in its layout there
    const otherLen = declaration?.otherLen ?? len
    ends.push({ at, move, lines: [start, start + len], other: [line, line + otherLen], otherBlock: -1, file, ...(declaration == null ? {} : { declaration: declaration.name }) })
  }
  subject?.movedOut.forEach(moved => across("source", moved))
  subject?.movedIn.forEach(moved => across("destination", moved))
  // the moves come by their new lines, those between files after them: the ends of each side of a block are put in
  // the order of its lines
  if (!sorted) {
    for (const { sources, destinations } of byBlock) {
      sources.sort((a, b) => a.lines[0] - b.lines[0])
      destinations.sort((a, b) => a.lines[0] - b.lines[0])
    }
  }
  marksOfDiffs.set(diff, byBlock)
  return byBlock
}

// read once per diff: every layout of a file asks
const marksOfDiffs = new WeakMap<TextDiff, readonly BlockMarks[]>()

/**
 * The marks of block `index` as a view draws them that plays some blocks down (`played`: the blocks of the imports
 * alone in a review with its plan): without the moves that have an end in such a block, at both their ends. A move is
 * drawn at both ends or at neither. The block that is played down shows its lines as it did, pale, with no note of a
 * move; the block of the other end shows the lines of the move as its other lines, each with its row in every layout:
 * no line behind a note whose other end the view does not draw, no edited line whose change shows nowhere. The same
 * marks where nothing is left out. An end of a move between two files in a block that is played down is not drawn
 * either; its other end is in another file, which says by itself what it draws
 */
export function drawnMarks(marks: BlockMarks, index: number, played: ReadonlySet<number>): BlockMarks {
  if (played.size === 0 || (marks.sources.length === 0 && marks.destinations.length === 0)) {
    return marks
  }
  if (played.has(index)) {
    return { ...marks, sources: [], destinations: [] }
  }
  const drawn = (end: MoveEnd) => !played.has(end.otherBlock)
  return marks.sources.every(drawn) && marks.destinations.every(drawn) ? marks : { ...marks, sources: marks.sources.filter(drawn), destinations: marks.destinations.filter(drawn) }
}

/**
 * The notes of the renames of a diff as the rows of a view name them: the renames the file declares
 * (`TextDiff.marks.renames`) and the names another file of the subject declares and this one uses
 * (`SubjectMarks.used`). `shown`: the blocks the rows show (a section of a review shows some blocks of its file, the
 * others one row each); `played`: the blocks they play down (the imports alone, in a review with its plan). A rename
 * has one note: at its declaration, or, where the rows do not show that block or the file does not declare the name,
 * at its first site in a block they show, one that is not played down before one that is. The renames whose notes
 * stand in one block share one note, at the first of those sites. A rename with no site in the rows has none. A run
 * of the kind `renamed` gets the note of the first rename with a site in it, the one note that opens it
 */
export function renameNotesOf(diff: TextDiff, shown: (block: number) => boolean = () => true, played: ReadonlySet<number> = NO_BLOCKS): RenameNotes {
  const renames = namedRenames(diff)
  if (renames.length === 0) {
    return NO_RENAME_NOTES
  }
  const sites = renameSites(diff)
  const { subject } = diff.marks
  /** What the subject says of rename `index` of `namedRenames`: of a name the file declares what the index proved, of one it uses also where it is declared */
  const reachOf = (index: number): RenameReach => {
    const used = subject?.used[index - diff.marks.renames.length]
    if (used !== undefined) {
      return { proof: used.proof, declared: { file: used.file, line: used.line, moved: used.moved ?? null }, across: null }
    }
    const check = subject?.declared.find(it => it.rename === index)
    return { proof: check?.proof ?? null, declared: null, across: check === undefined ? null : { sites: check.sites, files: check.files } }
  }
  const blockAt = (site: RenameSite) => {
    const block = blockOf(diff.blocks, NEW, site.line, 1)
    return block >= 0 && shown(block) ? block : -1
  }
  // the sites the notes stand at, by block
  const standing = new Map<number, RenameSite[]>()
  sites.ofRename.forEach((places, index) => {
    const declared = renames[index]!.declared.flatMap(place => places[place] ?? [])
    const inRows = [...declared, ...places].flatMap(site => {
      const block = blockAt(site)
      return block < 0 ? [] : [{ site, block }]
    })
    const at = inRows.find(({ site, block }) => declared.includes(site) && !played.has(block)) ?? inRows.find(({ block }) => !played.has(block)) ?? inRows[0]
    if (at !== undefined) {
      standing.set(at.block, [...(standing.get(at.block) ?? []), at.site])
    }
  })
  const byBlock = new Map<number, RenameNote & { hides: (readonly [number, number])[] }>()
  const ofRename = new Map<number, RenameNote & { hides: (readonly [number, number])[] }>()
  for (const [block, at] of standing) {
    at.sort((one, other) => one.line - other.line || one.start - other.start)
    const first = at[0]!
    const note = {
      key: first.twin.line,
      renames: at.map(site => site.rename),
      reach: at.map(site => reachOf(site.index)),
      lines: [first.twin.line, first.line] as const,
      sites: at.map(site => site.line),
      hides: [],
    }
    byBlock.set(block, note)
    for (const site of at) {
      ofRename.set(site.index, note)
    }
  }
  const ofRun = new Map<LineRun, RenameNote>()
  marksByBlock(diff).forEach((marks, block) => {
    if (!shown(block)) {
      return
    }
    for (const run of marks.runs) {
      if (run.kind !== "renamed") {
        continue
      }
      // the note of the first rename with a site in the run: the one that opens it
      let owner: (RenameNote & { hides: (readonly [number, number])[] }) | undefined
      for (let line = run.newStart; owner === undefined && line < run.newEnd; line++) {
        owner = sitesOfLine(sites, NEW, line)
          .map(site => ofRename.get(site.index))
          .find(note => note !== undefined)
      }
      if (owner !== undefined) {
        ofRun.set(run, owner)
        // a quiet run that every layout keeps whole hides no line for a note to open
        if (run.quiet && !marks.kept.has(run)) {
          owner.hides.push([run.oldStart, run.oldEnd])
        }
      }
    }
  })
  return { byBlock, ofRun }
}

const NO_BLOCKS: ReadonlySet<number> = new Set()

/**
 * The index of the block that holds the `count` lines of a side from `start`, one the whitespace policy does not
 * ignore; -1 when no block holds them all, or for no line
 */
function blockOf(blocks: readonly DiffBlock[], side: number, start: number, count: number): number {
  let low = 0
  let high = blocks.length - 1
  // the last block that starts at or before the line
  while (low < high) {
    const middle = (low + high + 1) >> 1
    if ((side === OLD ? blocks[middle]!.oldStart : blocks[middle]!.newStart) <= start) {
      low = middle
    } else {
      high = middle - 1
    }
  }
  const block = blocks[low]
  if (block === undefined || count <= 0 || block.kind === "ignored") {
    return -1
  }
  const [first, end] = side === OLD ? [block.oldStart, block.oldEnd] : [block.newStart, block.newEnd]
  return first <= start && start + count <= end ? low : -1
}

/**
 * The next end of `ends` from the index `at` with a line in `[cursor, limit)`, cut to those lines; the ends before the
 * cursor are passed. Null when none starts before `limit`
 */
function nextEnd(ends: readonly MoveEnd[], at: number, cursor: number, limit: number): { readonly at: number; readonly start: number; readonly end: number } | null {
  for (; at < ends.length; at++) {
    const [first, last] = ends[at]!.lines
    if (first >= limit) {
      return null
    }
    const start = Math.max(first, cursor)
    const end = Math.min(last, limit)
    if (start < end) {
      return { at, start, end }
    }
  }
  return null
}

/**
 * The parts of a block in a layout. The runs cut it in the order of both sides: the lines between two runs are a part
 * that shows as the block did; a run is a note that says what it is, then its lines. Split and Unified show every line
 * of a run, a quiet one played down (`tint: "quiet"`); Inline shows a quiet run once, by its new lines, until its note
 * is opened, then as Unified. A run that is not quiet is a label: its lines show as those of the block. A heading
 * (`PartsOptions.heading`) is the first row of the block; a run that starts the block says its words in that row.
 *
 * The ends of the moves cut the lines between two runs once more, each on its own side: where lines left (`sources`)
 * a note says where they stand now, then the lines, which Inline leaves out until the note is opened (the note stands
 * for them); where lines stand now (`destinations`) a note says where they were, then the lines, which Inline shows
 * once, an edited one as the pair of its two lines. The lines of no move between them show as the block did. In lines
 * the engine pairs (`inlinePairs`), a cut of one side cuts the other at the same line, so the lines beside a move
 * keep their pairs and the lines whose pair moved show alone.
 *
 * A run of the kind `renamed` that holds a site of a rename the rows name (`PartsOptions.renames`) has no note of its
 * own: the name is drawn in its mark where it stands, and the one note of the rename is the row right above the line
 * of the site it stands at ("Renamed total → sum · 12 sites"): the lines that hold the site are cut there, each side
 * at its line of the site, so the lines before it stay above the note; in the row of the heading when the site starts
 * the block. In Inline that note opens the old lines of the quiet runs that are its to open (`RenameNotes.ofRun`).
 * Lines the engine pairs that hold a name on two lines it does not pair (`BlockMarks.apart`) show apart, as lines it
 * does not pair.
 *
 * An end of a move between two files of the subject (`MoveEnd.file`) is a note and its lines in every layout, Inline
 * too: the note names the other file and hides nothing, and the lines that left are lines to read. A quiet run that
 * the index of the repository proved (`BlockMarks.kept`) is played down and keeps every line in Inline too, as a
 * label does: what comes after the rows of a file were shown adds words and a tint, never takes a row.
 */
export function blockParts(block: DiffBlock, index: number, marks: BlockMarks, { mode, tint, open, heading, renames = NO_RENAME_NOTES }: PartsOptions): BlockParts {
  const parts: BlockPart[] = []
  const ignored = block.kind === "ignored"
  /**
   * The engine pairs line i of both sides in these lines: as many of each side, at the same place in the block, and no
   * renamed name among them that stands on two lines of no pair
   */
  const paired = (oldStart: number, oldCount: number, newStart: number, newCount: number) =>
    block.inlinePairs &&
    oldCount === newCount &&
    oldStart - block.oldStart === newStart - block.newStart &&
    !marks.apart.some(([oldLine, newLine]) => (oldStart <= oldLine && oldLine < oldStart + oldCount) || (newStart <= newLine && newLine < newStart + newCount))
  /** The shape of lines that show as the block does: a pair per row only where the engine pairs line i of both sides */
  const plain = (oldStart: number, oldCount: number, newStart: number, newCount: number): PartShape => {
    if (mode === "split") {
      return "sides"
    }
    if (ignored) {
      return "new"
    }
    return mode === "inline" && paired(oldStart, oldCount, newStart, newCount) ? "pairs" : "stacked"
  }
  /**
   * `opens`: of the lines of a quiet run of Inline, which hides the old ones, the key of the note that opens them and
   * whether it is open; null for lines no note opens
   */
  const lines = (oldStart: number, oldEnd: number, newStart: number, newEnd: number, run: LineRun | null, opens: { readonly key: number; readonly open: boolean } | null) => {
    const oldCount = oldEnd - oldStart
    const newCount = newEnd - newStart
    if (oldCount === 0 && newCount === 0) {
      return
    }
    const quiet = run !== null && run.quiet
    // Inline shows a quiet run once, by its new lines, until its note is opened
    const shape = quiet && opens !== null ? (opens.open ? "stacked" : "new") : plain(oldStart, oldCount, newStart, newCount)
    parts.push({
      kind: "lines",
      shape,
      oldStart,
      oldCount,
      newStart,
      newCount,
      tint: quiet ? "quiet" : tint,
      run: quiet ? run.kind : null,
      ignored,
      move: null,
      note: opens?.key ?? null,
    })
  }
  /** The note of a run or of an end of a move: in the row of the heading when it starts the block, else a row of its own */
  const note = (remark: Remark) => {
    const last = parts.at(-1)
    if (last?.kind === "note" && last.remark === null) {
      // the run starts the block: its words join the heading instead of a second row of words
      parts[parts.length - 1] = { ...last, remark }
    } else {
      parts.push({ kind: "note", heading: null, label: "", remark })
    }
  }
  let source = 0
  let destination = 0
  /** The lines of no run from the old line `oldLine` to `oldEnd` and from the new line `newLine` to `newEnd`: cut at the ends of the moves, each on its side */
  const between = (oldLine: number, oldEnd: number, newLine: number, newEnd: number) => {
    const aligned = paired(oldLine, oldEnd - oldLine, newLine, newEnd - newLine)
    const [oldFirst, newFirst] = [oldLine, newLine]
    for (;;) {
      const left = nextEnd(marks.sources, source, oldLine, oldEnd)
      const stands = nextEnd(marks.destinations, destination, newLine, newEnd)
      if (left !== null && left.start === oldLine) {
        // where lines left: only Inline hides them, behind the note, which opens; never those that left for another file
        const end = marks.sources[left.at]!
        const shown = mode === "inline" && end.file === null ? open.has(left.start) : null
        note({
          kind: "moved",
          quiet: false,
          lines: [
            [left.start, left.end],
            [newLine, newLine],
          ],
          open: shown,
          move: end,
          renames: null,
        })
        if (shown !== false) {
          parts.push({
            kind: "lines",
            shape: mode === "split" ? "sides" : "stacked",
            oldStart: left.start,
            oldCount: left.end - left.start,
            newStart: newLine,
            newCount: 0,
            tint,
            run: null,
            ignored,
            move: end,
            note: shown === null ? null : left.start,
          })
        }
        oldLine = left.end
        source = left.at + 1
        continue
      }
      if (stands !== null && stands.start === newLine) {
        // where lines stand now: every layout shows them, Inline an edited one as the pair of its two lines
        const end = marks.destinations[stands.at]!
        note({
          kind: "moved",
          quiet: false,
          lines: [
            [oldLine, oldLine],
            [stands.start, stands.end],
          ],
          open: null,
          move: end,
          renames: null,
        })
        parts.push({
          kind: "lines",
          shape: mode === "split" ? "sides" : mode === "inline" ? "landed" : "stacked",
          oldStart: oldLine,
          oldCount: 0,
          newStart: stands.start,
          newCount: stands.end - stands.start,
          tint,
          run: null,
          ignored,
          move: end,
          note: null,
        })
        newLine = stands.end
        destination = stands.at + 1
        continue
      }
      // the lines of no move up to the next end of each side
      let oldTo = left?.start ?? oldEnd
      let newTo = stands?.start ?? newEnd
      if (oldTo === oldLine && newTo === newLine) {
        return
      }
      if (aligned) {
        // line i of both sides is a pair: both sides are cut at the same line, and the lines whose pair a move took
        // show alone before the pairs go on
        const ahead = oldLine - oldFirst - (newLine - newFirst)
        if (ahead > 0 && newTo > newLine) {
          oldTo = oldLine
          newTo = Math.min(newTo, newLine + ahead)
        } else if (ahead < 0 && oldTo > oldLine) {
          newTo = newLine
          oldTo = Math.min(oldTo, oldLine - ahead)
        } else if (ahead === 0) {
          const count = Math.min(oldTo - oldLine, newTo - newLine)
          if (count > 0) {
            oldTo = oldLine + count
            newTo = newLine + count
          }
        }
      }
      lines(oldLine, oldTo, newLine, newTo, null, null)
      oldLine = oldTo
      newLine = newTo
    }
  }
  if (heading !== null) {
    parts.push({ kind: "note", heading: heading.parts, label: heading.label, remark: null })
  }
  let oldLine = block.oldStart
  let newLine = block.newStart
  for (const run of marks.runs) {
    between(oldLine, run.oldStart, newLine, run.newStart)
    // the note of a rename the rows name with a site in the run: it says what the run is, and it alone opens it
    const named = renames.ofRun.get(run)
    // only Inline hides lines: the old ones of a quiet run, behind its own note or the one of its rename; never those
    // of a run every layout keeps whole
    const key = named?.key ?? run.oldStart
    const opens = mode === "inline" && run.quiet && !marks.kept.has(run) ? { key, open: open.has(key) } : null
    if (named === undefined) {
      note({
        kind: run.kind,
        quiet: run.quiet,
        lines: [
          [run.oldStart, run.oldEnd],
          [run.newStart, run.newEnd],
        ],
        open: opens?.open ?? null,
        move: null,
        renames: null,
      })
    }
    lines(run.oldStart, run.oldEnd, run.newStart, run.newEnd, run, opens)
    oldLine = run.oldEnd
    newLine = run.newEnd
  }
  between(oldLine, block.oldEnd, newLine, block.newEnd)
  const naming = renames.byBlock.get(index)
  if (naming !== undefined) {
    // the note of the renames named at the block: the row right above the line of the site it stands at
    const [oldSite, newSite] = naming.lines
    const holds = parts.findIndex(part => part.kind === "lines" && part.newStart <= newSite && newSite < part.newStart + part.newCount)
    let at = holds < 0 ? parts.length : holds
    const cut = holds < 0 ? null : cutAt(parts[holds] as LinesPart, oldSite, newSite)
    if (cut !== null) {
      // the lines of the part before the site stay above the note
      parts.splice(at, 1, ...cut)
      at++
    }
    const after = parts[at]
    const remark: Remark = {
      kind: "renamed",
      quiet: after?.kind === "lines" && after.run !== null,
      lines: [
        [oldSite, oldSite],
        [newSite, newSite],
      ],
      open: mode === "inline" && naming.hides.length > 0 ? open.has(naming.key) : null,
      move: null,
      renames: naming,
    }
    const before = parts[at - 1]
    if (before?.kind === "note" && before.remark === null) {
      // the lines start the block: the words join its heading
      parts[at - 1] = { ...before, remark }
    } else {
      parts.splice(at, 0, { kind: "note", heading: null, label: "", remark })
    }
  }
  const starts = new Uint32Array(parts.length + 1)
  parts.forEach((part, at) => {
    starts[at + 1] = starts[at]! + rowsOf(part)
  })
  return { block, index, parts, starts, rowCount: starts[parts.length]! }
}

/**
 * A part of lines cut at a site of a rename, the old side at `oldLine` and the new one at `newLine`, a line of the
 * part: the lines before the site, then the lines from it. Place `i` of a rename is the same place on both sides, so
 * what stands before it on one side stands before it on the other. Null where the site starts the part (no line of it
 * would have a row above the site), and for the lines of a move, which have a note of their own
 */
function cutAt(part: LinesPart, oldLine: number, newLine: number): [LinesPart, LinesPart] | null {
  if (part.move !== null) {
    return null
  }
  const newCut = newLine - part.newStart
  // a row per pair of lines: both sides at the same line; an old line of the site outside the part: at its nearer end
  const oldCut = part.shape === "pairs" ? newCut : Math.min(Math.max(oldLine - part.oldStart, 0), part.oldCount)
  const before: LinesPart = { ...part, oldCount: oldCut, newCount: newCut }
  if (rowsOf(before) === 0) {
    return null
  }
  return [before, { ...part, oldStart: part.oldStart + oldCut, oldCount: part.oldCount - oldCut, newStart: part.newStart + newCut, newCount: part.newCount - newCut }]
}

/** The index of the part that holds row `row` */
function partAt(parts: BlockParts, row: number): number {
  let low = 0
  let high = parts.parts.length - 1
  while (low < high) {
    const middle = (low + high + 1) >> 1
    if (parts.starts[middle]! <= row) {
      low = middle
    } else {
      high = middle - 1
    }
  }
  return low
}

/** The lines a note stands for: the old lines of its run, or of the move that left, that have no row (`Remark.open` false), none else */
function noteLines(remark: Remark | null, block: DiffBlock): [number, number][] {
  if (remark === null) {
    return [
      [block.oldStart, block.oldStart],
      [block.newStart, block.newStart],
    ]
  }
  const [oldStart, oldEnd] = remark.lines[OLD]!
  const newStart = remark.lines[NEW]![0]
  return [
    [oldStart, remark.open === false ? oldEnd : oldStart],
    [newStart, newStart],
  ]
}

/** Row `row` of the parts of a block */
export function partsRow(parts: BlockParts, row: number): LineRow | NoteRow {
  const at = partAt(parts, row)
  const part = parts.parts[at]!
  const { block, index } = parts
  if (part.kind === "note") {
    const { heading, label, remark } = part
    return { kind: "note", tone: heading === null ? "remark" : "unit", label, heading, block: index, lines: noteLines(remark, block), remark }
  }
  const offset = row - parts.starts[at]!
  const { move } = part
  const line = (kind: LineRowKind, oldLine: number | null, newLine: number | null): LineRow => ({
    kind,
    lines: [oldLine, newLine],
    block: index,
    blockRow: row,
    blockRows: parts.rowCount,
    // a line of a move has the colour of the move: pale where the line was edited on the way, as a block with inner ranges
    tint: move === null ? part.tint : isEditedLine(move, (move.at === "source" ? oldLine : newLine) ?? -1) ? "pale" : "full",
    run: part.run,
    move,
  })
  switch (part.shape) {
    case "sides":
      return line(part.ignored ? "ignored" : "changed", offset < part.oldCount ? part.oldStart + offset : null, offset < part.newCount ? part.newStart + offset : null)
    case "pairs":
      return line("inline", part.oldStart + offset, part.newStart + offset)
    case "stacked":
      return offset < part.oldCount ? line("deleted", part.oldStart + offset, null) : line("added", null, part.newStart + offset - part.oldCount)
    case "new":
      return line(part.ignored ? "ignored" : "once", null, part.newStart + offset)
    case "landed": {
      // an edited line of a move is the pair of its old line, which stands where the lines left, and its new one
      const newLine = part.newStart + offset
      return move !== null && isEditedLine(move, newLine) ? line("inline", move.move.oldStart + newLine - move.move.newStart, newLine) : line("added", null, newLine)
    }
  }
}

/**
 * Whether `line` of the side of an end of a move is a line the move says was edited on the way (`LineMove.edited`, by
 * its place in the move); never a blank line that moved beside it
 */
function isEditedLine({ at, move }: MoveEnd, line: number): boolean {
  const place = line - (at === "source" ? move.oldStart : move.newStart)
  if (place < 0 || place >= move.len) {
    return false
  }
  // in order: a move may hold thousands of lines
  let low = 0
  let high = move.edited.length - 1
  while (low <= high) {
    const middle = (low + high) >> 1
    const edited = move.edited[middle]!
    if (edited === place) {
      return true
    }
    if (edited < place) {
      low = middle + 1
    } else {
      high = middle - 1
    }
  }
  return false
}

/**
 * The row of the parts where `line` of a side, or a later line of it, shows; the row count when no row does. An old
 * line without a row of its own gives the row that stands for it: the note of its quiet run or of the move it left
 * with in Inline; in lines that one column shows by the new ones alone (a block the whitespace policy ignores, a quiet
 * run of a renamed name, which has no note of its own) the row of the new line at its place where both sides have as
 * many lines, else the first row of those lines. The old line of an edited line of a move is found where the lines
 * left, never in the row of its pair where they stand now
 */
export function partsRowOfLine(parts: BlockParts, side: number, line: number): number {
  for (const [at, part] of parts.parts.entries()) {
    const start = parts.starts[at]!
    if (part.kind === "note") {
      // a closed note of a run or of a move stands for its old lines; the note of renames for none (the rows of the new lines do)
      if (side === OLD && part.remark?.open === false && part.remark.renames === null && line < part.remark.lines[OLD]![1]) {
        return start
      }
      continue
    }
    const first = side === OLD ? part.oldStart : part.newStart
    const count = side === OLD ? part.oldCount : part.newCount
    if (line >= first + count) {
      continue
    }
    const offset = Math.max(line - first, 0)
    switch (part.shape) {
      case "sides":
      case "pairs":
        return start + offset
      case "stacked":
        return start + (side === OLD ? offset : part.oldCount + offset)
      case "new":
        // the old lines have no row: where each has its new line (as many of both sides) that row stands for it, else
        // the first row of the lines
        return start + (side === NEW || part.oldCount === part.newCount ? offset : 0)
      case "landed":
        return start + offset
    }
  }
  return parts.rowCount
}

/**
 * The row the reading of a block starts at in its layout, from `line` of a side, the line Rust names
 * (`TextDiff.marks.starts`). Rust cuts a block by its quiet runs alone and names the first new line of what is read
 * first, else its first old one (`line_diff::model::starts_of`); a view cuts the block at every run, a label too, and
 * shows the old lines of a part before its new ones. So the stop is the first row of lines of what Rust reads there:
 * from the part that shows the line back over the parts of no quiet run (lines to read, the note of a label) to the
 * one after the last quiet run. Lines deleted before a comment worded again are read from their first row, though
 * Rust names the comment, the first new line. The lines a move within the file took are nothing to read either (Rust
 * starts a block at a line no such move took): they end what is read before them as a quiet run does; the lines that
 * left for another file are read. The row count when no part shows the line
 */
export function partsStopRow(parts: BlockParts, side: number, line: number): number {
  // the first part of lines to read since the last quiet run
  let first = -1
  for (const [at, part] of parts.parts.entries()) {
    if (part.kind !== "lines") {
      continue
    }
    if (part.run !== null || (part.move?.at === "source" && part.move.file === null)) {
      first = -1
    } else if (first < 0) {
      first = at
    }
    const start = side === OLD ? part.oldStart : part.newStart
    if (start <= line && line < start + (side === OLD ? part.oldCount : part.newCount)) {
      // a line of a quiet run or one a move took, which Rust never names, is where it shows
      return parts.starts[first < 0 ? at : first]!
    }
  }
  return parts.rowCount
}

/** The lines of each side that rows `[from, from + count)` of the parts cover, the lines they stand for with them */
export function partsLines(parts: BlockParts, from: number, count: number): [number, number][] {
  const { block } = parts
  if (from <= 0 && from + count >= parts.rowCount) {
    return [
      [block.oldStart, block.oldEnd],
      [block.newStart, block.newEnd],
    ]
  }
  let covered: [number, number][] | undefined
  for (const [at, part] of parts.parts.entries()) {
    const rows = parts.starts[at + 1]! - parts.starts[at]!
    const start = Math.max(from - parts.starts[at]!, 0)
    const end = Math.min(from + count - parts.starts[at]!, rows)
    if (start >= end) {
      continue
    }
    const lines = part.kind === "note" ? noteLines(part.remark, block) : linesOf(part, start, end)
    covered = covered === undefined ? lines : covered.map((range, side) => [range[0], lines[side]![1]])
  }
  return (
    covered ?? [
      [block.oldStart, block.oldStart],
      [block.newStart, block.newStart],
    ]
  )
}

/** The lines of each side that rows `[from, to)` of a part of lines cover */
function linesOf(part: LinesPart, from: number, to: number): [number, number][] {
  const clip = (start: number, length: number, first: number, last: number): [number, number] => [
    start + Math.min(Math.max(first, 0), length),
    start + Math.min(Math.max(last, 0), length),
  ]
  switch (part.shape) {
    case "sides":
      return [clip(part.oldStart, part.oldCount, from, to), clip(part.newStart, part.newCount, from, to)]
    case "pairs":
      return [clip(part.oldStart, part.oldCount, from, to), clip(part.newStart, part.newCount, from, to)]
    case "stacked":
      return [clip(part.oldStart, part.oldCount, from, to), clip(part.newStart, part.newCount, from - part.oldCount, to - part.oldCount)]
    case "landed":
      // the old line of an edited pair is a line of where the lines left
      return [clip(part.oldStart, 0, 0, 0), clip(part.newStart, part.newCount, from, to)]
    case "new":
      // the old lines have no row: the first row stands for them from their start, the last one to their end
      return [clip(part.oldStart, part.oldCount, from === 0 ? 0 : from, to === part.newCount ? part.oldCount : to), clip(part.newStart, part.newCount, from, to)]
  }
}

/**
 * The notes of the parts that open (`Remark.open` not null), each with its key (`noteKey`), its row and the old lines
 * it hides or would hide, [start, end) each: the lines of its run or of the move that left, or of the note of renames
 * the lines of the quiet runs that are its to open, in this block and in others
 */
export function partsNotes(parts: BlockParts): { readonly key: number; readonly lines: readonly (readonly [number, number])[]; readonly row: number }[] {
  return parts.parts.flatMap((part, at) =>
    part.kind === "note" && part.remark !== null && part.remark.open !== null
      ? [{ key: noteKey(part.remark), lines: part.remark.renames?.hides ?? [part.remark.lines[OLD]!], row: parts.starts[at]! }]
      : [],
  )
}

/**
 * The key of the note that hides or shows lines of row `row` of the parts (`LinesPart.note`): a line that Inline shows
 * once, or a line under an opened note; null for a row of words and for a line no note opens
 */
export function partsNoteOfRow(parts: BlockParts, row: number): number | null {
  const part = parts.parts[partAt(parts, row)]
  return part?.kind === "lines" ? part.note : null
}

/** The notes of the parts that stand at an end of a move, each with its row */
export function partsMoveNotes(parts: BlockParts): { readonly end: MoveEnd; readonly row: number }[] {
  return parts.parts.flatMap((part, at) => (part.kind === "note" && part.remark !== null && part.remark.move !== null ? [{ end: part.remark.move, row: parts.starts[at]! }] : []))
}

/** A pair of lines that Inline shows as one row, with the inner ranges of each line (`[start, end, kind, ...]`) */
export interface InlineRowPair {
  readonly lines: readonly [number, number]
  readonly oldRanges: readonly number[]
  readonly newRanges: readonly number[]
}

/**
 * The pairs of lines of a block that Inline shows as one row each, whatever notes are open: the rows of its `pairs`
 * parts with the ranges of the block, and the edited lines of the moves that stand in it with the ranges of their
 * move (`landed`)
 */
export function inlinePairsOf(block: DiffBlock, index: number, marks: BlockMarks): InlineRowPair[] {
  const pairs: InlineRowPair[] = []
  // no pair of the engine and no edited line of a move: no row of two lines
  if (block.kind === "ignored" || !(block.inlinePairs || marks.destinations.some(end => end.move.edited.length > 0))) {
    return pairs
  }
  for (const part of blockParts(block, index, marks, { mode: "inline", tint: "full", open: NO_NOTES, heading: null }).parts) {
    if (part.kind !== "lines") {
      continue
    }
    if (part.shape === "pairs") {
      for (let row = 0; row < part.newCount; row++) {
        const [oldLine, newLine] = [part.oldStart + row, part.newStart + row]
        pairs.push({ lines: [oldLine, newLine], oldRanges: block.oldRanges[oldLine - block.oldStart] ?? [], newRanges: block.newRanges[newLine - block.newStart] ?? [] })
      }
    } else if (part.shape === "landed" && part.move !== null) {
      const { move } = part.move
      for (const place of move.edited) {
        const newLine = move.newStart + place
        if (newLine >= part.newStart && newLine < part.newStart + part.newCount) {
          pairs.push({ lines: [move.oldStart + place, newLine], oldRanges: move.oldRanges[place] ?? [], newRanges: move.newRanges[place] ?? [] })
        }
      }
    }
  }
  return pairs
}

const NO_NOTES: ReadonlySet<number> = new Set()

/**
 * The stops of a diff by block, in the order of its blocks: the line the reading of each block starts at, as Rust
 * says it (`TextDiff.marks.starts`, flat `[block, side, line]`). A block without one has nothing to read; with
 * `quietImports` (a review with its plan) a block of the imports alone is no stop either. The one source of where N
 * and P stop; the count of what is left to read of a file is Rust's (`FileDiff.reading`), from the same starts
 */
export function stopsOf(diff: TextDiff, quietImports = false): Map<number, { readonly side: number; readonly line: number }> {
  const stops = new Map<number, { side: number; line: number }>()
  const { starts } = diff.marks
  for (let at = 0; at + 2 < starts.length; at += 3) {
    if (!(quietImports && diff.importBlocks.includes(starts[at]!))) {
      stops.set(starts[at]!, { side: starts[at + 1]!, line: starts[at + 2]! })
    }
  }
  return stops
}

/** A block of changed lines: one the whitespace policy does not ignore, with a line (the stand-in of a use left outside has none) */
export function isChangeBlock(block: DiffBlock): boolean {
  return block.kind !== "ignored" && (block.oldStart < block.oldEnd || block.newStart < block.newEnd)
}
