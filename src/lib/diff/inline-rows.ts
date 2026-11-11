import { drawnMarks, inlinePairsOf, marksByBlock, NO_MARKS } from "@/lib/diff/block-parts"
import type { TextDiff } from "@/lib/diff/file-diff-model"
import { inlinePair, inlineParts, RANGE_STRIDE } from "@/lib/diff/line-parts"
import { columnsOf, lineText } from "@/lib/diff/line-table"
import type { LineTable } from "@/lib/diff/line-table"
import { pairRanges } from "@/lib/diff/rename-sites"

/*
 * The text of the rows of Inline (docs/design/diff-view.md, "The row model"): a similar pair is one row, the new line
 * with the deleted parts of the old line in it, so the row is longer than either line. An edited line of a move is
 * such a row where the line stands now, of its old line where it left and its new one. A pair with a site of a rename
 * shows the whole old name before the whole new one (rename-sites.ts).
 */

/** The text an Inline row shows: the new line with the deleted parts of the old line before the inserted parts (`inlineParts`) */
export function inlineRowText(oldLine: string, newLine: string, oldRanges: readonly number[], newRanges: readonly number[]): string {
  return inlineParts(inlinePair(oldLine, newLine, oldRanges, newRanges), null, null)
    .map(part => (part.side === 0 ? oldLine : newLine).slice(part.start, part.end))
    .join("")
}

/**
 * The columns of the widest row of a similar pair in Inline, a tab to its next stop; 0 for a diff without such a row.
 * The horizontal scroll of the one column reaches as far, past the widest line of both texts. Counted once for a diff,
 * whatever notes are open and whether or not its view plays the imports down (a review with its plan, which draws no
 * move with an end in a block of the imports alone): where the two readings of a block differ, the pairs of both count
 */
export function widestInlineRow(diff: TextDiff, oldTable: LineTable, newTable: LineTable, tabSize: number): number {
  const known = widestRows.get(diff)
  if (known !== undefined && known.oldTable === oldTable && known.newTable === newTable && known.tabSize === tabSize) {
    return known.columns
  }
  let widest = 0
  const marks = marksByBlock(diff)
  const imports: ReadonlySet<number> = new Set(diff.importBlocks)
  diff.blocks.forEach((block, index) => {
    // the rows of a pair that the parts of the block make (`blockParts`): none of a block the policy ignores, of a
    // block the engine does not pair and no edited line of a move stands in, nor of the lines of a quiet run, which
    // show once
    const all = marks[index] ?? NO_MARKS
    const pairs = inlinePairsOf(block, index, all)
    // the block as a review with its plan draws it: a move it leaves out cuts no pair, and its edited line is no pair
    const played = drawnMarks(all, index, imports)
    if (played !== all) {
      pairs.push(...inlinePairsOf(block, index, played))
    }
    for (const { lines, oldRanges: engineOld, newRanges: engineNew } of pairs) {
      const [oldLine, newLine] = lines
      // the row strikes the whole old name of a rename through before the new one, whatever letters the engine marked
      const [oldRanges, newRanges] = pairRanges(diff, lines, engineOld, engineNew)
      // a pair without a deleted part (the old line has none, or the markers of its empty sides alone) is its new
      // line, which the widest line of the text counts
      const deletes = oldRanges.length !== newRanges.length || oldRanges.some((start, at) => at % RANGE_STRIDE === 0 && start < oldRanges[at + 1]!)
      if (!deletes || oldLine >= oldTable.lineCount || newLine >= newTable.lineCount) {
        continue
      }
      const text = inlineRowText(lineText(oldTable, oldLine), lineText(newTable, newLine), oldRanges, newRanges)
      widest = Math.max(widest, columnsOf(text, tabSize))
    }
  })
  widestRows.set(diff, { oldTable, newTable, tabSize, columns: widest })
  return widest
}

// counted once per diff and its texts: the view asks at every layout
const widestRows = new WeakMap<TextDiff, { oldTable: LineTable; newTable: LineTable; tabSize: number; columns: number }>()
