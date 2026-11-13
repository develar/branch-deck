import { createHash } from "node:crypto"
import type { BlobText, CommitFile } from "../../src/lib/bindings"
import type { DiffBlock, FileDiff, LineMove, LineRun, Rename } from "../../src/lib/diff/file-diff-model"

// Synthetic commits for the diff benchmark: Kotlin-like files whose lines each hold several tokens (a keyword, names,
// a number, a string, a comment), so a highlighted line differs from a plain one, with the diffs of the engine and
// their marks: every fourth changed line of a file changes in its formatting alone, a quiet run with its note, two
// changes of every eight are the ends of a move, a line that leaves one place and stands at the next, every second
// one edited on the way, and one of every eight changes in the name of its value alone, a rename with its note

/** A file of a synthetic commit: its row in the files, its diff as the backend sends it, the blobs of both sides */
export interface SyntheticFile {
  file: CommitFile
  diff: FileDiff
  blobs: BlobText[]
}

export interface FileSpec {
  path: string
  lines: number
  /** Every `changeEvery`-th line changes (1:1); 0 for a new file */
  changeEvery: number
}

/**
 * Of the changed lines of a file, every `QUIET_EVERY`-th changes in its formatting alone: the marks call it a quiet
 * run. `PERF_QUIET_EVERY=0` gives a commit without a run, to tell the cost of the notes from the rest
 */
const QUIET_EVERY = Number(process.env.PERF_QUIET_EVERY ?? 4)

/**
 * Of every `MOVE_EVERY` changed lines of a file, the first is a line that leaves (a block that deletes it) and the
 * second the place it stands at now (a block that inserts it before the line there): a move of the marks, with a note
 * at each end; every second move is edited on the way, the row of two lines of Inline. `PERF_MOVE_EVERY=0` gives a
 * commit without a move, to tell the cost of the rows of a move from the rest
 */
const MOVE_EVERY = Number(process.env.PERF_MOVE_EVERY ?? 8)

/**
 * Of every `RENAME_EVERY` changed lines of a file, the third changes in the name of its value alone: a rename of the
 * marks with that one site, its declaration, in a run of the kind `renamed` that is a label, as the engine gives it:
 * a note before the line, the name in its mark. `PERF_RENAME_EVERY=0` gives a commit without a rename, to tell the
 * cost of the sites from the rest
 */
const RENAME_EVERY = Number(process.env.PERF_RENAME_EVERY ?? 8)

/**
 * `version`: 0 the old line, 1 the line with another number, `"formatted"` the old line with spaces inside its call,
 * `"renamed"` the old line with another name of its value
 */
function codeLine(fileIndex: number, index: number, version: 0 | 1 | "formatted" | "renamed"): string {
  const call = version === "formatted" ? `compute( ${index}, "file${fileIndex}" )` : `compute(${index + (version === 1 ? 1 : 0)}, "file${fileIndex}")`
  return `  val ${version === "renamed" ? "named" : "value"}${index} = ${call} // line ${index}`
}

/** The column of the name of the value of `codeLine` */
const NAME_COLUMN = "  val ".length

/** The offsets of the number argument of `codeLine`: the range the engine marks in a changed line, a modified one */
function numberRange(line: string): [number, number, number] {
  const start = line.indexOf("(") + 1
  return [start, line.indexOf(",", start), 2]
}

function blobId(path: string, side: string): string {
  return createHash("sha1").update(`${side}:${path}`).digest("hex")
}

/** A modified file with 1:1 changes, quiet runs and moves, or a new file, as the engine sends its diff */
export function syntheticFile(fileIndex: number, spec: FileSpec): SyntheticFile {
  const changes = (index: number) => spec.changeEvery !== 0 && index % spec.changeEvery === spec.changeEvery - 1
  /** The place of the changed line `index` among the changed lines of the file */
  const ordinal = (index: number) => Math.floor(index / spec.changeEvery)
  /** The changed line `index` leaves for the next changed line: the old end of a move */
  const leaves = (index: number) => MOVE_EVERY > 1 && changes(index) && ordinal(index) % MOVE_EVERY === 0 && index + spec.changeEvery < spec.lines
  /** The line that left the changed line before stands before the changed line `index` now: the new end of a move */
  const lands = (index: number) => MOVE_EVERY > 1 && changes(index) && ordinal(index) % MOVE_EVERY === 1
  /** The move that lands before `index` is edited on the way: every second move of the file */
  const edits = (index: number) => Math.floor(ordinal(index) / MOVE_EVERY) % 2 === 1
  /** The changed line `index` changes in its formatting alone: every `QUIET_EVERY`-th change of the file */
  const formats = (index: number) => QUIET_EVERY > 0 && changes(index) && !leaves(index) && !lands(index) && ordinal(index) % QUIET_EVERY === QUIET_EVERY - 1
  /** The changed line `index` changes in the name of its value alone: the third of every `RENAME_EVERY` changes of the file */
  const renames = (index: number) => RENAME_EVERY > 2 && changes(index) && !leaves(index) && !lands(index) && !formats(index) && ordinal(index) % RENAME_EVERY === 2
  const oldLines = spec.changeEvery === 0 ? [] : Array.from({ length: spec.lines }, (_, index) => codeLine(fileIndex, index, 0))
  const newLines: string[] = []
  const blocks: DiffBlock[] = []
  const runs: LineRun[] = []
  const moves: LineMove[] = []
  const renamed: Rename[] = []
  const starts: number[] = []
  /** A block of lines the engine pairs none of: no inner ranges */
  const unpaired = (lines: Pick<DiffBlock, "oldStart" | "oldEnd" | "newStart" | "newEnd">): DiffBlock => ({
    ...lines,
    kind: "changed",
    inlinePairs: false,
    oldRanges: [],
    newRanges: [],
  })
  for (let index = 0; index < spec.lines; index++) {
    if (spec.changeEvery === 0) {
      newLines.push(codeLine(fileIndex, index, 0))
    } else if (leaves(index)) {
      // the line is gone here: a block that deletes it, nothing of it to read
      blocks.push(unpaired({ oldStart: index, oldEnd: index + 1, newStart: newLines.length, newEnd: newLines.length }))
    } else if (lands(index)) {
      // the line that left stands before this one, which stays as it is: a block that inserts it, read where it stands
      const from = index - spec.changeEvery
      const moved = codeLine(fileIndex, from, edits(index) ? 1 : 0)
      const edited = edits(index) ? { edited: [0], oldRanges: [numberRange(oldLines[from]!)], newRanges: [numberRange(moved)] } : { edited: [], oldRanges: [], newRanges: [] }
      moves.push({ oldStart: from, newStart: newLines.length, len: 1, indent: 0, blanksBefore: 0, blanksAfter: 0, ...edited })
      starts.push(blocks.length, 1, newLines.length)
      blocks.push(unpaired({ oldStart: index, oldEnd: index, newStart: newLines.length, newEnd: newLines.length + 1 }))
      newLines.push(moved, oldLines[index]!)
    } else if (changes(index)) {
      const line = codeLine(fileIndex, index, formats(index) ? "formatted" : renames(index) ? "renamed" : 1)
      const lines = { oldStart: index, oldEnd: index + 1, newStart: newLines.length, newEnd: newLines.length + 1 }
      if (renames(index)) {
        // the name of the value alone: a pair of the engine with the name as its range, a run that is a label, read
        const [old, name] = [`value${index}`, `named${index}`]
        const site = (line: number, text: string) => [line, NAME_COLUMN, NAME_COLUMN + text.length]
        renamed.push({ old, new: name, sites: 1, local: true, declared: [0], oldSites: site(index, old), newSites: site(lines.newStart, name) })
        runs.push({ ...lines, kind: "renamed", quiet: false })
        starts.push(blocks.length, 1, lines.newStart)
        blocks.push({ ...lines, kind: "changed", inlinePairs: true, oldRanges: [[...site(index, old).slice(1), 2]], newRanges: [[...site(lines.newStart, name).slice(1), 2]] })
      } else if (formats(index)) {
        // a line that only gained spaces is no similar pair the views draw: its run shows it, quiet, with no start
        runs.push({ ...lines, kind: "formatting", quiet: true })
        blocks.push(unpaired(lines))
      } else {
        starts.push(blocks.length, 1, lines.newStart)
        blocks.push({ ...lines, kind: "changed", inlinePairs: true, oldRanges: [numberRange(oldLines[index]!)], newRanges: [numberRange(line)] })
      }
      newLines.push(line)
    } else {
      newLines.push(oldLines[index]!)
    }
  }
  if (spec.changeEvery === 0) {
    blocks.push(unpaired({ oldStart: 0, oldEnd: 0, newStart: 0, newEnd: spec.lines }))
    starts.push(0, 1, 0)
  }
  const oldId = spec.changeEvery === 0 ? null : blobId(spec.path, "old")
  const newId = blobId(spec.path, "new")
  const additions = blocks.reduce((sum, block) => sum + block.newEnd - block.newStart, 0)
  const deletions = blocks.reduce((sum, block) => sum + block.oldEnd - block.oldStart, 0)
  const text = (lines: string[]) => `${lines.join("\n")}\n`
  const blobs: BlobText[] = [{ kind: "text", id: newId, text: text(newLines) }]
  if (oldId !== null) {
    blobs.push({ kind: "text", id: oldId, text: text(oldLines) })
  }
  return {
    file: { path: spec.path, status: spec.changeEvery === 0 ? "added" : "modified", oldPath: null, additions, deletions, binary: false, oldId, newId, submodule: false },
    diff: {
      path: spec.path,
      oldPath: null,
      status: spec.changeEvery === 0 ? "added" : "modified",
      oldId,
      newId,
      // as the backend's `collapse_of` says it: over 1,000 changed lines a file starts collapsed
      collapse: additions + deletions > 1000 ? { kind: "large", changed: additions + deletions } : null,
      // as the backend's `DiffReading` counts it: every block a change, those with a start to read
      reading: { changes: blocks.length, toRead: starts.length / 3, toReadPastImports: starts.length / 3 },
      content: {
        kind: "text",
        // the engine counts the empty line after the final line end, and the one line of an empty text
        oldLineCount: oldLines.length + 1,
        newLineCount: newLines.length + 1,
        additions,
        deletions,
        lineEnds: null,
        finalNewline: null,
        emptyLastLine: { old: true, new: true },
        lineLevelOnly: false,
        blocks,
        oldScopes: [],
        newScopes: [{ startLine: 0, endLine: spec.lines, label: `class File${fileIndex}` }],
        importBlocks: [],
        // the marks as the engine gives them: a quiet run for each line that changed in its formatting alone, the
        // moves by their new lines, the renames with a run that is a label, and the start of the reading of every
        // block but a quiet one and one a move emptied
        marks: { runs, moves, renames: renamed, starts },
      },
    },
    blobs,
  }
}

/**
 * A large commit: 200 files in 20 directories, most of them small with a few changes, 4 of 2,000-5,000 lines with a
 * change every 12 lines, and 10 new files; every fourth change of a file a quiet run of formatting, two of every eight
 * the ends of a move, one of every eight a rename
 */
export function largeCommit(): SyntheticFile[] {
  const specs: FileSpec[] = []
  for (let index = 0; index < 200; index++) {
    const directory = `module${index % 20}/src/main/kotlin`
    if (index % 50 === 7) {
      specs.push({ path: `${directory}/Large${index}.kt`, lines: 2000 + (index % 4) * 1000, changeEvery: 12 })
    } else if (index % 20 === 3) {
      specs.push({ path: `${directory}/New${index}.kt`, lines: 40, changeEvery: 0 })
    } else {
      specs.push({ path: `${directory}/File${index}.kt`, lines: 80 + (index % 7) * 20, changeEvery: 25 })
    }
  }
  return sortByPath(specs).map((spec, index) => syntheticFile(index, spec))
}

/**
 * A commit of one file of 10,000 lines with a change every 20 lines, every fourth of them a quiet run of formatting,
 * two of every eight the ends of a move: 63 moves, half of them edited on the way
 */
export function hugeFileCommit(): SyntheticFile[] {
  return [syntheticFile(0, { path: "core/src/main/kotlin/Huge.kt", lines: 10_000, changeEvery: 20 })]
}

function sortByPath(specs: FileSpec[]): FileSpec[] {
  return specs.toSorted((a, b) => (a.path < b.path ? -1 : a.path > b.path ? 1 : 0))
}
