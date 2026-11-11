import type { TextDiff } from "@/lib/diff/file-diff-model"
import { inlineParts } from "@/lib/diff/line-parts"
import { columnsOf, lineText } from "@/lib/diff/line-table"
import type { LineTable } from "@/lib/diff/line-table"

/*
 * The text of the rows of Inline (docs/design/diff-view.md, "The row model"): a similar pair is one row, the new line
 * with the deleted parts of the old line in it, so the row is longer than either line.
 */

/** The text an Inline row shows: the new line with the deleted parts of the old line before the inserted parts (`inlineParts`) */
export function inlineRowText(oldLine: string, newLine: string, oldRanges: readonly number[], newRanges: readonly number[]): string {
  return inlineParts(oldLine.length, null, oldRanges, newLine.length, null, newRanges)
    .map(part => (part.side === 0 ? oldLine : newLine).slice(part.start, part.end))
    .join("")
}

/**
 * The columns of the widest row of a similar pair in Inline, a tab to its next stop; 0 for a diff without such a row.
 * The horizontal scroll of the one column reaches as far, past the widest line of both texts
 */
export function widestInlineRow(diff: TextDiff, oldTable: LineTable, newTable: LineTable, tabSize: number): number {
  const known = widestRows.get(diff)
  if (known !== undefined && known.oldTable === oldTable && known.newTable === newTable && known.tabSize === tabSize) {
    return known.columns
  }
  let widest = 0
  for (const block of diff.blocks) {
    const pairs = block.newEnd - block.newStart
    // the rows of `itemRow`: a pair per line of a block of similar pairs, which the policy does not ignore
    if (block.kind === "ignored" || !block.inlinePairs || block.oldEnd - block.oldStart !== pairs) {
      continue
    }
    for (let pair = 0; pair < pairs; pair++) {
      const oldRanges = block.oldRanges[pair] ?? []
      // a pair without a deleted part is its new line, which the widest line of the text counts
      if (oldRanges.length === 0 || block.oldStart + pair >= oldTable.lineCount || block.newStart + pair >= newTable.lineCount) {
        continue
      }
      const text = inlineRowText(lineText(oldTable, block.oldStart + pair), lineText(newTable, block.newStart + pair), oldRanges, block.newRanges[pair] ?? [])
      widest = Math.max(widest, columnsOf(text, tabSize))
    }
  }
  widestRows.set(diff, { oldTable, newTable, tabSize, columns: widest })
  return widest
}

// counted once per diff and its texts: the view asks at every layout
const widestRows = new WeakMap<TextDiff, { oldTable: LineTable; newTable: LineTable; tabSize: number; columns: number }>()
