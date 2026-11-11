// @spec openspec/specs/commit/commit-files/spec.md
import { describe, expect, it } from "vite-plus/test"
import type { MergeBlock } from "@/lib/diff/file-merge-model"
import { buildLineTable, lineText } from "@/lib/diff/line-table"
import {
  buildFileRows,
  buildMergeRows,
  EXPANDED,
  foldWords,
  getFoldLevels,
  getOpenNotes,
  LEFT,
  MIDDLE,
  NEW,
  nextFoldLevel,
  noteOfLine,
  OLD,
  RIGHT,
  rowAt,
  rowOfLine,
  setFoldLevels,
  setOpenNotes,
} from "@/lib/diff/rows"
import type { FileRows, FoldRow, Heading, LineRow, Row } from "@/lib/diff/rows"
import type { DiffViewMode } from "@/stores/settings/diff-view-settings"
import { block, mergeBlock, NESTED_SCOPES, run, textDiff, textMerge } from "@/test-support/diff-blocks"

/**
 * A row as text: `4:4` context, `-4` deleted, `+4` added, `~4:4` inline, `4|_` split, `=_:4` ignored, `1x4` a line
 * shown once, `fold 0-16`; a note its tone and its words, a remark of the marks its kind, the old lines it hides and
 * whether it opens (`remark formatting 3-4 closed`); a changed line not in the full tint ends with ` pale`,
 * ` whitespace` or ` quiet`
 */
function describeRow(row: Row): string {
  const line = (value: number | null | undefined) => (value === null || value === undefined ? "_" : String(value))
  const tint = row.kind === "fold" || row.kind === "note" || row.tint === "full" ? "" : ` ${row.tint}`
  switch (row.kind) {
    case "fold":
      return `fold ${row.lines[1]![0]}-${row.lines[1]![1]}${row.label === null ? "" : ` ${row.label}`}`
    case "note": {
      const { remark } = row
      const said = remark === null ? "" : ` ${remark.kind} ${row.lines[0]![0]}-${row.lines[0]![1]}${remark.open === null ? "" : remark.open ? " open" : " closed"}`
      return `${row.tone}${row.label === "" ? "" : ` ${row.label}`}${said}`
    }
    case "context":
      return row.lines.map(line).join(":")
    case "deleted":
      return `-${line(row.lines[0])}${tint}`
    case "added":
      return `+${line(row.lines[1])}${tint}`
    case "inline":
      return `~${line(row.lines[0])}:${line(row.lines[1])}`
    case "changed":
      return `${line(row.lines[0])}|${line(row.lines[1])}${tint}`
    case "once":
      return `1x${line(row.lines[1])}${tint}`
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
    expect(outline(buildFileRows(diff, "split"))).toEqual([...same(0, 3), "3|3 whitespace", ...same(4, 6), "=6:6", "=7:_", "fold 7-9"])
  })

  it("draws the lines of a block with inner ranges pale and the lines of a block without them in the full tint, as IntelliJ", () => {
    // a re-indent under Do Not Ignore: the engine marks the indentation as inner ranges; a line-level block has none
    const diff = textDiff(20, 20, [block(3, 4, 3, 4, { oldRanges: [[0, 2, 2]], newRanges: [[0, 4, 2]] }), block(12, 13, 12, 13)])
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

  it("counts the unchanged lines a fold hides, apart from the lines of the blocks the policy ignores there", () => {
    // an ignored deletion of a blank line (no row in Unified) and an ignored change of indentation, both folded
    const diff = textDiff(40, 39, [block(5, 6, 5, 5, { kind: "ignored" }), block(10, 11, 9, 10, { kind: "ignored" }), block(30, 31, 29, 30)])
    const fold = rowAt(buildFileRows(diff, "unified"), 0)
    expect(fold).toMatchObject({ kind: "fold", unchanged: 24, ignored: 2, elsewhere: [] })
    expect(foldWords(fold as FoldRow)).toEqual({ lines: "24 unchanged lines · 2 lines hidden by Whitespace", places: [] })
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

  it("plays down a block of the imports alone in a review: pale, and N and P pass it; elsewhere it is a change as any", () => {
    const diff = textDiff(40, 40, [block(2, 3, 2, 3), block(20, 21, 20, 21)], { importBlocks: [0] })
    const quiet = buildFileRows(diff, "unified", [EXPANDED, EXPANDED, EXPANDED], true)
    expect(quiet.changeStarts.map(start => describeRow(rowAt(quiet, start)))).toEqual(["-20"])
    // the first row of the block of the imports, after the two lines above it
    expect(describeRow(rowAt(quiet, 2))).toBe("-2 whitespace")
    const plain = buildFileRows(diff, "unified", [EXPANDED, EXPANDED, EXPANDED])
    expect(plain.changeStarts.map(start => describeRow(rowAt(plain, start)))).toEqual(["-2", "-20"])
  })

  it("makes the rows of a section of a review: a heading before each change it marks, one row for a block another section shows", () => {
    const diff = textDiff(40, 40, [block(2, 3, 2, 3), block(20, 21, 20, 21), block(30, 31, 30, 31)])
    const units = { marks: new Map([[1, [{ name: "Policy.check", words: "body changed" }]]]), elsewhere: new Map([[2, "Imports — in Imports (3 files)"]]) }
    const rows = buildFileRows(diff, "unified", [EXPANDED, EXPANDED, EXPANDED, EXPANDED], false, units)
    const all = outline(rows)
    const heading = all.indexOf("unit Policy.check — body changed")
    expect(all.slice(heading - 1, heading + 3)).toEqual(["19:19", "unit Policy.check — body changed", "-20", "+20"])
    expect(outline(rows)).toContain("elsewhere Imports — in Imports (3 files)")
    // N and P step to the changes of the section alone; a folded section keeps its heading next to its change
    expect(rows.changeStarts.map(start => describeRow(rowAt(rows, start)))).toEqual(["-2", "-20"])
    expect(outline(buildFileRows(diff, "unified", [], false, units)).filter(row => !/^\d/.test(row))).toEqual([
      "-2",
      "+2",
      "fold 7-16",
      "unit Policy.check — body changed",
      "-20",
      "+20",
      "fold 25-40",
    ])
  })

  it("names in a fold of a section the blocks another section shows that it hides, and counts only its unchanged lines", () => {
    // an import another part shows, in the fold before the change of this section: 62 unchanged lines on the old side
    // against 63 on the new one, the import no unchanged line
    const diff = textDiff(80, 81, [block(3, 3, 3, 4), block(70, 71, 71, 72)])
    const units = { marks: new Map<number, Heading>(), elsewhere: new Map([[0, "Shown in part 2"]]) }
    const fold = rowAt(buildFileRows(diff, "unified", [], false, units), 0)
    expect(fold).toMatchObject({
      kind: "fold",
      lines: [
        [0, 66],
        [0, 67],
      ],
      unchanged: 66,
      elsewhere: [{ block: 0, label: "Shown in part 2" }],
      ignored: 0,
    })
    expect(foldWords(fold as FoldRow)).toEqual({ lines: "66 unchanged lines", places: [{ block: 0, words: "1 change in part 2" }] })
    // a fold of the blocks of another section alone says only where they are, each place with its first block
    const only = { ...(fold as FoldRow), unchanged: 0, elsewhere: [3, 4].map(block => ({ block, label: "Shown in Other changes" })) }
    expect(foldWords(only)).toEqual({ lines: "", places: [{ block: 3, words: "2 changes in Other changes" }] })
  })

  it("makes a section of no change of its own the rows of the blocks other sections show, alone", () => {
    const diff = textDiff(40, 40, [block(5, 7, 5, 9)])
    const units = { marks: new Map<number, Heading>(), elsewhere: new Map([[0, "hooks — removed · shown in step 2"]]) }
    const rows = buildFileRows(diff, "unified", [], false, units)
    expect(outline(rows)).toEqual(["elsewhere hooks — removed · shown in step 2"])
    expect(rows.changeStarts).toEqual([])
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

describe("the marks of a diff in its rows", () => {
  /** Every row of a diff in a mode, nothing folded; `open`: the notes opened, by the first old line each hides */
  const whole = (diff: ReturnType<typeof textDiff>, mode: DiffViewMode, open: number[] = [], units: Parameters<typeof buildFileRows>[4] = null) =>
    buildFileRows(diff, mode, [EXPANDED, EXPANDED, EXPANDED], false, units, new Set(open))
  /** Only the rows of the blocks: no unchanged line */
  const changed = (rows: FileRows) => outline(rows).filter(row => !/^\d+:\d+$/.test(row))
  // a call of one line wrapped over three: the whole block is a quiet run of formatting, nothing of it to read
  const WRAPPED = textDiff(10, 12, [block(3, 4, 3, 6)], { runs: [run(3, 4, 3, 6)] })
  // three lines indented inside a new `if`: the `if` and its brace are read, the lines between them are quiet
  const INDENTED = textDiff(10, 12, [block(2, 5, 2, 7)], { runs: [run(2, 5, 3, 6)] })

  it("shows a quiet run of Inline once, by its new lines after a note, and with its old lines once the note is opened", () => {
    expect(outline(whole(WRAPPED, "inline"))).toEqual([
      ...same(0, 3),
      "remark formatting 3-4 closed",
      "1x3 quiet",
      "1x4 quiet",
      "1x5 quiet",
      "4:6",
      "5:7",
      "6:8",
      "7:9",
      "8:10",
      "9:11",
    ])
    // opened, the note stands for no line: the old line shows before the new ones, as in Unified, still played down
    expect(changed(whole(WRAPPED, "inline", [3]))).toEqual(["remark formatting 3-3 open", "-3 quiet", "+3 quiet", "+4 quiet", "+5 quiet"])
    // a note of another line opens nothing here
    expect(changed(whole(WRAPPED, "inline", [4]))).toEqual(["remark formatting 3-4 closed", "1x3 quiet", "1x4 quiet", "1x5 quiet"])
    const once = rowAt(whole(WRAPPED, "inline"), 4) as LineRow
    expect([once.kind, once.lines, once.tint, once.run]).toEqual(["once", [null, 3], "quiet", "formatting"])
  })

  it("keeps every line of a quiet run in Unified and Split, played down, after a note that opens nothing", () => {
    expect(changed(whole(WRAPPED, "unified"))).toEqual(["remark formatting 3-3", "-3 quiet", "+3 quiet", "+4 quiet", "+5 quiet"])
    // Split fills the shorter side of the run, as of any block
    expect(changed(whole(WRAPPED, "split"))).toEqual(["remark formatting 3-3", "3|3 quiet", "_|4 quiet", "_|5 quiet"])
    // an opened note of Inline changes nothing where no line is hidden
    expect(changed(whole(WRAPPED, "unified", [3]))).toEqual(changed(whole(WRAPPED, "unified")))
    expect([whole(WRAPPED, "unified").notes, whole(WRAPPED, "split").notes, whole(WRAPPED, "inline").notes]).toEqual([[], [], [[3, 4]]])
    // a note that opens has its row, opened too: what the key of the notes takes for the notes in view
    const two = textDiff(20, 24, [block(3, 4, 3, 6), block(10, 11, 12, 15)], { runs: [run(3, 4, 3, 6), run(10, 11, 12, 15)] })
    expect([whole(WRAPPED, "unified").noteRows, whole(two, "inline").noteRows, whole(two, "inline", [3]).noteRows, whole(two, "inline", [3]).notes]).toEqual([
      [],
      [3, 13],
      [3, 14],
      [
        [3, 4],
        [10, 11],
      ],
    ])
  })

  it("cuts a block at its runs in the order of both sides: the lines between them show as the block does, a run after its note", () => {
    expect(changed(whole(INDENTED, "inline"))).toEqual(["+2", "remark formatting 2-5 closed", "1x3 quiet", "1x4 quiet", "1x5 quiet", "+6"])
    expect(changed(whole(INDENTED, "unified"))).toEqual(["+2", "remark formatting 2-2", "-2 quiet", "-3 quiet", "-4 quiet", "+3 quiet", "+4 quiet", "+5 quiet", "+6"])
    expect(changed(whole(INDENTED, "split"))).toEqual(["_|2", "remark formatting 2-2", "2|3 quiet", "3|4 quiet", "4|5 quiet", "_|6"])
    // two runs of a block: each its note, the line between them a row of the block
    const two = textDiff(10, 10, [block(2, 7, 2, 7)], { runs: [run(2, 4, 2, 4), run(5, 7, 5, 7)] })
    expect(changed(whole(two, "inline"))).toEqual(["remark formatting 2-4 closed", "1x2 quiet", "1x3 quiet", "-4", "+4", "remark formatting 5-7 closed", "1x5 quiet", "1x6 quiet"])
    expect(changed(whole(two, "inline", [5]))).toEqual([
      "remark formatting 2-4 closed",
      "1x2 quiet",
      "1x3 quiet",
      "-4",
      "+4",
      "remark formatting 5-5 open",
      "-5 quiet",
      "-6 quiet",
      "+5 quiet",
      "+6 quiet",
    ])
  })

  it("labels a run of comments with a note and plays nothing of it down", () => {
    const comment = block(4, 5, 4, 5, { inlinePairs: true, oldRanges: [[3, 6, 2]], newRanges: [[3, 8, 2]] })
    const diff = textDiff(10, 10, [comment], { runs: [run(4, 5, 4, 5, { kind: "comments", quiet: false })] })
    // the lines of the run show as the block does: a similar pair in Inline, pale with their ranges
    expect(changed(whole(diff, "inline"))).toEqual(["remark comments 4-4", "~4:4"])
    expect(changed(whole(diff, "unified"))).toEqual(["remark comments 4-4", "-4 pale", "+4 pale"])
    expect(changed(whole(diff, "split"))).toEqual(["remark comments 4-4", "4|4 pale"])
    const pair = rowAt(whole(diff, "inline"), 5) as LineRow
    expect([pair.kind, pair.tint, pair.run]).toEqual(["inline", "pale", null])
    // nothing is hidden, so no layout has a note of it to open
    expect(whole(diff, "inline").notes).toEqual([])
  })

  it("pairs the lines of a part of Inline only where the engine pairs line i of both sides", () => {
    // the middle line of three pairs is a quiet run: the pairs around it stay rows of a pair
    const pairs = textDiff(10, 10, [block(2, 5, 2, 5, { inlinePairs: true })], { runs: [run(3, 4, 3, 4)] })
    expect(changed(whole(pairs, "inline"))).toEqual(["~2:2", "remark formatting 3-4 closed", "1x3 quiet", "~4:4"])
    // a run of one line against two moves the lines after it apart: they are no pairs of the engine
    const apart = textDiff(10, 11, [block(2, 5, 2, 6, { inlinePairs: true })], { runs: [run(2, 3, 2, 4)] })
    expect(changed(whole(apart, "inline"))).toEqual(["remark formatting 2-3 closed", "1x2 quiet", "1x3 quiet", "-3", "-4", "+4", "+5"])
  })

  it("stops N and P where Rust starts the reading of a block, at the first row of that part, never at a block of quiet lines alone", () => {
    const stops = (rows: FileRows) => rows.changeStarts.map(start => describeRow(rowAt(rows, start)))
    // the wrapped call has no start; a real change beside it has
    const diff = textDiff(20, 22, [block(3, 4, 3, 6), block(10, 11, 12, 13)], { runs: [run(3, 4, 3, 6)] })
    expect(diff.marks.starts).toEqual([1, 1, 12])
    for (const mode of ["inline", "unified", "split"] as const) {
      const rows = whole(diff, mode)
      expect([stops(rows), rows.changes], mode).toEqual([[mode === "split" ? "10|12" : "-10"], 2])
    }
    // the reading starts at the `if`, before the quiet lines; Rust names the new line, Unified shows the part by its first row
    expect(stops(whole(INDENTED, "inline"))).toEqual(["+2"])
    // a quiet run first, then a deleted line: the start is that old line
    const after = textDiff(10, 9, [block(2, 5, 2, 4)], { runs: [run(2, 4, 2, 4)] })
    expect(after.marks.starts).toEqual([0, 0, 4])
    expect(stops(whole(after, "inline"))).toEqual(["-4"])
    // a run that is a label is no cut: the reading starts at its lines, after its note
    const comment = textDiff(10, 10, [block(4, 5, 4, 5)], { runs: [run(4, 5, 4, 5, { kind: "comments", quiet: false })] })
    expect(stops(whole(comment, "unified"))).toEqual(["-4"])
    // six lines deleted, then a comment worded again, one block: Rust names the comment, its first new line, and a
    // view shows the deleted lines first, in a part of their own before the note. The reading starts at the first of
    // them in every layout, as it did before the block had a label
    const label = run(7, 8, 1, 2, { kind: "comments", quiet: false })
    const deletion = textDiff(20, 14, [block(1, 8, 1, 2)], { runs: [label] })
    expect(deletion.marks.starts).toEqual([0, 1, 1])
    for (const mode of ["inline", "unified", "split"] as const) {
      const rows = whole(deletion, mode)
      expect([stops(rows), rows.changeStarts], mode).toEqual([[mode === "split" ? "1|_" : "-1"], whole(textDiff(20, 14, [block(1, 8, 1, 2)]), mode).changeStarts])
      expect(changed(rows).slice(0, 8), mode).toEqual([
        ...[1, 2, 3, 4, 5, 6].map(line => (mode === "split" ? `${line}|_` : `-${line}`)),
        "remark comments 7-7",
        mode === "split" ? "7|1" : "-7",
      ])
    }
    // after a quiet run the reading starts anew: at the lines deleted after it, before the label that follows them
    const both = textDiff(20, 17, [block(1, 8, 1, 5)], { runs: [run(1, 2, 1, 3), run(7, 8, 4, 5, { kind: "comments", quiet: false })] })
    expect(both.marks.starts).toEqual([0, 1, 3])
    expect(changed(whole(both, "inline"))).toEqual([
      "remark formatting 1-2 closed",
      "1x1 quiet",
      "1x2 quiet",
      "-2",
      "-3",
      "-4",
      "-5",
      "-6",
      "+3",
      "remark comments 7-7",
      "-7",
      "+4",
    ])
    expect(["inline", "unified", "split"].map(mode => stops(whole(both, mode as DiffViewMode)))).toEqual([["-2"], ["-2"], ["2|3"]])
    // a start that names no line of the block (no engine gives one) stops at its last row, never past it
    const stray = { ...WRAPPED, marks: { ...WRAPPED.marks, starts: [0, 1, 40] } }
    expect(stops(whole(stray, "inline"))).toEqual(["1x5 quiet"])
  })

  it("says the words of a run that starts its block in the heading of the change, one row of words", () => {
    const units = { marks: new Map([[0, [{ name: "record", words: "body changed" }]]]), elsewhere: new Map<number, string>() }
    expect(changed(whole(WRAPPED, "inline", [], units))).toEqual(["unit record — body changed formatting 3-4 closed", "1x3 quiet", "1x4 quiet", "1x5 quiet"])
    expect(changed(whole(WRAPPED, "inline", [3], units))).toEqual(["unit record — body changed formatting 3-3 open", "-3 quiet", "+3 quiet", "+4 quiet", "+5 quiet"])
    // a run after the first lines of the block keeps its own note
    expect(changed(whole(INDENTED, "inline", [], units))).toEqual(["unit record — body changed", "+2", "remark formatting 2-5 closed", "1x3 quiet", "1x4 quiet", "1x5 quiet", "+6"])
    // the heading is a row of the block: N stops at the line after it
    const rows = whole(INDENTED, "inline", [], units)
    expect(rows.changeStarts.map(start => describeRow(rowAt(rows, start)))).toEqual(["+2"])
  })

  it("gives the note that stands for an old line shown once, and the row of the line once the note is open", () => {
    const closed = whole(WRAPPED, "inline")
    expect([rowOfLine(closed, OLD, 3), rowOfLine(closed, NEW, 3), rowOfLine(closed, NEW, 5), rowOfLine(closed, OLD, 4)]).toEqual([3, 4, 6, 7])
    expect([noteOfLine(closed, 3), noteOfLine(closed, 2), noteOfLine(closed, 4)]).toEqual([3, null, null])
    const open = whole(WRAPPED, "inline", [3])
    expect([rowOfLine(open, OLD, 3), rowOfLine(open, NEW, 3), noteOfLine(open, 3)]).toEqual([4, 5, 3])
    // where every line stays, an old line has its row and no note stands for it
    const unified = whole(WRAPPED, "unified")
    expect([rowOfLine(unified, OLD, 3), noteOfLine(unified, 3)]).toEqual([4, null])
    // the old lines of the indented run are behind its note, after the row of the `if`
    expect([2, 3, 4].map(line => rowOfLine(whole(INDENTED, "inline"), OLD, line))).toEqual([3, 3, 3])
  })

  it("leaves a run that lies in no block out, so every line of every block shows", () => {
    const stray = textDiff(10, 10, [block(4, 5, 4, 5)], { runs: [run(0, 1, 0, 1), run(4, 6, 4, 5), run(7, 8, 7, 8)] })
    expect(changed(whole(stray, "inline"))).toEqual(["-4", "+4"])
  })

  it("keeps the open notes of a diff beside the levels of its folds", () => {
    expect(getOpenNotes(WRAPPED).size).toBe(0)
    setOpenNotes(WRAPPED, new Set([3]))
    expect([...getOpenNotes(WRAPPED)]).toEqual([3])
    expect(getFoldLevels(WRAPPED)).toEqual([])
    setOpenNotes(WRAPPED, new Set())
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
