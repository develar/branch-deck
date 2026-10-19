import type { FileMergeHeader, MergeBlock, MergeContent, MergeSource, TextMerge } from "@/lib/bindings"
import { lineText } from "@/lib/diff/line-table"
import type { LineTable } from "@/lib/diff/line-table"

/*
 * The merge of a conflicting file as the backend streams it (`get_file_merges`, the Specta bindings): the blocks of the
 * three-way merge of the engine, the texts apart by the blob ids of the stages (docs/design/diff-view.md, "The 3-way
 * view").
 */

export type { FileMergeHeader, GitConflictKind, MergeBlock, MergeBlockKind, MergeContent, MergeResolution, MergeSide, MergeSource, TextMerge } from "@/lib/bindings"

/** A conflicting file with its content, once the stream gave it */
export interface FileMerge extends FileMergeHeader {
  content: MergeContent
}

/**
 * What the middle column of a merge shows: the result the engine proposes (each block resolved where it can be, the
 * base where it cannot), or the base
 */
export type MergeMiddle = "result" | "base"

/** Where the result of a block takes its lines from: a side, or the text of its resolve by words */
export type ResultSource = "left" | "base" | "right" | "text"

/**
 * The lines the result of a block takes. A resolved block takes the side that changed it, or the text of its resolve
 * by words; a conflict that does not resolve keeps the base.
 */
export function resultSourceOf(block: MergeBlock): ResultSource {
  if (block.resolution === "text" && block.resolvedText !== null) {
    return "text"
  }
  if (block.resolution === null && block.leftChanged && block.rightChanged) {
    return "base"
  }
  // a side changed alone, or both changed the same way
  return block.leftChanged ? "left" : block.rightChanged ? "right" : "base"
}

/** The lines of a resolve by words: `resolvedText` has no end after its last line */
export function resolvedLines(text: string): string[] {
  return text === "" ? [] : text.split(/\r\n|\r|\n/)
}

/** The lines of a block in the middle column */
export function middleLineCount(block: MergeBlock, middle: MergeMiddle): number {
  if (middle === "base") {
    return block.baseEnd - block.baseStart
  }
  switch (resultSourceOf(block)) {
    case "left":
      return block.leftEnd - block.leftStart
    case "right":
      return block.rightEnd - block.rightStart
    case "base":
      return block.baseEnd - block.baseStart
    case "text":
      return resolvedLines(block.resolvedText!).length
  }
}

/**
 * The text of the result: the equal lines from the base, each block as `resultSourceOf` says. Its line indices are
 * the ones of the middle column of the rows (`buildMergeRows`).
 */
export function composeResult(merge: TextMerge, [left, base, right]: readonly [LineTable, LineTable, LineTable]): string {
  const lines: string[] = []
  const push = (table: LineTable, start: number, end: number) => {
    for (let line = start; line < end; line++) {
      lines.push(lineText(table, line))
    }
  }
  let baseLine = 0
  for (const block of merge.blocks) {
    push(base, baseLine, block.baseStart)
    switch (resultSourceOf(block)) {
      case "left":
        push(left, block.leftStart, block.leftEnd)
        break
      case "right":
        push(right, block.rightStart, block.rightEnd)
        break
      case "base":
        push(base, block.baseStart, block.baseEnd)
        break
      case "text":
        lines.push(...resolvedLines(block.resolvedText!))
        break
    }
    baseLine = block.baseEnd
  }
  push(base, baseLine, merge.baseLineCount)
  return lines.join("\n")
}

const resultIds = new WeakMap<TextMerge, string>()
let lastResultId = 0

/**
 * The id of the result text of a merge in the blob texts store: one per merge object, so a merge read again (another
 * policy, a new stream) composes its own. Never a git blob id.
 */
export function resultTextId(merge: TextMerge): string {
  let id = resultIds.get(merge)
  if (id === undefined) {
    lastResultId += 1
    id = `merge-result-${lastResultId}`
    resultIds.set(merge, id)
  }
  return id
}

/** The id of a merge source in the stores; the merges of a resolved copy also name its blobs */
export function mergeSourceId(source: MergeSource): string {
  return `${source.parent}\0${source.target}\0${source.commit}\0${source.resolved ?? ""}`
}
