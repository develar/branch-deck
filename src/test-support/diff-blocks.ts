import type { DiffBlock, FileMove, LineMove, LineRun, Rename, Scope, SubjectMarks, TextDiff } from "@/lib/diff/file-diff-model"
import type { MergeBlock, TextMerge } from "@/lib/diff/file-merge-model"

/** A block of the diff engine: a change by default, without inner ranges */
export function block(oldStart: number, oldEnd: number, newStart: number, newEnd: number, extra: Partial<DiffBlock> = {}): DiffBlock {
  return { oldStart, oldEnd, newStart, newEnd, kind: "changed", inlinePairs: false, oldRanges: [], newRanges: [], ...extra }
}

/** A run of the marks of a diff: lines of a block that changed in their formatting alone, quiet, by default */
export function run(oldStart: number, oldEnd: number, newStart: number, newEnd: number, extra: Partial<LineRun> = {}): LineRun {
  return { oldStart, oldEnd, newStart, newEnd, kind: "formatting", quiet: true, ...extra }
}

/**
 * A move of the marks of a diff: `len` lines from the old line `oldStart` stand from the new line `newStart`, as they
 * were by default (no shift of the indent, no blank line beside them, no edited line)
 */
export function move(oldStart: number, newStart: number, len: number, extra: Partial<LineMove> = {}): LineMove {
  return { oldStart, newStart, len, indent: 0, blanksBefore: 0, blanksAfter: 0, edited: [], oldRanges: [], newRanges: [], ...extra }
}

/**
 * A rename of the marks of a diff: the name `old` is `renamed` at `sites`, each `[old line, old column, new line, new
 * column]` of where the name starts on both sides, in the order of the texts; a local whose first site declares it, by
 * default
 */
export function rename(old: string, renamed: string, sites: readonly (readonly [number, number, number, number])[], extra: Partial<Rename> = {}): Rename {
  return {
    old,
    new: renamed,
    sites: sites.length,
    local: true,
    declared: [0],
    oldSites: sites.flatMap(([line, column]) => [line, column, column + old.length]),
    newSites: sites.flatMap(([, , line, column]) => [line, column, column + renamed.length]),
    ...extra,
  }
}

/**
 * Lines that moved between two files of a subject, as the file of one end says it (`SubjectMarks.movedOut`, `movedIn`):
 * `len` lines of this file from `start`, whose other end is in file `file` of the subject, from its line `line`
 */
export function fileMove(start: number, len: number, file: number, line: number): FileMove {
  return { start, len, file, line }
}

/** What the other files of a subject say of a file (`DiffMarks.subject`): nothing by default */
export function subjectMarks(extra: Partial<SubjectMarks> = {}): SubjectMarks {
  return { movedOut: [], movedIn: [], declared: [], used: [], confirmed: [], ...extra }
}

/**
 * Where the reading of each block starts, as the engine says it (`text_diff::model::starts_of`): the quiet runs cut a
 * block into parts, the start is the first new line of the first part with a line, else its first old line that no
 * move took; an ignored block, one whose every line is in a quiet run and one whose old lines the moves took all have
 * none. Flat `[block, side, line]`
 */
export function startsOf(blocks: readonly DiffBlock[], runs: readonly LineRun[], moves: readonly LineMove[] = []): number[] {
  const starts: number[] = []
  // the old lines of the moves, the blank lines that left with them too (`LineMove::old_lines`)
  const left = moves.map(it => [it.oldStart - it.blanksBefore, it.oldStart + it.len + it.blanksAfter] as const).toSorted((a, b) => a[0] - b[0])
  /** The first line of `[from, to)` that no move took; `to` when none */
  const firstKept = (from: number, to: number) => {
    let line = from
    for (const [start, end] of left) {
      if (start <= line && line < end) {
        line = end
      }
    }
    return Math.min(line, to)
  }
  blocks.forEach((it, index) => {
    if (it.kind === "ignored") {
      return
    }
    const quiet = runs.filter(one => one.quiet && one.oldStart >= it.oldStart && one.oldEnd <= it.oldEnd && one.newStart >= it.newStart && one.newEnd <= it.newEnd)
    let oldLine = it.oldStart
    let newLine = it.newStart
    for (const part of [...quiet, null]) {
      if (newLine < (part?.newStart ?? it.newEnd)) {
        starts.push(index, 1, newLine)
        return
      }
      const oldEnd = part?.oldStart ?? it.oldEnd
      const kept = firstKept(oldLine, oldEnd)
      if (kept < oldEnd) {
        starts.push(index, 0, kept)
        return
      }
      oldLine = part?.oldEnd ?? oldLine
      newLine = part?.newEnd ?? newLine
    }
  })
  return starts
}

/**
 * The text diff of two texts of these line counts with these blocks, and the runs, the moves and the renames of its
 * marks, and what the other files of its subject say of it (`subject`, none by default); its starts as the engine
 * gives them
 */
export function textDiff(
  oldLineCount: number,
  newLineCount: number,
  blocks: DiffBlock[],
  {
    oldScopes = [],
    newScopes = [],
    emptyLastLine = false,
    importBlocks = [],
    runs = [],
    moves = [],
    renames = [],
    subject,
  }: {
    oldScopes?: Scope[]
    newScopes?: Scope[]
    emptyLastLine?: boolean
    importBlocks?: number[]
    runs?: LineRun[]
    moves?: LineMove[]
    renames?: Rename[]
    subject?: SubjectMarks
  } = {},
): TextDiff {
  return {
    oldLineCount,
    newLineCount,
    additions: 0,
    deletions: 0,
    lineEnds: null,
    finalNewline: null,
    emptyLastLine: { old: emptyLastLine, new: emptyLastLine },
    lineLevelOnly: false,
    blocks,
    importBlocks,
    oldScopes,
    newScopes,
    marks: { runs, moves, renames, starts: startsOf(blocks, runs, moves), ...(subject !== undefined && { subject }) },
  }
}

/** Three nested declarations of a 40-line text: a class, and two functions in it that overlap */
export const NESTED_SCOPES: Scope[] = [
  { startLine: 0, endLine: 40, label: "class A" },
  { startLine: 2, endLine: 30, label: "class A › fun read" },
  { startLine: 25, endLine: 40, label: "class A › fun write" },
]

/** A block of a merge: lines `[start, end)` of left, base and right; a conflict by default, without inner ranges */
export function mergeBlock(left: [number, number], base: [number, number], right: [number, number], extra: Partial<MergeBlock> = {}): MergeBlock {
  return {
    leftStart: left[0],
    leftEnd: left[1],
    baseStart: base[0],
    baseEnd: base[1],
    rightStart: right[0],
    rightEnd: right[1],
    kind: "conflict",
    leftChanged: true,
    rightChanged: true,
    ignored: false,
    resolution: null,
    resolvedText: null,
    leftRanges: [],
    baseRanges: [],
    rightRanges: [],
    ...extra,
  }
}

/** The text merge of three texts of these line counts (left, base, right) with these blocks */
export function textMerge([leftLineCount, baseLineCount, rightLineCount]: [number, number, number], blocks: MergeBlock[], extra: Partial<TextMerge> = {}): TextMerge {
  return {
    leftLineCount,
    baseLineCount,
    rightLineCount,
    lineEnds: null,
    finalNewline: null,
    emptyLastLine: { left: false, base: false, right: false },
    lineLevelOnly: false,
    blocks,
    conflicts: blocks.filter(block => block.kind === "conflict").length,
    unresolved: blocks.filter(block => block.kind === "conflict" && block.resolution === null).length,
    leftScopes: [],
    baseScopes: [],
    rightScopes: [],
    ...extra,
  }
}
