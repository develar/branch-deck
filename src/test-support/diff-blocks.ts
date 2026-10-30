import type { DiffBlock, Scope, TextDiff } from "@/lib/diff/file-diff-model"
import type { MergeBlock, TextMerge } from "@/lib/diff/file-merge-model"

/** A block of the diff engine: a change by default, without inner ranges */
export function block(oldStart: number, oldEnd: number, newStart: number, newEnd: number, extra: Partial<DiffBlock> = {}): DiffBlock {
  return { oldStart, oldEnd, newStart, newEnd, kind: "changed", inlinePairs: false, oldRanges: [], newRanges: [], ...extra }
}

/** The text diff of two texts of these line counts with these blocks */
export function textDiff(
  oldLineCount: number,
  newLineCount: number,
  blocks: DiffBlock[],
  { oldScopes = [], newScopes = [], emptyLastLine = false, importBlocks = [] }: { oldScopes?: Scope[]; newScopes?: Scope[]; emptyLastLine?: boolean; importBlocks?: number[] } = {},
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
