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
  notesOfRows,
  OLD,
  RIGHT,
  rowAt,
  rowOfLine,
  rowOfPlace,
  setFoldLevels,
  setOpenNotes,
} from "@/lib/diff/rows"
import type { FileRows, FoldRow, Heading, LineRow, Row } from "@/lib/diff/rows"
import type { DiffViewMode } from "@/stores/settings/diff-view-settings"
import { block, fileMove, mergeBlock, move, NESTED_SCOPES, rename, run, startsOf, subjectMarks, textDiff, textMerge } from "@/test-support/diff-blocks"

/**
 * A row as text: `4:4` context, `-4` deleted, `+4` added, `~4:4` inline, `4|_` split, `=_:4` ignored, `1x4` a line
 * shown once, `fold 0-16`; a note its tone and its words, a remark of the marks its kind, the old lines it hides and
 * whether it opens (`remark formatting 3-4 closed`), the note at an end of a move its end too (`remark moved 20-24
 * closed source`); a changed line not in the full tint ends with ` pale`, ` whitespace` or ` quiet`, a line of a move
 * with ` away` where it left and ` here` where it stands now
 */
function describeRow(row: Row): string {
  const line = (value: number | null | undefined) => (value === null || value === undefined ? "_" : String(value))
  const moved = row.kind === "fold" || row.kind === "note" || row.move === null ? "" : row.move.at === "source" ? " away" : " here"
  const tint = row.kind === "fold" || row.kind === "note" ? "" : `${row.tint === "full" ? "" : ` ${row.tint}`}${moved}`
  switch (row.kind) {
    case "fold":
      return `fold ${row.lines[1]![0]}-${row.lines[1]![1]}${row.label === null ? "" : ` ${row.label}`}`
    case "note": {
      const { remark } = row
      const end = remark?.move == null ? "" : ` ${remark.move.at}`
      const said = remark === null ? "" : ` ${remark.kind} ${row.lines[0]![0]}-${row.lines[0]![1]}${remark.open === null ? "" : remark.open ? " open" : " closed"}${end}`
      return `${row.tone}${row.label === "" ? "" : ` ${row.label}`}${said}`
    }
    case "context":
      return row.lines.map(line).join(":")
    case "deleted":
      return `-${line(row.lines[0])}${tint}`
    case "added":
      return `+${line(row.lines[1])}${tint}`
    case "inline":
      return `~${line(row.lines[0])}:${line(row.lines[1])}${moved}`
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

/** Every row of a diff in a mode, nothing folded; `open`: the notes opened, by the first old line each hides */
const whole = (diff: ReturnType<typeof textDiff>, mode: DiffViewMode, open: number[] = [], units: Parameters<typeof buildFileRows>[4] = null) =>
  buildFileRows(diff, mode, [EXPANDED, EXPANDED, EXPANDED], false, units, new Set(open))
/** Only the rows of the blocks: no unchanged line */
const changed = (rows: FileRows) => outline(rows).filter(row => !/^\d+:\d+$/.test(row))

describe("the marks of a diff in its rows", () => {
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
    expect([whole(WRAPPED, "unified").notes, whole(WRAPPED, "split").notes, whole(WRAPPED, "inline").notes]).toEqual([[], [], [{ key: 3, lines: [[3, 4]] }]])
    // a note that opens has its row, opened too: what the key of the notes takes for the notes in view
    const two = textDiff(20, 24, [block(3, 4, 3, 6), block(10, 11, 12, 15)], { runs: [run(3, 4, 3, 6), run(10, 11, 12, 15)] })
    expect([whole(WRAPPED, "unified").noteRows, whole(two, "inline").noteRows, whole(two, "inline", [3]).noteRows, whole(two, "inline", [3]).notes]).toEqual([
      [],
      [3, 13],
      [3, 14],
      [
        { key: 3, lines: [[3, 4]] },
        { key: 10, lines: [[10, 11]] },
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

  it("finds an old line of a block the policy ignores at the row that shows its new line, and at the first row of the block where the sides differ in their lines", () => {
    const levels = [EXPANDED, EXPANDED, EXPANDED]
    // as many lines on both sides: the lines are unchanged rows, each old line at the row of its own line
    const same = buildFileRows(textDiff(20, 20, [block(5, 8, 5, 8, { kind: "ignored" }), block(15, 16, 15, 16)]), "unified", levels)
    expect([5, 6, 7, 8].map(line => rowOfLine(same, OLD, line))).toEqual([5, 6, 7, 8])
    // a line fewer on the new side: one column shows the two new lines, and the first of them stands for the old ones
    const fewer = buildFileRows(textDiff(20, 19, [block(5, 8, 5, 7, { kind: "ignored" }), block(15, 16, 14, 15)]), "unified", levels)
    expect(outline(fewer).slice(4, 8)).toEqual(["4:4", "=_:5", "=_:6", "8:7"])
    expect([5, 6, 7, 8].map(line => rowOfLine(fewer, OLD, line))).toEqual([5, 5, 5, 7])
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

describe("the moves of a diff in its rows", () => {
  // four lines leave the old lines 20..24 and stand at the new lines 5..9, as they were
  const MOVED = textDiff(40, 40, [block(5, 5, 5, 9), block(20, 24, 24, 24)], { moves: [move(20, 5, 4)] })
  // indented two columns, the third line edited on the way
  const EDITED = textDiff(40, 40, [block(5, 5, 5, 9), block(20, 24, 24, 24)], {
    moves: [move(20, 5, 4, { indent: 2, edited: [2], oldRanges: [[], [], [4, 9, 2], []], newRanges: [[], [], [6, 12, 2], []] })],
  })

  it("shows the lines of a move once in Inline, where they stand now, and one note that opens where they left", () => {
    expect(outline(whole(MOVED, "inline"))).toEqual([
      ...same(0, 5),
      "remark moved 5-5 destination",
      "+5 here",
      "+6 here",
      "+7 here",
      "+8 here",
      ...Array.from({ length: 15 }, (_, at) => `${5 + at}:${9 + at}`),
      "remark moved 20-24 closed source",
      ...same(24, 40),
    ])
    // opened, the lines that left show after their note, as in Unified; where they stand now nothing changes
    expect(changed(whole(MOVED, "inline", [20]))).toEqual([
      "remark moved 5-5 destination",
      "+5 here",
      "+6 here",
      "+7 here",
      "+8 here",
      "remark moved 20-20 open source",
      "-20 away",
      "-21 away",
      "-22 away",
      "-23 away",
    ])
    // an edited line is the row of its two lines, pale as a line with inner ranges
    expect(changed(whole(EDITED, "inline")).slice(0, 5)).toEqual(["remark moved 5-5 destination", "+5 here", "+6 here", "~22:7 here", "+8 here"])
    const pair = rowAt(whole(EDITED, "inline"), 8) as LineRow
    expect([pair.kind, pair.lines, pair.tint, pair.move?.at, pair.move?.move]).toEqual(["inline", [22, 7], "pale", "destination", EDITED.marks.moves[0]])
  })

  it("keeps every line of a move in Unified and Split, both ends rows of the move after a note that opens nothing", () => {
    expect(changed(whole(EDITED, "unified"))).toEqual([
      "remark moved 5-5 destination",
      "+5 here",
      "+6 here",
      "+7 pale here",
      "+8 here",
      "remark moved 20-20 source",
      "-20 away",
      "-21 away",
      "-22 pale away",
      "-23 away",
    ])
    expect(changed(whole(EDITED, "split"))).toEqual([
      "remark moved 5-5 destination",
      "_|5 here",
      "_|6 here",
      "_|7 pale here",
      "_|8 here",
      "remark moved 20-20 source",
      "20|_ away",
      "21|_ away",
      "22|_ pale away",
      "23|_ away",
    ])
    // an opened note of Inline changes nothing where no line is hidden
    expect(changed(whole(MOVED, "unified", [20]))).toEqual(changed(whole(MOVED, "unified")))
  })

  it("stops N and P where the lines stand now and never where they left, and lists the notes of both ends", () => {
    for (const mode of ["inline", "unified", "split"] as const) {
      const rows = whole(MOVED, mode)
      // one stop, the first line of the move under its note; both blocks are changes of the file
      expect([rows.changeStarts.map(start => describeRow(rowAt(rows, start))), rows.changes], mode).toEqual([[mode === "split" ? "_|5 here" : "+5 here"], 2])
      expect(
        rows.moveNotes.map(note => [note.end.at, describeRow(rowAt(rows, note.row))]),
        mode,
      ).toEqual([
        ["destination", "remark moved 5-5 destination"],
        ["source", mode === "inline" ? "remark moved 20-24 closed source" : "remark moved 20-20 source"],
      ])
    }
    // only the note of Inline where the lines left opens: the key of the notes and ⌥-click take it
    expect([whole(MOVED, "inline").notes, whole(MOVED, "inline").noteRows, whole(MOVED, "unified").notes]).toEqual([[{ key: 20, lines: [[20, 24]] }], [25], []])
    // a block that lost some of its lines is read from a line that stayed
    const mixed = textDiff(50, 48, [block(10, 16, 10, 11), block(35, 35, 30, 33)], { moves: [move(11, 30, 3)] })
    const rows = whole(mixed, "inline")
    expect(changed(rows)).toEqual(["-10", "+10", "remark moved 11-14 closed source", "-14", "-15", "remark moved 35-35 destination", "+30 here", "+31 here", "+32 here"])
    expect(rows.changeStarts.map(start => describeRow(rowAt(rows, start)))).toEqual(["-10", "+30 here"])
    // a line after the note of the lines that left is its own row for a jump, never that note: the note stands
    // before other lines than the one asked
    expect([rowOfPlace(rows, OLD, 14), describeRow(rowAt(rows, rowOfPlace(rows, OLD, 14))), describeRow(rowAt(rows, rowOfPlace(rows, OLD, 11)))]).toEqual([
      rowOfLine(rows, OLD, 14),
      "-14",
      "remark moved 11-14 closed source",
    ])
  })

  it("finds an old line of a move at the note that stands for it, and lands a jump to an end on its note", () => {
    const closed = whole(MOVED, "inline")
    // the lines that left are behind their note; the new lines have their rows
    expect([20, 21, 23].map(line => rowOfLine(closed, OLD, line))).toEqual([25, 25, 25])
    expect([noteOfLine(closed, 21), noteOfLine(closed, 24), rowOfLine(closed, NEW, 5), rowOfLine(closed, NEW, 8)]).toEqual([20, null, 6, 9])
    // a jump to an end lands on its note: the row of words right above the first line of the end, or the note that
    // stands for the lines
    expect([rowOfPlace(closed, NEW, 5), rowOfPlace(closed, OLD, 20)]).toEqual([5, 25])
    const open = whole(MOVED, "inline", [20])
    expect([rowOfLine(open, OLD, 20), rowOfPlace(open, OLD, 20), rowOfLine(open, OLD, 22)]).toEqual([26, 25, 28])
    // any other line is its own row: the second line of the move, an unchanged line, the first row of the file
    expect([rowOfPlace(open, NEW, 6), rowOfPlace(open, OLD, 21), rowOfPlace(open, NEW, 12), rowOfPlace(open, NEW, 0)]).toEqual([7, 27, 13, 0])
    // the old line of an edited pair is found where the lines left, never in the row of the pair
    const edited = whole(EDITED, "inline", [20])
    expect([rowOfLine(edited, OLD, 22), describeRow(rowAt(edited, rowOfLine(edited, OLD, 22)))]).toEqual([28, "-22 pale away"])
  })

  it("says the words of an end of a move that starts its block in the heading of the change, and leaves a move with an end in no block out", () => {
    const units = { marks: new Map([[1, [{ name: "describe", words: "removed" }]]]), elsewhere: new Map<number, string>() }
    expect(changed(whole(MOVED, "inline", [], units)).slice(5)).toEqual(["unit describe — removed moved 20-24 closed source"])
    expect(changed(whole(MOVED, "unified", [], units)).slice(5, 7)).toEqual(["unit describe — removed moved 20-20 source", "-20 away"])
    // a block another section shows is one row of words: its move has no note here
    const elsewhere = { marks: new Map<number, Heading>(), elsewhere: new Map([[1, "Shown in part 2"]]) }
    const rows = whole(MOVED, "inline", [], elsewhere)
    expect([changed(rows).slice(5), rows.moveNotes.map(note => note.end.at)]).toEqual([["elsewhere Shown in part 2"], ["destination"]])
    const stray = textDiff(40, 40, [block(5, 5, 5, 9), block(20, 24, 24, 24)], { moves: [move(18, 5, 4), move(20, 9, 1)] })
    expect(changed(whole(stray, "inline"))).toEqual(["+5", "+6", "+7", "+8", "-20", "-21", "-22", "-23"])
  })

  it("lands a jump on the note of the other end by the line that end has, also where blank lines of the move reach out of its block", () => {
    // marks no engine gives: two blank lines before the old lines and three after them, the blocks hold none of them
    const tight = textDiff(40, 40, [block(5, 5, 5, 9), block(20, 24, 24, 24)], { moves: [move(20, 5, 4, { blanksBefore: 2, blanksAfter: 3 })] })
    for (const mode of ["inline", "unified", "split"] as const) {
      const rows = whole(tight, mode)
      const [stands, left] = rows.moveNotes
      // each note names the first line the other end shows, and a jump to that line lands on the note there
      expect([stands!.end.other, left!.end.other], mode).toEqual([
        [20, 24],
        [5, 9],
      ])
      expect([rowOfPlace(rows, OLD, stands!.end.other[0]), rowOfPlace(rows, NEW, left!.end.other[0])], mode).toEqual([left!.row, stands!.row])
    }
  })

  it("draws a move with an end in a block of the imports alone that a review plays down at neither end: every line a row of its block, no note; elsewhere the move shows", () => {
    const played = (diff: ReturnType<typeof textDiff>, mode: DiffViewMode) => buildFileRows(diff, mode, [EXPANDED, EXPANDED, EXPANDED], true)
    const stops = (rows: FileRows) => rows.changeStarts.map(start => describeRow(rowAt(rows, start)))
    // the import of line 3 moves up to line 1: block 0 adds it, block 1 deletes it, both blocks of the imports alone
    const imports = textDiff(10, 10, [block(1, 1, 1, 2), block(3, 4, 4, 4)], { importBlocks: [0, 1], moves: [move(3, 1, 1)] })
    for (const mode of ["inline", "unified"] as const) {
      const rows = played(imports, mode)
      // every line of both blocks, pale as a change of whitespace alone: no line left out, none in the colour of a move
      expect([changed(rows), rows.changeStarts, rows.moveNotes, rows.notes], mode).toEqual([["+1 whitespace", "-3 whitespace"], [], [], []])
    }
    expect(changed(played(imports, "split"))).toEqual(["_|1 whitespace", "3|_ whitespace"])
    // outside a review with its plan the same blocks are changes as any: the move shows, a stop where the line stands
    const plain = whole(imports, "inline")
    expect([changed(plain), stops(plain)]).toEqual([["remark moved 1-1 destination", "+1 here", "remark moved 3-4 closed source"], ["+1 here"]])

    // an import leaves a block of the imports alone for a block with code beside it: the block that is played down
    // shows its line as it did, and the line where it stands now is an inserted line of its block, read with it
    const into = textDiff(12, 12, [block(1, 2, 1, 1), block(6, 7, 5, 7)], { importBlocks: [0], moves: [move(1, 5, 1)] })
    for (const [mode, rows] of [
      ["inline", ["-1 whitespace", "-6", "+5", "+6"]],
      ["unified", ["-1 whitespace", "-6", "+5", "+6"]],
      ["split", ["1|_ whitespace", "6|5", "_|6"]],
    ] as const) {
      const shown = played(into, mode)
      expect([changed(shown), stops(shown), shown.moveNotes, shown.notes], mode).toEqual([rows, [rows[1]], [], []])
    }
    expect(changed(whole(into, "inline"))).toEqual(["remark moved 1-2 closed source", "remark moved 6-6 destination", "+5 here", "-6", "+6"])

    // four imports leave a block with code (a line changed before them) for a block of the imports alone, the second
    // edited on the way. The block they left shows them as its deleted lines, each with its row: none behind a note
    // that says they moved, since no row where they stand now says so or shows what the edited line was
    const out = textDiff(20, 20, [block(1, 6, 1, 2), block(8, 8, 4, 8)], {
      importBlocks: [1],
      moves: [move(2, 4, 4, { edited: [1], oldRanges: [[], [7, 10, 2], [], []], newRanges: [[], [7, 12, 2], [], []] })],
    })
    const stands = ["+4 whitespace", "+5 whitespace", "+6 whitespace", "+7 whitespace"]
    for (const [mode, rows] of [
      ["inline", ["-1", "-2", "-3", "-4", "-5", "+1", ...stands]],
      ["unified", ["-1", "-2", "-3", "-4", "-5", "+1", ...stands]],
      ["split", ["1|1", "2|_", "3|_", "4|_", "5|_", ...stands.map(row => `_|${row.slice(1)}`)]],
    ] as const) {
      const shown = played(out, mode)
      // the one stop is the block with code, read from its first row; the imports where they stand are passed
      expect([changed(shown), stops(shown), shown.moveNotes, shown.notes], mode).toEqual([rows, [rows[0]], [], []])
      // the old line of the edited import is its own row, in the block it left
      expect(describeRow(rowAt(shown, rowOfLine(shown, OLD, 3))), mode).toBe(mode === "split" ? "3|_" : "-3")
      expect([noteOfLine(shown, 3), rowOfPlace(shown, OLD, 2)], mode).toEqual([null, rowOfLine(shown, OLD, 2)])
    }
    // outside a review the move shows at both ends, the edited import as the row of its two lines where it stands
    expect(changed(whole(out, "inline"))).toEqual(["-1", "+1", "remark moved 2-6 closed source", "remark moved 8-8 destination", "+4 here", "~3:5 here", "+6 here", "+7 here"])
    // a move between two blocks with code in the same review is drawn as anywhere
    const code = textDiff(20, 20, [block(1, 6, 1, 2), block(8, 8, 4, 8), block(12, 13, 12, 13)], { importBlocks: [2], moves: [move(2, 4, 4)] })
    expect(played(code, "inline").moveNotes.map(note => note.end.at)).toEqual(["source", "destination"])
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

describe("the renames of a diff in its rows", () => {
  // the local `total` is `sum` at three lines, far apart, the name alone at each: a run of the kind `renamed` a block
  const renamed = (quiet: boolean, importBlocks: number[] = []) =>
    textDiff(60, 60, [block(10, 11, 10, 11, { inlinePairs: true }), block(30, 31, 30, 31, { inlinePairs: true }), block(50, 51, 50, 51, { inlinePairs: true })], {
      importBlocks,
      runs: [10, 30, 50].map(line => run(line, line + 1, line, line + 1, { kind: "renamed", quiet })),
      renames: [
        rename("total", "sum", [
          [10, 4, 10, 4],
          [30, 2, 30, 2],
          [50, 9, 50, 9],
        ]),
      ],
    })
  /** The rows of the blocks, no unchanged line and no fold, with the names a note names after it */
  const named = (rows: FileRows) =>
    Array.from({ length: rows.rowCount }, (_, index) => rowAt(rows, index)).flatMap(row => {
      const text = describeRow(row)
      if (/^\d+:\d+$/.test(text) || row.kind === "fold") {
        return []
      }
      return [row.kind === "note" && row.remark?.renames != null ? `${text} ${row.remark.renames.renames.map(it => `${it.old}>${it.new}`).join(" ")}` : text]
    })

  it("names a rename once in the rows of a file, before the lines of its declaration, and gives its other sites no note", () => {
    const label = renamed(false)
    expect(named(whole(label, "inline"))).toEqual(["remark renamed 10-10 total>sum", "~10:10", "~30:30", "~50:50"])
    expect(named(whole(label, "unified"))).toEqual(["remark renamed 10-10 total>sum", "-10", "+10", "-30", "+30", "-50", "+50"])
    expect(named(whole(label, "split"))).toEqual(["remark renamed 10-10 total>sum", "10|10", "30|30", "50|50"])
    // every line of a label is read: N and P stop at each block, at its line and never at the note; nothing opens
    const rows = whole(label, "inline")
    expect([rows.changeStarts.map(row => describeRow(rowAt(rows, row))), rows.changes, rows.notes, rows.noteRows]).toEqual([["~10:10", "~30:30", "~50:50"], 3, [], []])
  })

  it("shows the lines of a quiet rename once in Inline, which N and P pass, and opens them all by the one note of the rename", () => {
    const quiet = renamed(true)
    const closed = whole(quiet, "inline")
    expect(named(closed)).toEqual(["remark renamed 10-10 closed total>sum", "1x10 quiet", "1x30 quiet", "1x50 quiet"])
    // nothing of it to read: no stop, and the three blocks are still the changes the file counts
    expect([closed.changeStarts, closed.changes]).toEqual([[], 3])
    // the one note that opens, with every line it hides: the key of the notes, ⌥-click and a reveal of an old line take it
    expect([closed.notes, closed.noteRows.map(row => describeRow(rowAt(closed, row)))]).toEqual([
      [
        {
          key: 10,
          lines: [
            [10, 11],
            [30, 31],
            [50, 51],
          ],
        },
      ],
      ["remark renamed 10-10 closed"],
    ])
    expect([10, 30, 50, 31].map(line => noteOfLine(closed, line))).toEqual([10, 10, 10, null])
    // an old line the rows show once is found at the row of its new line, far from the note
    expect([10, 30, 50].map(line => describeRow(rowAt(closed, rowOfLine(closed, OLD, line))))).toEqual(["1x10 quiet", "1x30 quiet", "1x50 quiet"])
    // opened: every old line before its new one, all played down
    const opened = whole(quiet, "inline", [10])
    expect(named(opened)).toEqual(["remark renamed 10-10 open total>sum", "-10 quiet", "+10 quiet", "-30 quiet", "+30 quiet", "-50 quiet", "+50 quiet"])
    expect([10, 30, 50].map(line => describeRow(rowAt(opened, rowOfLine(opened, OLD, line))))).toEqual(["-10 quiet", "-30 quiet", "-50 quiet"])
    // Unified and Split keep both lines of each site, played down; their note opens nothing
    expect(named(whole(quiet, "unified"))).toEqual(["remark renamed 10-10 total>sum", "-10 quiet", "+10 quiet", "-30 quiet", "+30 quiet", "-50 quiet", "+50 quiet"])
    expect(named(whole(quiet, "split"))).toEqual(["remark renamed 10-10 total>sum", "10|10 quiet", "30|30 quiet", "50|50 quiet"])
    expect([whole(quiet, "unified").notes, whole(quiet, "split").changeStarts]).toEqual([[], []])
  })

  it("takes for the key of the notes the note of a line it hides or shows, wherever that note stands, beside the notes among the rows", () => {
    const quiet = renamed(true)
    const closed = whole(quiet, "inline")
    // the rows around the last site, forty lines below the note: the one note of the rename answers for the line shown once
    const last = rowOfLine(closed, NEW, 50)
    expect(describeRow(rowAt(closed, last))).toBe("1x50 quiet")
    expect(notesOfRows(closed, last - 2, last + 3)).toEqual([10])
    // unchanged rows alone answer for none; the note and its lines together are one note
    expect([notesOfRows(closed, last + 1, last + 4), notesOfRows(closed, 0, closed.rowCount), notesOfRows(closed, -5, 2)]).toEqual([[], [10], []])
    // opened: the old line and the new one of the site answer for the note too, so the key hides them again from there
    const opened = whole(quiet, "inline", [10])
    const shown = rowOfLine(opened, OLD, 50)
    expect([describeRow(rowAt(opened, shown)), notesOfRows(opened, shown, shown + 1), notesOfRows(opened, shown + 1, shown + 2)]).toEqual(["-50 quiet", [10], [10]])
    // lines that are read, and the layouts that hide no line, have no note to take
    expect([notesOfRows(whole(renamed(false), "inline"), 0, 100), notesOfRows(whole(quiet, "unified"), 0, 100)]).toEqual([[], []])
    // a quiet run of formatting: its note by its own row, and by the rows of its lines, closed and opened
    const wrapped = textDiff(10, 12, [block(3, 4, 3, 6)], { runs: [run(3, 4, 3, 6)] })
    const call = whole(wrapped, "inline")
    expect([notesOfRows(call, 3, 4), notesOfRows(call, 5, 7), notesOfRows(whole(wrapped, "inline", [3]), 4, 5)]).toEqual([[3], [3], [3]])
    // the lines a move took: the note stands for them, and the lines under it answer for it once it is opened
    const moved = textDiff(40, 40, [block(5, 5, 5, 8), block(20, 23, 21, 21)], { moves: [move(20, 5, 3)] })
    const left = whole(moved, "inline", [20])
    const first = rowOfLine(left, OLD, 21)
    expect([describeRow(rowAt(left, first)), notesOfRows(left, first, first + 1), notesOfRows(whole(moved, "inline"), 0, 100)]).toEqual(["-21 away", [20], [20]])
    // where the lines stand now nothing is hidden: no note
    expect(notesOfRows(left, 0, 9)).toEqual([])
  })

  it("names the rename at its first site in a section of a review that does not show its declaration, and opens the runs of that section alone", () => {
    const quiet = renamed(true)
    const units = { marks: new Map([[1, [{ name: "balance", words: "body changed" }]]]), elsewhere: new Map([[0, "Shown in part 1"]]) }
    const section = whole(quiet, "inline", [], units)
    // the block of the declaration is one row of words here; the note joins the heading of the block of the first site shown
    expect(named(section)).toEqual(["elsewhere Shown in part 1", "unit balance — body changed renamed 30-30 closed total>sum", "1x30 quiet", "1x50 quiet"])
    expect(section.notes).toEqual([
      {
        key: 30,
        lines: [
          [30, 31],
          [50, 51],
        ],
      },
    ])
    expect(named(whole(quiet, "inline", [30], units)).slice(1)).toEqual([
      "unit balance — body changed renamed 30-30 open total>sum",
      "-30 quiet",
      "+30 quiet",
      "-50 quiet",
      "+50 quiet",
    ])
  })

  it("keeps the note of a rename out of a block of the imports alone that a review plays down while another block holds a site", () => {
    // the first block is of the imports alone: with the plan of a review it is pale and passed, and the note stands at the next site
    const label = renamed(false, [0])
    const played = buildFileRows(label, "unified", [EXPANDED, EXPANDED, EXPANDED, EXPANDED], true)
    expect(named(played)).toEqual(["-10 whitespace", "+10 whitespace", "remark renamed 30-30 total>sum", "-30", "+30", "-50", "+50"])
    // without the plan the block is a block as any: the note is at the declaration
    expect(named(whole(label, "unified"))[0]).toBe("remark renamed 10-10 total>sum")
  })
})

describe("what the other files of a subject say of a file, in its rows", () => {
  // the report of a subject: two calls of a function another file declares and renamed, each alone on its line, and
  // five lines added at its end, of which the last four stood in another file
  const blocks = () => [block(4, 5, 4, 5, { inlinePairs: true }), block(10, 11, 10, 11, { inlinePairs: true }), block(16, 16, 16, 21)]
  const open = rename(
    "openStore",
    "connectStore",
    [
      [4, 16, 4, 16],
      [10, 16, 10, 16],
    ],
    { local: false, declared: [] },
  )
  const calls = (quiet: boolean) => [run(4, 5, 4, 5, { kind: "renamed", quiet }), run(10, 11, 10, 11, { kind: "renamed", quiet })]
  // as the pair of its two texts says it: nothing of the subject
  const alone = textDiff(16, 21, blocks())
  // with what the files of the subject say of each other, the index still to be asked: labels alone
  const told = textDiff(16, 21, blocks(), {
    runs: calls(false),
    subject: subjectMarks({ movedIn: [fileMove(17, 4, 0, 54)], used: [{ rename: open, file: 2, line: 18, proof: "asked" }] }),
  })
  // with the answer of the index: the two runs it confirmed quiet, their blocks no stop
  const proven: typeof told = {
    ...told,
    marks: {
      ...told.marks,
      runs: calls(true),
      starts: startsOf(blocks(), calls(true)),
      subject: subjectMarks({ movedIn: [fileMove(17, 4, 0, 54)], used: [{ rename: open, file: 2, line: 18, proof: "confirmed" }], confirmed: [0, 1] }),
    },
  }
  /** The rows of lines of a layout, each by its kind and its lines: what a reader has on screen beside the rows of words */
  const lineRows = (rows: FileRows) =>
    Array.from({ length: rows.rowCount }, (_, index) => rowAt(rows, index)).flatMap(row =>
      row.kind === "note" || row.kind === "fold" ? [] : [`${row.kind} ${row.lines.join(",")}`],
    )

  it("adds rows of words alone to the rows the content of the file gave: every row of a line stays, the same line in the same order, in every layout", () => {
    for (const mode of ["inline", "unified", "split"] as const) {
      const before = whole(alone, mode)
      // the marks of the subject, then the answer of the index: what comes after the rows were shown
      for (const later of [told, proven]) {
        const after = whole(later, mode)
        expect(lineRows(after)).toEqual(lineRows(before))
        // two rows of words more: the note of the rename at its first site, the note where the moved lines stand
        expect(after.rowCount).toBe(before.rowCount + 2)
      }
    }
    expect(changed(whole(told, "inline"))).toEqual([
      "remark renamed 4-4",
      "~4:4",
      "~10:10",
      "+16",
      "remark moved 16-16 destination",
      "+17 here",
      "+18 here",
      "+19 here",
      "+20 here",
    ])
    // nothing of it opens: no note of a late mark hides a line
    expect([whole(told, "inline").notes, whole(proven, "inline").notes]).toEqual([[], []])
  })

  it("regroups the lines of a block that a mark of the subject cuts around its note, part by part, each line with the one row it had", () => {
    // three lines became three others, which the engine does not pair; the old line 11 stands in another file now
    const cut = () => [block(10, 13, 10, 13)]
    const before = textDiff(20, 20, cut())
    const after = textDiff(20, 20, cut(), { subject: subjectMarks({ movedOut: [fileMove(11, 1, 1, 30)] }) })
    // one column: the old lines, then the new ones; cut at the end of the move, the lines before it of both sides
    // come first, then the note and its line, then the rest
    expect(changed(whole(before, "unified"))).toEqual(["-10", "-11", "-12", "+10", "+11", "+12"])
    expect(changed(whole(after, "unified"))).toEqual(["-10", "+10", "+11", "+12", "remark moved 11-11 source", "-11 away", "-12"])
    for (const mode of ["inline", "unified", "split"] as const) {
      // the same lines, each in one row, whatever their order in the block
      const lines = (rows: FileRows, side: number) =>
        Array.from({ length: rows.rowCount }, (_, index) => rowAt(rows, index))
          .flatMap(row => (row.kind === "note" || row.kind === "fold" || row.block < 0 ? [] : [row.lines[side] ?? -1]))
          .filter(line => line >= 0)
          .toSorted((one, other) => one - other)
      expect([lines(whole(after, mode), OLD), lines(whole(after, mode), NEW)]).toEqual([lines(whole(before, mode), OLD), lines(whole(before, mode), NEW)])
      expect(lines(whole(after, mode), OLD)).toEqual([10, 11, 12])
    }
  })

  it("keeps the row at the top of a view at its line while the notes come above it", () => {
    // the view stands at the second call (the new line 10), then at the first of the moved lines (the new line 17)
    const [before, after] = [whole(alone, "inline"), whole(told, "inline")]
    expect([rowOfLine(before, NEW, 10), rowOfLine(after, NEW, 10)]).toEqual([10, 11])
    expect([rowOfLine(before, NEW, 17), rowOfLine(after, NEW, 17)]).toEqual([17, 19])
    // the row of the line itself, never the note that came above it
    expect([describeRow(rowAt(after, 11)), describeRow(rowAt(after, 19)), describeRow(rowAt(after, 18))]).toEqual(["~10:10", "+17 here", "remark moved 16-16 destination"])
  })

  it("stops N and P at each use of the name while the index proved nothing, and passes the uses it proved, which stay on screen played down", () => {
    const stops = (rows: FileRows) => rows.changeStarts.map(row => describeRow(rowAt(rows, row)))
    // labels: each call is read, and the lines that came from another file are read where they stand
    expect([stops(whole(told, "inline")), whole(told, "inline").changes]).toEqual([["~4:4", "~10:10", "+16"], 3])
    // proven: the calls are no stop; their rows are the rows they were, in the quiet tint
    const rows = whole(proven, "inline")
    expect([stops(rows), rows.changes]).toEqual([["+16"], 3])
    const call = rowAt(rows, rowOfLine(rows, NEW, 10)) as LineRow
    expect([call.kind, call.lines, call.tint, call.run]).toEqual(["inline", [10, 10], "quiet", "renamed"])
    // a jump to the end of the move that stands here lands on its note, the row right above its first line
    expect([rowOfPlace(rows, NEW, 17), rows.moveNotes.map(note => [note.end.at, note.end.file, note.row])]).toEqual([18, [["destination", 0, 18]]])
  })

  it("reads the lines that left for another file where they left: a stop of N and P, every line a row after the note, in Inline too", () => {
    // the ledger of the subject: a blank line and four lines deleted at its end, which stand in the report now
    const ledger = textDiff(58, 53, [block(53, 58, 53, 53)], { subject: subjectMarks({ movedOut: [fileMove(54, 4, 1, 17)] }) })
    for (const mode of ["inline", "unified"] as const) {
      const rows = whole(ledger, mode)
      expect(changed(rows)).toEqual(["-53", "remark moved 54-54 source", "-54 away", "-55 away", "-56 away", "-57 away"])
      expect(rows.changeStarts.map(row => describeRow(rowAt(rows, row)))).toEqual(["-53"])
      // no old line is behind a note: each is found at its own row, and a jump to the end lands on its note
      expect([noteOfLine(rows, 54), rowOfLine(rows, OLD, 55), rowOfPlace(rows, OLD, 54)]).toEqual([null, 56, 54])
    }
    expect(changed(whole(ledger, "split"))).toEqual(["53|_", "remark moved 54-54 source", "54|_ away", "55|_ away", "56|_ away", "57|_ away"])
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
