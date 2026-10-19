// @spec spec/commit/commit-files.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { MergeBlock } from "@/lib/diff/file-merge-model"
import { buildLineTable, lineText } from "@/lib/diff/line-table"
import { buildFileRows, buildMergeRows, EXPANDED, getFoldLevels, LEFT, MIDDLE, nextFoldLevel, RIGHT, rowAt, rowOfLine, setFoldLevels } from "@/lib/diff/rows"
import type { FileRows, LineRow, Row } from "@/lib/diff/rows"
import type { DiffViewMode } from "@/stores/settings/diff-view-settings"
import { block, mergeBlock, NESTED_SCOPES, textDiff, textMerge } from "@/test-support/diff-blocks"

/**
 * A row as text: `4:4` context, `-4` deleted, `+4` added, `~4:4` inline, `4|_` split, `=_:4` ignored, `fold 0-16`; a
 * changed line not in the full tint ends with ` pale` or ` whitespace`
 */
function describeRow(row: Row): string {
  const line = (value: number | null | undefined) => (value === null || value === undefined ? "_" : String(value))
  const tint = row.kind === "fold" || row.tint === "full" ? "" : ` ${row.tint}`
  switch (row.kind) {
    case "fold":
      return `fold ${row.lines[1]![0]}-${row.lines[1]![1]}${row.label === null ? "" : ` ${row.label}`}`
    case "context":
      return row.lines.map(line).join(":")
    case "deleted":
      return `-${line(row.lines[0])}${tint}`
    case "added":
      return `+${line(row.lines[1])}${tint}`
    case "inline":
      return `~${line(row.lines[0])}:${line(row.lines[1])}`
    case "changed":
      return `${line(row.lines[0])}|${line(row.lines[1])}`
    case "ignored":
      return `=${row.lines.map(line).join(":")}`
    case "merge":
      return `${row.lines.map(line).join("|")}${tint}`
  }
}

function outline(rows: FileRows): string[] {
  return Array.from({ length: rows.rowCount }, (_, index) => describeRow(rowAt(rows, index)))
}

/** Unchanged rows `from..to` as `n:n` */
function same(from: number, to: number): string[] {
  return Array.from({ length: to - from }, (_, index) => `${from + index}:${from + index}`)
}

const ONE_CHANGE = textDiff(40, 40, [block(20, 21, 20, 21)])

describe("buildFileRows", () => {
  it("keeps 4 lines of context next to a change, as IntelliJ, and folds the others", () => {
    const rows = buildFileRows(ONE_CHANGE, "unified")
    expect(outline(rows)).toEqual(["fold 0-16", ...same(16, 20), "-20", "+20", ...same(21, 25), "fold 25-40"])
    expect(rows.changeStarts).toEqual([5])
    expect(outline(buildFileRows(ONE_CHANGE, "split"))).toEqual(["fold 0-16", ...same(16, 20), "20|20", ...same(21, 25), "fold 25-40"])
  })

  it("nests the folds at 4, 8 and 16 lines of context, then shows all lines", () => {
    const unfold = (levels: number[]) => outline(buildFileRows(ONE_CHANGE, "unified", levels)).filter(row => row.startsWith("fold"))
    expect(unfold([1])).toEqual(["fold 0-12", "fold 25-40"])
    expect(unfold([2, 1])).toEqual(["fold 0-4", "fold 29-40"])
    expect(unfold([EXPANDED, 2])).toEqual(["fold 37-40"])
    expect(unfold([EXPANDED, EXPANDED])).toEqual([])
    expect([0, 1, 2].map(level => nextFoldLevel(buildFileRows(ONE_CHANGE, "unified"), 0, level))).toEqual([1, 2, EXPANDED])
  })

  it("skips a level that shows no more lines, and folds no fewer than 2 lines", () => {
    // 10 lines before the change: level 0 hides 6, level 1 hides 2, level 2 hides none
    const diff = textDiff(12, 12, [block(10, 11, 10, 11)])
    expect(outline(buildFileRows(diff, "unified"))[0]).toBe("fold 0-6")
    expect(nextFoldLevel(buildFileRows(diff, "unified"), 0, 1)).toBe(2)
    // 9 lines between two changes leave one line beside their contexts: it shows
    expect(outline(buildFileRows(textDiff(30, 30, [block(10, 11, 10, 11), block(20, 21, 20, 21)]), "unified"))).toEqual([
      "fold 0-6",
      ...same(6, 10),
      "-10",
      "+10",
      ...same(11, 20),
      "-20",
      "+20",
      ...same(21, 25),
      "fold 25-30",
    ])
    // 10 lines between them fold their middle two
    const apart = outline(buildFileRows(textDiff(30, 30, [block(10, 11, 10, 11), block(21, 22, 21, 22)]), "unified"))
    expect(apart.slice(7, 17)).toEqual([...same(11, 15), "fold 15-17", ...same(17, 21), "-21"])
  })

  it("cuts the context at both ends of the file", () => {
    expect(outline(buildFileRows(textDiff(10, 10, [block(0, 1, 0, 1), block(9, 10, 9, 10)]), "unified"))).toEqual(["-0", "+0", ...same(1, 9), "-9", "+9"])
  })

  it("draws a whitespace-only block in the whitespace tint, and a block the policy ignores as unchanged lines inside the context", () => {
    // line 3 lost its indentation, lines 6-7 became one blank line
    const diff = textDiff(10, 9, [block(3, 4, 3, 4, { kind: "whitespaceOnly" }), block(6, 8, 6, 7, { kind: "ignored" })])
    expect(outline(buildFileRows(diff, "unified"))).toEqual([...same(0, 3), "-3 whitespace", "+3 whitespace", ...same(4, 6), "=_:6", "8:7", "9:8"])
    // Split shows both sides of the ignored block, so its gap has one more row, and its last two fold
    expect(outline(buildFileRows(diff, "split"))).toEqual([...same(0, 3), "3|3", ...same(4, 6), "=6:6", "=7:_", "fold 7-9"])
  })

  it("draws the lines of a block with inner ranges pale and the lines of a block without them in the full tint, as IntelliJ", () => {
    // a re-indent under Do Not Ignore: the engine marks the indentation as inner ranges; a line-level block has none
    const diff = textDiff(20, 20, [block(3, 4, 3, 4, { oldRanges: [[0, 2]], newRanges: [[0, 4]] }), block(12, 13, 12, 13)])
    const rows = outline(buildFileRows(diff, "unified", [EXPANDED, EXPANDED, EXPANDED]))
    expect(rows[3]).toBe("-3 pale")
    expect(rows[4]).toBe("+3 pale")
    expect(rows).toContain("-12")
    expect(rows).toContain("+12")
  })

  it("leaves out the empty line after a final line end, unless a block touches it", () => {
    // 8 lines and a final line end: the engine counts 9 lines, the last one empty
    const expanded = buildFileRows(textDiff(9, 9, [block(3, 4, 3, 4)], { emptyLastLine: true }), "split", [EXPANDED, EXPANDED])
    expect(outline(expanded).at(-1)).toBe("7:7")
    expect(buildFileRows(textDiff(30, 30, [block(3, 4, 3, 4)], { emptyLastLine: true }), "split").rowCount).toBe(3 + 1 + 4 + 1)
    expect(rowAt(buildFileRows(textDiff(30, 30, [block(3, 4, 3, 4)], { emptyLastLine: true }), "split"), 8)).toMatchObject({
      kind: "fold",
      lines: [
        [8, 29],
        [8, 29],
      ],
    })
    // "a" to "a\n": the new final line end is a block of the empty last line, which shows
    const added = { ...textDiff(1, 2, [block(1, 1, 1, 2, { kind: "whitespaceOnly" })]), emptyLastLine: { old: false, new: true } }
    expect(outline(buildFileRows(added, "unified"))).toEqual(["0:0", "+1 whitespace"])
  })

  it("shows no rows for a file whose only blocks the policy ignores", () => {
    expect(buildFileRows(textDiff(20, 19, [block(10, 12, 10, 11, { kind: "ignored" })]), "unified").rowCount).toBe(0)
    expect(buildFileRows(textDiff(20, 20, [block(10, 11, 10, 11, { kind: "ignored" })]), "split").rowCount).toBe(0)
  })

  it("folds the lines an ignored block holds, with their own count on each side", () => {
    const diff = textDiff(40, 39, [block(5, 7, 5, 6, { kind: "ignored" }), block(30, 31, 29, 30)])
    const fold = rowAt(buildFileRows(diff, "unified"), 0)
    expect(fold).toMatchObject({
      kind: "fold",
      lines: [
        [0, 26],
        [0, 25],
      ],
    })
  })

  it("makes each similar pair of a block one row only in the Inline view, also at the end of a file", () => {
    const pairs = [block(2, 3, 2, 3, { inlinePairs: true })]
    expect(outline(buildFileRows(textDiff(5, 5, pairs), "inline"))).toEqual(["0:0", "1:1", "~2:2", "3:3", "4:4"])
    expect(outline(buildFileRows(textDiff(5, 5, pairs), "unified"))).toEqual(["0:0", "1:1", "-2", "+2", "3:3", "4:4"])
    expect(outline(buildFileRows(textDiff(2, 2, [block(1, 2, 1, 2, { inlinePairs: true })]), "inline"))).toEqual(["0:0", "~1:1"])
    // a block of unequal sides keeps two rows per pair
    expect(outline(buildFileRows(textDiff(5, 4, [block(1, 3, 1, 2, { inlinePairs: true })]), "inline"))).toEqual(["0:0", "-1", "-2", "+1", "3:2", "4:3"])
  })

  it("fills the shorter side of a block in the split view", () => {
    expect(outline(buildFileRows(textDiff(4, 3, [block(1, 3, 1, 2)]), "split"))).toEqual(["0:0", "1|1", "2|_", "3:2"])
  })

  it("shows a new file and a deleted file as one block of one side", () => {
    const modes: DiffViewMode[] = ["unified", "inline", "split"]
    expect(modes.map(mode => outline(buildFileRows(textDiff(0, 3, [block(0, 0, 0, 3)]), mode)))).toEqual([
      ["+0", "+1", "+2"],
      ["+0", "+1", "+2"],
      ["_|0", "_|1", "_|2"],
    ])
    expect(outline(buildFileRows(textDiff(2, 0, [block(0, 2, 0, 0)]), "unified"))).toEqual(["-0", "-1"])
  })

  it("labels a fold with the innermost scope around its lines, the new side first", () => {
    const scopes = NESTED_SCOPES
    const labels = (newScopes: typeof scopes, oldScopes: typeof scopes = []) =>
      outline(buildFileRows(textDiff(40, 40, [block(20, 21, 20, 21)], { newScopes, oldScopes }), "unified")).filter(row => row.startsWith("fold"))
    expect(labels(scopes)).toEqual(["fold 0-16 class A", "fold 25-40 class A › fun write"])
    // a scope around the last folded line when none holds them all
    expect(labels(scopes.slice(1))).toEqual(["fold 0-16 class A › fun read", "fold 25-40 class A › fun write"])
    expect(labels([], scopes.slice(0, 1))).toEqual(["fold 0-16 class A", "fold 25-40 class A"])
    expect(labels([])).toEqual(["fold 0-16", "fold 25-40"])
  })

  it("starts N and P at each block of changed lines, a deleted one too, never at a block the policy ignores", () => {
    const diff = textDiff(40, 39, [block(5, 6, 5, 6), block(10, 12, 10, 11, { kind: "ignored" }), block(30, 31, 29, 29)])
    const rows = buildFileRows(diff, "unified", [EXPANDED, EXPANDED, EXPANDED])
    expect(rows.changeStarts.map(start => describeRow(rowAt(rows, start)))).toEqual(["-5", "-30"])
  })

  it("finds each row from its segment, and keeps the fold levels of a diff object", () => {
    const rows = buildFileRows(ONE_CHANGE, "unified")
    expect(rows.segmentStarts.at(-1)).toBe(rows.rowCount)
    expect([rowAt(rows, 0).kind, rowAt(rows, 6).kind, rowAt(rows, rows.rowCount - 1).kind]).toEqual(["fold", "added", "fold"])
    expect(getFoldLevels(ONE_CHANGE)).toEqual([])
    setFoldLevels(ONE_CHANGE, [EXPANDED])
    expect(getFoldLevels(ONE_CHANGE)).toEqual([EXPANDED])
    expect(getFoldLevels(textDiff(40, 40, [block(20, 21, 20, 21)]))).toEqual([])
  })
})

describe("buildMergeRows", () => {
  // host; the limits line, which both sides changed and whose words resolve; port and name; a line the commit added
  const RESOLVED = mergeBlock([1, 2], [1, 2], [1, 2], { resolution: "text", resolvedText: "export const limits = { timeout: 10, retries: 3 }" })
  const ADDED = mergeBlock([4, 4], [4, 4], [4, 5], { kind: "inserted", leftChanged: false, resolution: "default" })
  const MERGE = textMerge([5, 5, 6], [RESOLVED, ADDED], { emptyLastLine: { left: true, base: true, right: true } })

  it("aligns the blocks of a merge in three columns, the middle the result, without the empty last line", () => {
    expect(outline(buildMergeRows(MERGE, "result"))).toEqual(["0:0:0", "1|1|1", "2:2:2", "3:3:3", "_|4|4"])
    // the base has no line where the commit added one
    expect(outline(buildMergeRows(MERGE, "base"))).toEqual(["0:0:0", "1|1|1", "2:2:2", "3:3:3", "_|_|4"])
    expect(buildMergeRows(MERGE, "result").changeStarts).toEqual([1, 4])
  })

  it("counts the lines of a resolve by words in the middle, and moves the middle lines after it", () => {
    const merge = textMerge([4, 4, 4], [mergeBlock([1, 2], [1, 2], [1, 2], { resolution: "text", resolvedText: "a\nb" })])
    const rows = buildMergeRows(merge, "result")
    expect(outline(rows)).toEqual(["0:0:0", "1|1|1", "_|2|_", "2:3:2", "3:4:3"])
    expect(rowOfLine(rows, MIDDLE, 3)).toBe(3)
    expect(rowOfLine(rows, LEFT, 3)).toBe(4)
    // a conflict that does not resolve keeps the base; a side that changed alone gives its lines
    expect(outline(buildMergeRows(textMerge([4, 4, 3], [mergeBlock([1, 2], [1, 2], [1, 1])]), "result"))).toEqual(["0:0:0", "1|1|_", "2:2:1", "3:3:2"])
    const deleted = mergeBlock([1, 2], [1, 2], [1, 1], { kind: "deleted", leftChanged: false, resolution: "default" })
    expect(outline(buildMergeRows(textMerge([4, 4, 3], [deleted]), "result"))).toEqual(["0:0:0", "1|_|_", "2:1:1", "3:2:2"])
  })

  it("folds the equal lines of a merge, labelled by the scopes of the left side under the result", () => {
    const merge = textMerge([40, 40, 40], [mergeBlock([20, 21], [20, 21], [20, 21])], { leftScopes: NESTED_SCOPES })
    const rows = buildMergeRows(merge, "result")
    expect(rowAt(rows, 0)).toMatchObject({
      kind: "fold",
      lines: [
        [0, 16],
        [0, 16],
        [0, 16],
      ],
      label: "class A",
    })
    expect(outline(rows).slice(1, 7)).toEqual(["16:16:16", "17:17:17", "18:18:18", "19:19:19", "20|20|20", "21:21:21"])
    expect(nextFoldLevel(rows, 0, 0)).toBe(1)
  })

  it("draws a block the policy ignores pale, and keeps it a change that N and P stop at, as IntelliJ's merge viewer", () => {
    const ignored = mergeBlock([1, 2], [1, 2], [1, 2], { kind: "modified", rightChanged: false, ignored: true, resolution: "default" })
    const merge = textMerge([10, 10, 10], [ignored, mergeBlock([8, 9], [8, 9], [8, 9])])
    const rows = buildMergeRows(merge, "result", [EXPANDED, EXPANDED, EXPANDED])
    expect(outline(rows)).toEqual(["0:0:0", "1|1|1 pale", "2:2:2", "3:3:3", "4:4:4", "5:5:5", "6:6:6", "7:7:7", "8|8|8", "9:9:9"])
    expect(rows.changeStarts).toEqual([1, 8])
    expect(rowAt(rows, 8)).toMatchObject({ kind: "merge", block: 1, blockRow: 0, lines: [8, 8, 8] })
  })

  it("tints the commit_whitespace merge by IntelliJ's rule under each policy", () => {
    // base = left = the old Format.kt, right = the new one: the indentation (lines 2-3), the blank line (5), `4` (6)
    const indentation = (ranges: number[][]) =>
      mergeBlock([2, 4], [2, 4], [2, 4], { kind: "modified", leftChanged: false, resolution: "default", baseRanges: ranges, rightRanges: ranges })
    const blank = (ignored: boolean) => mergeBlock([5, 5], [5, 5], [5, 6], { kind: "inserted", leftChanged: false, resolution: "default", ignored })
    const four = mergeBlock([6, 7], [6, 7], [7, 8], { kind: "modified", leftChanged: false, resolution: "default", baseRanges: [[17, 18]], rightRanges: [[17, 18]] })
    const tints = (blocks: MergeBlock[]) => {
      const rows = buildMergeRows(textMerge([9, 9, 10], blocks, { emptyLastLine: { left: true, base: true, right: true } }), "result", [EXPANDED, EXPANDED, EXPANDED, EXPANDED])
      return rows.changeStarts.map(start => (rowAt(rows, start) as LineRow).tint)
    }
    // Default: the indentation by its words, the blank line in the full colour, `4` by its word
    expect(
      tints([
        indentation([
          [0, 2],
          [0, 2],
        ]),
        blank(false),
        four,
      ]),
    ).toEqual(["pale", "full", "pale"])
    // Trim: the indentation pale without words
    expect(tints([indentation([[], []]), blank(false), four])).toEqual(["pale", "full", "pale"])
    // Ignore and Ignore Blank Lines: the blank line ignored, pale, still a change
    expect(tints([indentation([[], []]), blank(true), four])).toEqual(["pale", "pale", "pale"])
  })

  it("draws the lines of a merge block with inner ranges pale", () => {
    const merge = textMerge([3, 3, 3], [mergeBlock([1, 2], [1, 2], [1, 2], { leftRanges: [[0, 2]], baseRanges: [[0, 2]], rightRanges: [[4, 5]] })])
    expect(outline(buildMergeRows(merge, "base"))).toEqual(["0:0:0", "1|1|1 pale", "2:2:2"])
    expect(RIGHT).toBe(2)
  })
})

describe("buildLineTable", () => {
  it("splits lines as IntelliJ: LF, CRLF and a lone CR end a line, and a text may end without one", () => {
    const table = buildLineTable("a\r\nb\rc\nd")
    expect(Array.from({ length: table.lineCount }, (_, index) => lineText(table, index))).toEqual(["a", "b", "c", "d"])
    expect(table.endsWithLineEnd).toBe(false)
    // the line indices of the engine: a final line end, and an empty text, end with an empty line
    expect([buildLineTable("a\n").lineCount, buildLineTable("a\n").endsWithLineEnd, lineText(buildLineTable("a\n"), 1)]).toEqual([2, true, ""])
    expect([buildLineTable("").lineCount, buildLineTable("\n\n").lineCount, lineText(buildLineTable("\r\n\r\n"), 1)]).toEqual([1, 3, ""])
  })
})
