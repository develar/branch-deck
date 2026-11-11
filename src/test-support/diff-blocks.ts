import type { DiffBlock, LineRun, Scope, TextDiff } from "@/lib/diff/file-diff-model"
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
 * Where the reading of each block starts, as the engine says it (`text_diff::model::starts_of`): the quiet runs cut a
 * block into parts, the start is the first new line of the first part with a line, else its first old line; an
 * ignored block and one whose every line is in a quiet run have none. Flat `[block, side, line]`
 */
export function startsOf(blocks: readonly DiffBlock[], runs: readonly LineRun[]): number[] {
  const starts: number[] = []
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
      if (oldLine < (part?.oldStart ?? it.oldEnd)) {
        starts.push(index, 0, oldLine)
        return
      }
      oldLine = part?.oldEnd ?? oldLine
      newLine = part?.newEnd ?? newLine
    }
  })
  return starts
}

/** The text diff of two texts of these line counts with these blocks, and the runs of its marks; its starts as the engine gives them */
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
  }: { oldScopes?: Scope[]; newScopes?: Scope[]; emptyLastLine?: boolean; importBlocks?: number[]; runs?: LineRun[] } = {},
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
    marks: { runs, moves: [], renames: [], starts: startsOf(blocks, runs) },
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
