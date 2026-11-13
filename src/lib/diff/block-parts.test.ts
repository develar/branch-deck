// @spec openspec/specs/commit/quiet-changes/spec.md
import { describe, expect, it } from "vite-plus/test"
import {
  blockParts,
  drawnMarks,
  inlinePairsOf,
  marksByBlock,
  NO_MARKS,
  NO_RENAME_NOTES,
  noteKey,
  partsLines,
  partsMoveNotes,
  partsNoteOfRow,
  partsNotes,
  partsRow,
  partsRowOfLine,
  partsStopRow,
  renameJumpOf,
  renameNotesOf,
  stopsOf,
} from "@/lib/diff/block-parts"
import type { BlockMarks, PartsOptions, RenameNotes } from "@/lib/diff/block-parts"
import type { LineRun, TextDiff } from "@/lib/diff/file-diff-model"
import type { DiffViewMode } from "@/stores/settings/diff-view-settings"
import { block, fileMove, move, rename, renameCheck, run, subjectMarks, textDiff } from "@/test-support/diff-blocks"

const OLD = 0
const NEW = 1
const options = (mode: PartsOptions["mode"], open: number[] = []): PartsOptions => ({ mode, tint: "full", open: new Set(open), heading: null })
/** A small generator with a seed (mulberry32): the next number below `below`, the same numbers in every run of a test */
function seeded(start: number): (below: number) => number {
  let seed = start
  return below => {
    seed = (seed + 0x6d2b79f5) | 0
    let mixed = Math.imul(seed ^ (seed >>> 15), 1 | seed)
    mixed = (mixed + Math.imul(mixed ^ (mixed >>> 7), 61 | mixed)) ^ mixed
    return ((mixed ^ (mixed >>> 14)) >>> 0) % below
  }
}

/** The marks of a block that has runs alone */
const ofRuns = (runs: LineRun[]): BlockMarks => ({ ...NO_MARKS, runs })

describe("blockParts", () => {
  // a line added, three lines indented again (quiet), a line deleted, a comment worded again (a label), a line added
  const marked = block(2, 7, 2, 8)
  const runs = ofRuns([run(2, 5, 3, 6), run(6, 7, 6, 7, { kind: "comments", quiet: false })])

  it("is one part of lines for a block without a run, in the shape of its layout", () => {
    const plain = block(2, 4, 2, 3)
    expect([
      blockParts(plain, 0, NO_MARKS, options("split")).rowCount,
      blockParts(plain, 0, NO_MARKS, options("unified")).rowCount,
      blockParts(plain, 0, NO_MARKS, options("inline")).rowCount,
    ]).toEqual([2, 3, 3])
    // a similar pair is one row in Inline alone; an ignored block shows its new lines in one column
    const pair = block(2, 3, 2, 3, { inlinePairs: true })
    expect([blockParts(pair, 0, NO_MARKS, options("inline")).rowCount, blockParts(pair, 0, NO_MARKS, options("unified")).rowCount]).toEqual([1, 2])
    const ignored = block(2, 4, 2, 3, { kind: "ignored" })
    expect([blockParts(ignored, 0, NO_MARKS, options("unified")).rowCount, blockParts(ignored, 0, NO_MARKS, options("split")).rowCount]).toEqual([1, 2])
  })

  it("counts the rows of each part: a note a row, a quiet run of Inline its new lines until its note is opened", () => {
    // Inline: +2, note, 3 lines once, -5, note, -6 +6, +7
    const inline = blockParts(marked, 0, runs, options("inline"))
    expect([inline.rowCount, Array.from(inline.starts)]).toEqual([10, [0, 1, 2, 5, 6, 7, 9, 10]])
    // opened: the three old lines before the three new ones
    expect(blockParts(marked, 0, runs, options("inline", [2])).rowCount).toBe(13)
    expect(blockParts(marked, 0, runs, options("unified")).rowCount).toBe(13)
    // Split: _|2, note, 3 rows, 5|_, note, 6|6, _|7
    expect(blockParts(marked, 0, runs, options("split")).rowCount).toBe(9)
    // only a quiet run of Inline has a note that opens, by the old lines it hides
    expect([partsNotes(inline), partsNotes(blockParts(marked, 0, runs, options("unified")))]).toEqual([[{ key: 2, lines: [[2, 5]], row: 1 }], []])
  })

  it("gives each row by its part, and the row of each line of a side", () => {
    const inline = blockParts(marked, 3, runs, options("inline"))
    const kinds = Array.from({ length: inline.rowCount }, (_, row) => {
      const value = partsRow(inline, row)
      return value.kind === "note" ? `note ${value.remark!.kind}` : `${value.kind} ${value.lines.join(",")}`
    })
    expect(kinds).toEqual(["added ,2", "note formatting", "once ,3", "once ,4", "once ,5", "deleted 5,", "note comments", "deleted 6,", "added ,6", "added ,7"])
    expect(partsRow(inline, 2)).toMatchObject({ block: 3, blockRow: 2, blockRows: 10, tint: "quiet", run: "formatting" })
    // the old lines of the quiet run are behind its note; the lines after it have their rows
    expect([2, 3, 4, 5, 6, 7].map(line => partsRowOfLine(inline, OLD, line))).toEqual([1, 1, 1, 5, 7, 10])
    expect([2, 3, 5, 6, 7, 8].map(line => partsRowOfLine(inline, NEW, line))).toEqual([0, 2, 4, 8, 9, 10])
    // the reading starts at the first row of lines since the last quiet run: the comment is read with the line deleted
    // before its note, as Rust cuts a block by its quiet runs alone; a line of no part is past the rows
    expect([partsStopRow(inline, NEW, 2), partsStopRow(inline, NEW, 6), partsStopRow(inline, OLD, 5), partsStopRow(inline, NEW, 7), partsStopRow(inline, NEW, 9)]).toEqual([
      0, 5, 5, 5, 10,
    ])
    // a line of the quiet run, which Rust never names, is where it shows
    expect(partsStopRow(inline, NEW, 4)).toBe(2)
  })

  it("gives an old line of lines that one column shows by the new ones alone the row of the new line at its place, and the first row where the sides differ in their lines", () => {
    // a block the whitespace policy ignores, in one column: its new lines stand for the old ones
    const same = blockParts(block(2, 5, 2, 5, { kind: "ignored" }), 0, NO_MARKS, options("unified"))
    expect([2, 3, 4, 5].map(line => partsRowOfLine(same, OLD, line))).toEqual([0, 1, 2, 3])
    const fewer = blockParts(block(2, 5, 2, 4, { kind: "ignored" }), 0, NO_MARKS, options("unified"))
    expect([2, 3, 4, 5].map(line => partsRowOfLine(fewer, OLD, line))).toEqual([0, 0, 0, 2])
    expect([2, 3, 4].map(line => partsRowOfLine(fewer, NEW, line))).toEqual([0, 1, 2])
  })

  it("starts the reading of a block at its first row of lines to read, in every layout, whatever labels cut it", () => {
    // six lines deleted, then a comment worded again: Rust names the comment, the first new line of the block
    const deletion = block(1, 8, 1, 2)
    const label = ofRuns([run(7, 8, 1, 2, { kind: "comments", quiet: false })])
    for (const mode of ["inline", "unified", "split"] as const) {
      const cut = blockParts(deletion, 0, label, options(mode))
      // the deleted lines, the note, the comment: the stop is the first deleted line, where the block without a run stops
      expect([cut.parts.length, partsStopRow(cut, NEW, 1), partsStopRow(blockParts(deletion, 0, NO_MARKS, options(mode)), NEW, 1)], mode).toEqual([3, 0, 0])
    }
    // under a heading the first row of lines, never the heading; two labels of a block are one reading
    const heading = { parts: [{ name: "record", words: "body changed" }], label: "record — body changed" }
    const labels = ofRuns([run(1, 2, 1, 2, { kind: "comments", quiet: false }), run(4, 5, 2, 3, { kind: "comments", quiet: false })])
    const headed = blockParts(block(1, 5, 1, 3), 0, labels, { ...options("unified"), heading })
    // the heading with the first label, -1 +1, -2 -3, the second label, -4 +2
    expect([headed.rowCount, partsStopRow(headed, NEW, 1), partsStopRow(headed, NEW, 2), partsStopRow(headed, OLD, 2)]).toEqual([8, 1, 1, 1])
  })

  it("says the lines its rows cover, the lines a note stands for with them", () => {
    const inline = blockParts(marked, 0, runs, options("inline"))
    // the whole block; the note and the lines shown once; the lines once alone
    expect(partsLines(inline, 0, 10)).toEqual([
      [2, 7],
      [2, 8],
    ])
    expect(partsLines(inline, 1, 4)).toEqual([
      [2, 5],
      [3, 6],
    ])
    expect(partsLines(inline, 3, 1)).toEqual([
      [3, 4],
      [4, 5],
    ])
    expect(partsLines(inline, 7, 3)).toEqual([
      [6, 7],
      [6, 8],
    ])
  })
})

describe("the marks of a diff", () => {
  it("gives each block its runs, and none to a run that lies in no block", () => {
    const diff = textDiff(20, 20, [block(2, 5, 2, 5), block(10, 12, 10, 12)], { runs: [run(0, 1, 0, 1), run(2, 3, 2, 3), run(4, 5, 4, 5), run(4, 6, 4, 6), run(10, 12, 10, 12)] })
    expect(marksByBlock(diff).map(marks => marks.runs.map(it => it.oldStart))).toEqual([[2, 4], [10]])
    // read once per diff
    expect(marksByBlock(diff)).toBe(marksByBlock(diff))
  })

  it("reads the stops as Rust gives them, and passes a block of the imports alone in a review with its plan", () => {
    const diff = textDiff(30, 32, [block(1, 2, 1, 2), block(10, 11, 10, 13), block(20, 21, 22, 23), block(25, 26, 27, 28, { kind: "ignored" })], {
      runs: [run(10, 11, 10, 13)],
      importBlocks: [0],
    })
    expect([...stopsOf(diff)]).toEqual([
      [0, { side: NEW, line: 1 }],
      [2, { side: NEW, line: 22 }],
    ])
    expect([...stopsOf(diff, true).keys()]).toEqual([2])
  })

  it("lists the pairs of lines that are one row each in Inline", () => {
    const pairs = block(2, 5, 2, 5, { inlinePairs: true })
    const linesOf = (marks: BlockMarks) => inlinePairsOf(pairs, 0, marks).map(pair => pair.lines)
    expect(linesOf(NO_MARKS)).toEqual([
      [2, 2],
      [3, 3],
      [4, 4],
    ])
    expect(linesOf(ofRuns([run(3, 4, 3, 4)]))).toEqual([
      [2, 2],
      [4, 4],
    ])
    expect(inlinePairsOf(block(2, 5, 2, 5), 0, NO_MARKS)).toEqual([])
  })
})

/**
 * The rows of the parts of a block with moves as text: a note by its kind, the lines it stands for, whether it opens
 * and its end, of a move between two files the file of its other end too ("source file 1"); a line by its kind, its
 * lines, its tint and the end of its move
 */
const moveRowsOf = (parts: ReturnType<typeof blockParts>) =>
  Array.from({ length: parts.rowCount }, (_, row) => {
    const value = partsRow(parts, row)
    if (value.kind === "note") {
      const { remark } = value
      const end = remark!.move === null ? "" : ` ${remark!.move.at}${remark!.move.file === null ? "" : ` file ${remark!.move.file}`}`
      return `note ${remark!.kind} ${value.lines[OLD]!.join("-")}${remark!.open === null ? "" : remark!.open ? " open" : " closed"}${end}`
    }
    return `${value.kind} ${value.lines.map(line => line ?? "_").join(",")}${value.tint === "full" ? "" : ` ${value.tint}`}${value.move === null ? "" : ` ${value.move.at}`}`
  })

describe("the ends of the moves of a block", () => {
  const rowsOf = moveRowsOf
  // four lines leave the old lines 20..24 and stand at the new lines 5..9: the block they left deletes them alone, the
  // block they stand in adds them alone
  const left = block(20, 24, 24, 24)
  const stands = block(5, 5, 5, 9)
  const exact = textDiff(40, 40, [stands, left], { moves: [move(20, 5, 4)] })
  // the same lines indented, the third one edited on the way, and the blank line before them now after them
  const edited = textDiff(40, 40, [stands, left], {
    moves: [move(21, 5, 3, { indent: 2, blanksBefore: 1, edited: [2], oldRanges: [[], [], [4, 9, 2]], newRanges: [[], [], [6, 12, 2]] })],
  })

  it("gives each block the ends of its moves, the blank lines that moved with them among their lines, and none of a move with an end in no block", () => {
    const [to, from] = marksByBlock(exact)
    expect([to!.destinations.map(end => [end.at, end.lines]), to!.sources, from!.sources.map(end => [end.at, end.lines]), from!.destinations]).toEqual([
      [["destination", [5, 9]]],
      [],
      [["source", [20, 24]]],
      [],
    ])
    expect(to!.destinations[0]!.move).toBe(exact.marks.moves[0])
    // each end knows the lines of the other one: what its note names, and where a jump from it lands
    expect([to!.destinations[0]!.other, from!.sources[0]!.other]).toEqual([
      [20, 24],
      [5, 9],
    ])
    // and the block that holds them
    expect([to!.destinations[0]!.otherBlock, from!.sources[0]!.otherBlock]).toEqual([1, 0])
    // one blank line before the old lines stands after the new ones: a line of the move on both sides
    const [toEdited, fromEdited] = marksByBlock(edited)
    expect([toEdited!.destinations[0]!.lines, fromEdited!.sources[0]!.lines]).toEqual([
      [5, 9],
      [20, 24],
    ])
    // the blank lines never reach out of the block, on the end itself and as the other end names it: the one place
    // the lines of an end are read from the blank lines of the move
    const tight = marksByBlock(textDiff(40, 40, [stands, left], { moves: [move(20, 5, 4, { blanksBefore: 2, blanksAfter: 3 })] }))
    expect([tight[0]!.destinations[0]!.lines, tight[1]!.sources[0]!.lines, tight[0]!.destinations[0]!.other, tight[1]!.sources[0]!.other]).toEqual([
      [5, 9],
      [20, 24],
      [20, 24],
      [5, 9],
    ])
    // old lines that reach out of their block, new lines in no block, no line, and a block the policy ignores
    for (const stray of [move(22, 5, 4), move(20, 12, 2), move(20, 5, 0), move(30, 5, 1)]) {
      const marks = marksByBlock(textDiff(40, 40, [stands, left, block(30, 31, 30, 31, { kind: "ignored" })], { moves: [stray] }))
      expect(marks.map(it => [it.sources.length, it.destinations.length])).toEqual([
        [0, 0],
        [0, 0],
        [0, 0],
      ])
    }
    // the ends of the old side of a block in the order of its old lines, though the moves come by their new lines
    const crossed = textDiff(40, 40, [block(5, 5, 5, 9), block(20, 24, 24, 24)], { moves: [move(22, 5, 2), move(20, 7, 2)] })
    expect(marksByBlock(crossed)[1]!.sources.map(end => end.lines)).toEqual([
      [20, 22],
      [22, 24],
    ])
  })

  it("leaves a move with an end in a block a view plays down out of both its blocks, and no other move", () => {
    // two moves into the first block, one from each of the others; a run in the second
    const diff = textDiff(40, 40, [block(5, 5, 5, 9), block(20, 24, 24, 24), block(30, 33, 30, 31)], { moves: [move(20, 5, 2), move(31, 7, 2)], runs: [run(30, 31, 30, 31)] })
    const marks = marksByBlock(diff)
    const ends = (played: number[]) =>
      marks.map((it, index) => {
        const drawn = drawnMarks(it, index, new Set(played))
        return [drawn.sources.map(end => end.lines[0]), drawn.destinations.map(end => end.lines[0]), drawn.runs.length]
      })
    // none played down: the marks themselves, no copy
    expect(marks.map((it, index) => drawnMarks(it, index, new Set()))).toEqual(marks)
    expect(drawnMarks(marks[0]!, 0, new Set())).toBe(marks[0])
    expect(ends([])).toEqual([
      [[], [5, 7], 0],
      [[20], [], 0],
      [[31], [], 1],
    ])
    // the block of where the lines left: its end and the end where they stand now go, the other move stays whole
    expect(ends([1])).toEqual([
      [[], [7], 0],
      [[], [], 0],
      [[31], [], 1],
    ])
    expect(drawnMarks(marks[2]!, 2, new Set([1]))).toBe(marks[2])
    // the block of where the lines stand now: both moves go from every block; the runs stay
    expect(ends([0])).toEqual([
      [[], [], 0],
      [[], [], 0],
      [[], [], 1],
    ])
    expect(drawnMarks(NO_MARKS, 0, new Set([0]))).toBe(NO_MARKS)
  })

  it("stands a note for the lines that left in Inline until it is opened, and shows them after it elsewhere", () => {
    const marks = marksByBlock(exact)[1]!
    // Inline: the note alone, which stands for the four old lines and opens
    const closed = blockParts(left, 1, marks, options("inline"))
    expect(rowsOf(closed)).toEqual(["note moved 20-24 closed source"])
    expect(partsNotes(closed)).toEqual([{ key: 20, lines: [[20, 24]], row: 0 }])
    expect([20, 21, 23, 24].map(line => partsRowOfLine(closed, OLD, line))).toEqual([0, 0, 0, 1])
    expect(partsLines(closed, 0, 1)).toEqual([
      [20, 24],
      [24, 24],
    ])
    // opened, and in Unified: the lines after the note, deleted rows of the move
    const open = ["note moved 20-20", "deleted 20,_ source", "deleted 21,_ source", "deleted 22,_ source", "deleted 23,_ source"]
    expect(rowsOf(blockParts(left, 1, marks, options("inline", [20])))).toEqual([`${open[0]} open source`, ...open.slice(1)])
    expect(rowsOf(blockParts(left, 1, marks, options("unified")))).toEqual([`${open[0]} source`, ...open.slice(1)])
    expect(partsNotes(blockParts(left, 1, marks, options("unified")))).toEqual([])
    // Split: the old side, the new one filled
    expect(rowsOf(blockParts(left, 1, marks, options("split")))).toEqual([
      "note moved 20-20 source",
      "changed 20,_ source",
      "changed 21,_ source",
      "changed 22,_ source",
      "changed 23,_ source",
    ])
  })

  it("shows the lines of a move once where they stand now, after a note, an edited one as the pair of its two lines in Inline", () => {
    const marks = marksByBlock(edited)[0]!
    // Inline: the third line is the pair of the old line 23 and the new line 7, pale as a line with inner ranges; the
    // blank line that moved with the lines is one of them
    const inline = blockParts(stands, 0, marks, options("inline"))
    expect(rowsOf(inline)).toEqual(["note moved 5-5 destination", "added _,5 destination", "added _,6 destination", "inline 23,7 pale destination", "added _,8 destination"])
    // no line is hidden where the lines stand: the note opens nothing
    expect([partsNotes(inline), partsMoveNotes(inline).map(note => [note.end.at, note.row])]).toEqual([[], [["destination", 0]]])
    // the old line of the pair is a line of where the lines left: no row of this block is found by it
    expect([partsRowOfLine(inline, OLD, 23), partsRowOfLine(inline, NEW, 7), partsRowOfLine(inline, NEW, 8)]).toEqual([5, 3, 4])
    expect(partsLines(inline, 3, 1)).toEqual([
      [5, 5],
      [7, 8],
    ])
    // Unified and Split keep the new lines alone: every line stays at its own place
    expect(rowsOf(blockParts(stands, 0, marks, options("unified")))).toEqual([
      "note moved 5-5 destination",
      "added _,5 destination",
      "added _,6 destination",
      "added _,7 pale destination",
      "added _,8 destination",
    ])
    expect(rowsOf(blockParts(stands, 0, marks, options("split"))).slice(3)).toEqual(["changed _,7 pale destination", "changed _,8 destination"])
    // where they left, the edited line is pale too, and the blank line before them is the first line of the move
    expect(rowsOf(blockParts(left, 1, marksByBlock(edited)[1]!, options("unified")))).toEqual([
      "note moved 20-20 source",
      "deleted 20,_ source",
      "deleted 21,_ source",
      "deleted 22,_ source",
      "deleted 23,_ pale source",
    ])
  })

  it("cuts the lines of no run at the ends of the moves, each on its own side, and reads a block from its first line no move took", () => {
    // a line changed, three lines that left, two lines deleted: the lines of no move show as the block does
    const mixed = block(10, 16, 10, 11)
    const diff = textDiff(50, 48, [mixed, block(35, 35, 30, 33)], { moves: [move(11, 30, 3)] })
    const marks = marksByBlock(diff)[0]!
    const inline = blockParts(mixed, 0, marks, options("inline"))
    expect(rowsOf(inline)).toEqual(["deleted 10,_", "added _,10", "note moved 11-14 closed source", "deleted 14,_", "deleted 15,_"])
    expect(rowsOf(blockParts(mixed, 0, marks, options("split")))).toEqual([
      "changed 10,10",
      "note moved 11-11 source",
      "changed 11,_ source",
      "changed 12,_ source",
      "changed 13,_ source",
      "changed 14,_",
      "changed 15,_",
    ])
    // Rust names the first new line; one column shows the old line of that part first
    expect([...stopsOf(diff)]).toEqual([
      [0, { side: NEW, line: 10 }],
      [1, { side: NEW, line: 30 }],
    ])
    expect([partsStopRow(inline, NEW, 10), partsStopRow(blockParts(mixed, 0, marks, options("unified")), NEW, 10)]).toEqual([0, 0])
    // the lines a move took end what is read before them, as a quiet run does: a block that starts with them is read
    // from the line after them, in every layout, the lines shown or behind their note
    const after = block(10, 15, 10, 10)
    const taken = textDiff(50, 48, [after, block(35, 35, 30, 33)], { moves: [move(10, 30, 3)] })
    expect([...stopsOf(taken)][0]).toEqual([0, { side: OLD, line: 13 }])
    for (const [mode, row] of [
      ["inline", 1],
      ["unified", 4],
      ["split", 4],
    ] as const) {
      const cut = blockParts(after, 0, marksByBlock(taken)[0]!, options(mode))
      expect([partsStopRow(cut, OLD, 13), partsRow(cut, row).kind === "note"], mode).toEqual([row, false])
    }
    // a block whose every old line a move took has no start: N and P pass it
    expect([...stopsOf(exact).keys()]).toEqual([0])
    // where the lines stand now the reading starts at the first of them, under their note
    expect(partsStopRow(blockParts(stands, 0, marksByBlock(exact)[0]!, options("inline")), NEW, 5)).toBe(1)
  })

  it("keeps the pairs of the lines beside a move in a block the engine pairs, and shows the line whose pair moved alone", () => {
    // four pairs; the third old line left, so the third new line has no pair
    const pairs = block(10, 14, 10, 14, { inlinePairs: true })
    const diff = textDiff(50, 51, [pairs, block(30, 30, 30, 31)], { moves: [move(12, 30, 1)] })
    const marks = marksByBlock(diff)[0]!
    expect(rowsOf(blockParts(pairs, 0, marks, options("inline")))).toEqual(["inline 10,10", "inline 11,11", "note moved 12-13 closed source", "added _,12", "inline 13,13"])
    expect(inlinePairsOf(pairs, 0, marks).map(pair => pair.lines)).toEqual([
      [10, 10],
      [11, 11],
      [13, 13],
    ])
    // a line that stands in such a block now: the old line beside it has no pair
    const landing = textDiff(51, 50, [block(5, 6, 5, 5), pairs], { moves: [move(5, 12, 1)] })
    expect(rowsOf(blockParts(pairs, 1, marksByBlock(landing)[1]!, options("inline")))).toEqual([
      "inline 10,10",
      "inline 11,11",
      "note moved 12-12 destination",
      "added _,12 destination",
      "deleted 12,_",
      "inline 13,13",
    ])
  })

  it("lists the edited lines of the moves that stand in a block as rows of two lines, with the ranges of their move", () => {
    expect(inlinePairsOf(stands, 0, marksByBlock(edited)[0]!)).toEqual([{ lines: [23, 7], oldRanges: [4, 9, 2], newRanges: [6, 12, 2] }])
    // a move without an edited line makes no such row, and the block its lines left none
    expect([inlinePairsOf(stands, 0, marksByBlock(exact)[0]!), inlinePairsOf(left, 1, marksByBlock(edited)[1]!)]).toEqual([[], []])
  })

  it("shows every line of a block once whatever the marks say: by its own row, or behind the note that stands for it", () => {
    // the same blocks, runs and moves in every run of the test
    const random = seeded(20261111)
    // what the rounds held, so the test says when its marks stop being what it is about
    const held = { sources: 0, destinations: 0, overlapping: 0, runs: 0 }
    for (let round = 0; round < 600; round++) {
      const one = block(10, 10 + random(9), 10, 10 + random(9), { inlinePairs: random(3) === 0 })
      const runs = Array.from({ length: random(3) }, () => {
        const [oldStart, newStart] = [10 + random(9), 10 + random(9)]
        return run(oldStart, oldStart + 1 + random(3), newStart, newStart + 1 + random(3), { quiet: random(2) === 0 })
      }).toSorted((a, b) => a.oldStart - b.oldStart)
      // moves out of the block and into it, valid or not: overlapping each other and the runs, reaching past the block
      const moves = Array.from({ length: random(4) }, () =>
        random(2) === 0
          ? move(10 + random(9), 40 + random(4), 1 + random(4), { blanksBefore: random(2), blanksAfter: random(2), edited: random(2) === 0 ? [0] : [] })
          : move(40 + random(4), 10 + random(9), 1 + random(4), { blanksBefore: random(2), blanksAfter: random(2), edited: random(2) === 0 ? [0] : [] }),
      ).toSorted((a, b) => a.newStart - b.newStart)
      const diff = textDiff(60, 60, [one, block(40, 48, 40, 48)], { runs, moves })
      const marks = marksByBlock(diff)[0]!
      held.sources += marks.sources.length > 0 ? 1 : 0
      held.destinations += marks.destinations.length > 0 ? 1 : 0
      held.runs += marks.runs.length > 0 && marks.sources.length + marks.destinations.length > 0 ? 1 : 0
      const overlap = (ends: typeof marks.sources) => ends.some((end, at) => at > 0 && end.lines[0] < ends[at - 1]!.lines[1])
      held.overlapping += overlap(marks.sources) || overlap(marks.destinations) ? 1 : 0
      for (const mode of ["inline", "unified", "split"] as const) {
        for (const open of [[], Array.from({ length: 20 }, (_, line) => line + 5)]) {
          const parts = blockParts(one, 0, marks, options(mode, open))
          const shown = [new Map<number, number>(), new Map<number, number>()]
          for (let row = 0; row < parts.rowCount; row++) {
            const value = partsRow(parts, row)
            if (value.kind === "note") {
              // the old lines a closed note stands for
              for (let line = value.lines[OLD]![0]; line < value.lines[OLD]![1]; line++) {
                shown[OLD]!.set(line, (shown[OLD]!.get(line) ?? 0) + 1)
              }
            } else {
              // the old line of the pair of an edited line of a move is a line of the block it left
              const sides = value.move?.at === "destination" ? [NEW] : [OLD, NEW]
              for (const side of sides) {
                const line = value.lines[side]
                if (line != null) {
                  shown[side]!.set(line, (shown[side]!.get(line) ?? 0) + 1)
                }
              }
            }
          }
          const context = `round ${round} ${mode} ${open.length === 0 ? "closed" : "open"}`
          expect(
            [...shown[OLD]!].toSorted((a, b) => a[0] - b[0]),
            context,
          ).toEqual(Array.from({ length: one.oldEnd - one.oldStart }, (_, at) => [one.oldStart + at, 1]))
          expect(
            [...shown[NEW]!].toSorted((a, b) => a[0] - b[0]),
            context,
          ).toEqual(Array.from({ length: one.newEnd - one.newStart }, (_, at) => [one.newStart + at, 1]))
          // and the row of each line is where it shows, or the note that stands for it
          for (let line = one.oldStart; line < one.oldEnd; line++) {
            const value = partsRow(parts, partsRowOfLine(parts, OLD, line))
            expect(value.kind === "note" ? value.lines[OLD]![0] <= line && line < value.lines[OLD]![1] : value.lines[OLD] === line, `${context} old ${line}`).toBe(true)
          }
          for (let line = one.newStart; line < one.newEnd; line++) {
            const value = partsRow(parts, partsRowOfLine(parts, NEW, line))
            expect(value.kind !== "note" && value.lines[NEW] === line, `${context} new ${line}`).toBe(true)
          }
        }
      }
    }
    // the rounds held blocks with ends of each side, ends that overlap, and ends beside runs
    expect([held.sources > 50, held.destinations > 50, held.overlapping > 10, held.runs > 10], JSON.stringify(held)).toEqual([true, true, true, true])
  })
})

describe("the notes of the renames of a diff", () => {
  // the local `total` is `sum` at three lines of three blocks, the name alone at each: a run of the kind `renamed` a
  // block, quiet once Rust says so; its declaration is the first site
  const sites = [
    [3, 4, 3, 4],
    [6, 2, 6, 2],
    [9, 9, 9, 9],
  ] as const
  const blocks = () => [block(3, 4, 3, 4, { inlinePairs: true }), block(6, 7, 6, 7, { inlinePairs: true }), block(9, 10, 9, 10, { inlinePairs: true })]
  const renamedRuns = (quiet: boolean) => [3, 6, 9].map(line => run(line, line + 1, line, line + 1, { kind: "renamed", quiet }))
  const local = (quiet: boolean, declared = [0]) => textDiff(20, 20, blocks(), { runs: renamedRuns(quiet), renames: [rename("total", "sum", sites, { declared })] })
  /** A note as its key, the names it names, the site it stands at and the old lines it opens */
  const told = (notes: RenameNotes) => [...notes.byBlock].map(([at, note]) => [at, note.key, note.renames.map(it => `${it.old}>${it.new}`).join(" "), note.lines, note.hides])
  /** The rows of block `index` of a diff in a layout: a note by its kind, whether it opens and the names it names, a line by its kind, its lines and its tint */
  const rowsOf = (diff: TextDiff, index: number, mode: DiffViewMode, open: number[] = [], renames = renameNotesOf(diff), heading: PartsOptions["heading"] = null) => {
    const parts = blockParts(diff.blocks[index]!, index, marksByBlock(diff)[index]!, { ...options(mode, open), heading, renames })
    return Array.from({ length: parts.rowCount }, (_, row) => {
      const value = partsRow(parts, row)
      if (value.kind === "note") {
        const { remark } = value
        const names = remark?.renames == null ? "" : ` ${remark.renames.renames.map(it => `${it.old}>${it.new}`).join(" ")}`
        const said = remark === null ? "" : ` ${remark.kind}${remark.open === null ? "" : remark.open ? " open" : " closed"}${remark.quiet ? " quiet" : ""}${names}`
        return `${value.tone}${value.label === "" ? "" : ` ${value.label}`}${said} ${value.lines.map(([start, end]) => end - start).join("+")}`
      }
      return `${value.kind} ${value.lines.map(line => line ?? "_").join(",")}${value.tint === "full" ? "" : ` ${value.tint}`}${value.run === null ? "" : ` ${value.run}`}`
    })
  }

  it("gives a rename one note, at its declaration, with the old lines of its quiet runs to open; its runs have no note of their own", () => {
    const quiet = local(true)
    const notes = renameNotesOf(quiet)
    expect(told(notes)).toEqual([
      [
        0,
        3,
        "total>sum",
        [3, 3],
        [
          [3, 4],
          [6, 7],
          [9, 10],
        ],
      ],
    ])
    // each run of the rename is named by that note
    const [note] = [...notes.byBlock.values()]
    expect(quiet.marks.runs.map(one => notes.ofRun.get(one))).toEqual([note, note, note])
    // a label hides no line: its note opens nothing
    expect(told(renameNotesOf(local(false)))).toEqual([[0, 3, "total>sum", [3, 3], []]])
    // the declaration is the place of the note, wherever it is among the sites
    expect(told(renameNotesOf(local(true, [1]))).map(([at, key]) => [at, key])).toEqual([[1, 6]])
    // a diff that renames nothing has no note, and costs nothing
    expect(renameNotesOf(textDiff(20, 20, blocks()))).toBe(NO_RENAME_NOTES)
  })

  it("stands the note at the first site the rows show where they do not show the declaration, and opens the runs they show alone", () => {
    const quiet = local(true)
    // a section of a review that shows the last two blocks: the note is at the first of them
    expect(told(renameNotesOf(quiet, index => index !== 0))).toEqual([
      [
        1,
        6,
        "total>sum",
        [6, 6],
        [
          [6, 7],
          [9, 10],
        ],
      ],
    ])
    expect(told(renameNotesOf(quiet, index => index === 2)).map(([at, key]) => [at, key])).toEqual([[2, 9]])
    // rows that show no site of the rename name it nowhere
    expect(told(renameNotesOf(quiet, () => false))).toEqual([])
    // a block that is played down (the imports alone, in a review with its plan) has the note only when no other block can
    expect(told(renameNotesOf(local(true, [1]), () => true, new Set([1]))).map(([at, key]) => [at, key])).toEqual([[0, 3]])
    expect(told(renameNotesOf(quiet, () => true, new Set([0, 1, 2]))).map(([at, key]) => [at, key])).toEqual([[0, 3]])
  })

  // `count` is `n` on the line of the declaration of `total`, after it; `flag` is `on` in the last block alone, on a
  // line that holds a site of `total` too
  const three = textDiff(20, 20, blocks(), {
    runs: renamedRuns(true),
    renames: [rename("count", "n", [[3, 12, 3, 10]]), rename("total", "sum", sites), rename("flag", "on", [[9, 2, 9, 2]])],
  })
  /** A run of two lines of the name `total`, a line each on both sides: its places, and the one of them that declares it */
  const twoLines = (places: readonly (readonly [number, number, number, number])[], declared: number) =>
    textDiff(20, 20, [block(3, 5, 3, 5)], { runs: [run(3, 5, 3, 5, { kind: "renamed" })], renames: [rename("total", "sum", places, { declared: [declared] })] })

  it("names the renames whose notes stand in one block in one note, at the first of their sites, and any other in its own", () => {
    const notes = renameNotesOf(three)
    // the run of the last block holds a site of both notes: it is the run of the note of its first name, `flag`, alone
    expect(told(notes)).toEqual([
      [
        0,
        3,
        "total>sum count>n",
        [3, 3],
        [
          [3, 4],
          [6, 7],
        ],
      ],
      [2, 9, "flag>on", [9, 9], [[9, 10]]],
    ])
    const [shared, own] = [...notes.byBlock.values()]
    expect(three.marks.runs.map(one => notes.ofRun.get(one))).toEqual([shared, shared, own])
  })

  it("opens a quiet run by one note alone, the note of the first rename with a site in it; a note with no run of its own is plain words", () => {
    // the run of the last block opens with the note of `flag`, never with the note of `total`, which has a site in it too
    expect(rowsOf(three, 2, "inline")).toEqual(["remark renamed closed quiet flag>on 0+0", "once _,9 quiet renamed"])
    expect(rowsOf(three, 2, "inline", [3])).toEqual(["remark renamed closed quiet flag>on 0+0", "once _,9 quiet renamed"])
    expect(rowsOf(three, 2, "inline", [9])).toEqual(["remark renamed open quiet flag>on 0+0", "deleted 9,_ quiet renamed", "added _,9 quiet renamed"])
    // so a note that says it is closed hides a line, and one that says it is open shows every line it names
    expect(rowsOf(three, 0, "inline", [9])).toEqual(["remark renamed closed quiet total>sum count>n 0+0", "once _,3 quiet renamed"])
    expect(rowsOf(three, 0, "inline", [3])).toEqual(["remark renamed open quiet total>sum count>n 0+0", "deleted 3,_ quiet renamed", "added _,3 quiet renamed"])
    // a rename declared on a line that is read, whose one quiet line is the run of another note: its note opens nothing
    const read = textDiff(20, 20, blocks(), {
      runs: [run(3, 4, 3, 4, { kind: "renamed", quiet: false }), run(9, 10, 9, 10, { kind: "renamed", quiet: true })],
      renames: [
        rename("total", "sum", [
          [3, 4, 3, 4],
          [9, 9, 9, 9],
        ]),
        rename("flag", "on", [[9, 2, 9, 2]]),
      ],
    })
    expect(told(renameNotesOf(read)).map(([at, key, , , hides]) => [at, key, hides])).toEqual([
      [0, 3, []],
      [2, 9, [[9, 10]]],
    ])
    expect(rowsOf(read, 0, "inline", [3])).toEqual(["remark renamed total>sum 0+0", "inline 3,3"])
    // the rows of a quiet run answer the key of the notes as its one note does, closed and opened; lines that are read answer none
    const parts = (index: number, open: number[] = []) =>
      blockParts(read.blocks[index]!, index, marksByBlock(read)[index]!, { ...options("inline", open), renames: renameNotesOf(read) })
    expect([partsNoteOfRow(parts(2), 0), partsNoteOfRow(parts(2), 1), partsNoteOfRow(parts(2, [9]), 1), partsNoteOfRow(parts(2, [9]), 2), partsNoteOfRow(parts(0), 1)]).toEqual([
      null,
      9,
      9,
      9,
      null,
    ])
  })

  it("shows a run of a rename that is a label as the lines of its block, the note before the lines of the declaration alone", () => {
    const label = local(false)
    // Inline: the pair of each line is one row; Unified and Split show both lines; the note opens nothing
    expect([rowsOf(label, 0, "inline"), rowsOf(label, 1, "inline")]).toEqual([["remark renamed total>sum 0+0", "inline 3,3"], ["inline 6,6"]])
    expect([rowsOf(label, 0, "unified"), rowsOf(label, 2, "unified")]).toEqual([
      ["remark renamed total>sum 0+0", "deleted 3,_", "added _,3"],
      ["deleted 9,_", "added _,9"],
    ])
    expect([rowsOf(label, 0, "split"), rowsOf(label, 1, "split")]).toEqual([["remark renamed total>sum 0+0", "changed 3,3"], ["changed 6,6"]])
    // nothing of a label opens, and its lines are read: the reading of each block starts at its line
    const parts = (index: number) => blockParts(label.blocks[index]!, index, marksByBlock(label)[index]!, { ...options("inline"), renames: renameNotesOf(label) })
    expect([partsNotes(parts(0)), partsStopRow(parts(0), NEW, 3), partsStopRow(parts(1), NEW, 6)]).toEqual([[], 1, 0])
  })

  it("shows a quiet run of a rename once in Inline, without a note of its own, until the note of the rename is opened; Unified and Split keep both lines", () => {
    const quiet = local(true)
    expect([rowsOf(quiet, 0, "inline"), rowsOf(quiet, 1, "inline"), rowsOf(quiet, 2, "inline")]).toEqual([
      ["remark renamed closed quiet total>sum 0+0", "once _,3 quiet renamed"],
      ["once _,6 quiet renamed"],
      ["once _,9 quiet renamed"],
    ])
    // the one note opens the old lines of every run of the rename, in its block and in the others
    expect([rowsOf(quiet, 0, "inline", [3]), rowsOf(quiet, 2, "inline", [3])]).toEqual([
      ["remark renamed open quiet total>sum 0+0", "deleted 3,_ quiet renamed", "added _,3 quiet renamed"],
      ["deleted 9,_ quiet renamed", "added _,9 quiet renamed"],
    ])
    expect([rowsOf(quiet, 0, "unified"), rowsOf(quiet, 1, "split")]).toEqual([
      ["remark renamed quiet total>sum 0+0", "deleted 3,_ quiet renamed", "added _,3 quiet renamed"],
      ["changed 6,6 quiet renamed"],
    ])
    // the note that opens: its key, its row and every line it hides; a block of the rename without the note has none
    const parts = (index: number, open: number[] = []) =>
      blockParts(quiet.blocks[index]!, index, marksByBlock(quiet)[index]!, { ...options("inline", open), renames: renameNotesOf(quiet) })
    expect([partsNotes(parts(0)), partsNotes(parts(1))]).toEqual([
      [
        {
          key: 3,
          lines: [
            [3, 4],
            [6, 7],
            [9, 10],
          ],
          row: 0,
        },
      ],
      [],
    ])
    const [note] = parts(0).parts
    expect(note?.kind === "note" && note.remark !== null ? noteKey(note.remark) : null).toBe(3)
    // an old line Inline shows once is found at the row of its new line; opened, at its own row
    expect([partsRowOfLine(parts(0), OLD, 3), partsRowOfLine(parts(1), OLD, 6), partsRowOfLine(parts(0, [3]), OLD, 3), partsRowOfLine(parts(1, [3]), NEW, 6)]).toEqual([1, 0, 1, 1])
    // a run wrapped otherwise (one old line, two new ones, the name on the first): its old line at the first row of its lines
    const wrapped = textDiff(20, 21, [block(3, 4, 3, 5)], { runs: [run(3, 4, 3, 5, { kind: "renamed" })], renames: [rename("total", "sum", [[3, 9, 3, 9]])] })
    const rewrapped = blockParts(wrapped.blocks[0]!, 0, marksByBlock(wrapped)[0]!, { ...options("inline"), renames: renameNotesOf(wrapped) })
    expect([partsRowOfLine(rewrapped, OLD, 3), partsRowOfLine(rewrapped, NEW, 4)]).toEqual([1, 2])
    // a run of two lines of the name, each with its new line: each old line at the row of its own new line
    const two = twoLines(
      [
        [3, 4, 3, 4],
        [4, 2, 4, 2],
      ],
      0,
    )
    const both = blockParts(two.blocks[0]!, 0, marksByBlock(two)[0]!, { ...options("inline"), renames: renameNotesOf(two) })
    expect([partsRowOfLine(both, OLD, 3), partsRowOfLine(both, OLD, 4), partsRowOfLine(both, NEW, 4)]).toEqual([1, 2, 2])
  })

  it("stands the note before the lines that hold its site: after a run that comes first, in the row of the heading where those lines start the block", () => {
    // a call wrapped (a quiet run of formatting), then the declaration of the name on a line that changed otherwise too
    // (no run), then a line of the name alone
    const mixed = textDiff(20, 22, [block(2, 5, 2, 7)], {
      runs: [run(2, 3, 2, 5), run(4, 5, 6, 7, { kind: "renamed", quiet: true })],
      renames: [
        rename("total", "sum", [
          [3, 4, 5, 4],
          [4, 0, 6, 0],
        ]),
      ],
    })
    expect(rowsOf(mixed, 0, "inline")).toEqual([
      "remark formatting closed quiet 1+0",
      "once _,2 quiet formatting",
      "once _,3 quiet formatting",
      "once _,4 quiet formatting",
      "remark renamed closed total>sum 0+0",
      "deleted 3,_",
      "added _,5",
      "once _,6 quiet renamed",
    ])
    // the note of the rename opens its run, the note of the formatting its own: each by its key
    expect(rowsOf(mixed, 0, "inline", [3]).slice(4)).toEqual([
      "remark renamed open total>sum 0+0",
      "deleted 3,_",
      "added _,5",
      "deleted 4,_ quiet renamed",
      "added _,6 quiet renamed",
    ])
    // the heading of a change is the first row of its block: the note joins it only where its lines start the block
    const heading = { parts: [{ name: "balance", words: "body changed" }], label: "balance — body changed" }
    expect(rowsOf(mixed, 0, "unified", [], renameNotesOf(mixed), heading)[0]).toBe("unit balance — body changed formatting quiet 0+0")
    const label = local(false)
    expect(rowsOf(label, 0, "unified", [], renameNotesOf(label), heading)).toEqual(["unit balance — body changed renamed total>sum 0+0", "deleted 3,_", "added _,3"])
  })

  it("cuts the lines that hold the site of the note at the site, each side at its line, so the note is the row right above it", () => {
    // five changed lines of a function with no run; the declaration of the name is the fourth line of both sides
    const site = [[6, 4, 6, 4]] as const
    const paired = textDiff(20, 20, [block(3, 8, 3, 8, { inlinePairs: true })], { renames: [rename("total", "sum", site)] })
    expect(rowsOf(paired, 0, "inline")).toEqual(["inline 3,3", "inline 4,4", "inline 5,5", "remark renamed total>sum 0+0", "inline 6,6", "inline 7,7"])
    expect(rowsOf(paired, 0, "split")).toEqual(["changed 3,3", "changed 4,4", "changed 5,5", "remark renamed total>sum 0+0", "changed 6,6", "changed 7,7"])
    // lines the engine does not pair: Unified shows what stands before the site on both sides, the note, then the lines from it
    const stacked = textDiff(20, 20, [block(3, 8, 3, 7)], { renames: [rename("total", "sum", [[6, 4, 5, 4]])] })
    expect(rowsOf(stacked, 0, "unified")).toEqual([
      "deleted 3,_",
      "deleted 4,_",
      "deleted 5,_",
      "added _,3",
      "added _,4",
      "remark renamed total>sum 0+0",
      "deleted 6,_",
      "deleted 7,_",
      "added _,5",
      "added _,6",
    ])
    // Split: the two lines of the site in one row, under the note
    expect(rowsOf(stacked, 0, "split")).toEqual(["changed 3,3", "changed 4,4", "changed 5,_", "remark renamed total>sum 0+0", "changed 6,5", "changed 7,6"])
    // every line of the block has its row, and the row of a line is the same line
    const parts = blockParts(stacked.blocks[0]!, 0, marksByBlock(stacked)[0]!, { ...options("unified"), renames: renameNotesOf(stacked) })
    expect([3, 5, 6, 7].map(line => partsRowOfLine(parts, OLD, line))).toEqual([0, 2, 6, 7])
    expect([3, 4, 5, 6].map(line => partsRowOfLine(parts, NEW, line))).toEqual([3, 4, 8, 9])
    // the reading of the block starts at its first row, above the note
    expect(partsStopRow(parts, NEW, 3)).toBe(0)
    // a quiet run of two lines of the name, the declaration on the second: Inline shows the first once, then the note
    const two = twoLines(
      [
        [3, 9, 3, 9],
        [4, 4, 4, 4],
      ],
      1,
    )
    expect(rowsOf(two, 0, "inline")).toEqual(["once _,3 quiet renamed", "remark renamed closed quiet total>sum 0+0", "once _,4 quiet renamed"])
    expect(rowsOf(two, 0, "inline", [4])).toEqual([
      "deleted 3,_ quiet renamed",
      "added _,3 quiet renamed",
      "remark renamed open quiet total>sum 0+0",
      "deleted 4,_ quiet renamed",
      "added _,4 quiet renamed",
    ])
    // a site on the first line of its lines cuts nothing: the note is before them, as it was
    const first = textDiff(20, 20, [block(3, 8, 3, 8, { inlinePairs: true })], { renames: [rename("total", "sum", [[3, 4, 3, 4]])] })
    expect(rowsOf(first, 0, "inline").slice(0, 2)).toEqual(["remark renamed total>sum 0+0", "inline 3,3"])
    // the old line of the site stands before lines the new side has not: those old lines stay above the note
    const late = textDiff(20, 20, [block(3, 6, 3, 4)], { renames: [rename("total", "sum", [[5, 4, 3, 4]])] })
    expect(rowsOf(late, 0, "unified")).toEqual(["deleted 3,_", "deleted 4,_", "remark renamed total>sum 0+0", "deleted 5,_", "added _,3"])
    // Inline shows a quiet run by its new lines: a site on its first new line is under the note, whatever old lines come before it
    const wrapped = textDiff(20, 20, [block(3, 5, 3, 4)], { runs: [run(3, 5, 3, 4, { kind: "renamed" })], renames: [rename("total", "sum", [[4, 4, 3, 9]])] })
    expect(rowsOf(wrapped, 0, "inline")).toEqual(["remark renamed closed quiet total>sum 0+0", "once _,3 quiet renamed"])
  })

  it("shows the lines of a pair apart where a renamed name stands on two lines the engine does not pair, so both lines mark it", () => {
    // `val a = total` / `  + 1` is `val a =` / `  sum + 1`: the engine pairs line i with line i, the name moved a line down
    const moved = textDiff(20, 20, [block(3, 5, 3, 5, { inlinePairs: true }), block(9, 10, 9, 10, { inlinePairs: true })], {
      renames: [
        rename("total", "sum", [
          [3, 8, 4, 2],
          [9, 0, 9, 0],
        ]),
      ],
    })
    expect(marksByBlock(moved).map(marks => marks.apart)).toEqual([[[3, 4]], []])
    // Inline: no row of a pair; the new line before the name, the note, then the old lines from the one with the name and
    // the new line with it. The block whose name stays on its line keeps its row
    expect(rowsOf(moved, 0, "inline")).toEqual(["added _,3", "remark renamed total>sum 0+0", "deleted 3,_", "deleted 4,_", "added _,4"])
    expect(rowsOf(moved, 0, "unified")).toEqual(rowsOf(moved, 0, "inline"))
    expect(rowsOf(moved, 1, "inline")).toEqual(["inline 9,9"])
    expect(inlinePairsOf(moved.blocks[0]!, 0, marksByBlock(moved)[0]!)).toEqual([])
    expect(inlinePairsOf(moved.blocks[1]!, 1, marksByBlock(moved)[1]!).map(pair => pair.lines)).toEqual([[9, 9]])
    // a view that plays a block down keeps what it knows of the names of the block
    expect(drawnMarks(marksByBlock(moved)[0]!, 0, new Set([1])).apart).toEqual([[3, 4]])
    // a block the engine does not pair has no pair to take apart
    const stacked = textDiff(20, 20, [block(3, 5, 3, 5)], { renames: [rename("total", "sum", [[3, 8, 4, 2]])] })
    expect(marksByBlock(stacked)[0]!.apart).toEqual([])
  })

  it("shows every line of a block once whatever sites and runs of renames it holds, by its own row or by the new lines of a quiet run of Inline, the note right above the new line of its site", () => {
    // the same blocks, runs and sites in every run of the test
    const random = seeded(20261112)
    // what the rounds held, so the test says when its marks stop being what it is about
    const held = { cut: 0, apart: 0, quiet: 0 }
    for (let round = 0; round < 600; round++) {
      const [oldCount, newCount] = [1 + random(7), 1 + random(7)]
      const one = block(10, 10 + oldCount, 10, 10 + newCount, { inlinePairs: oldCount === newCount && random(2) === 0 })
      // a run of the kind `renamed` somewhere in the block, quiet or a label, or none
      const from = [random(oldCount), random(newCount)] as const
      const runs =
        random(3) === 0
          ? []
          : [
              run(10 + from[0], 10 + from[0] + 1 + random(oldCount - from[0]), 10 + from[1], 10 + from[1] + 1 + random(newCount - from[1]), {
                kind: "renamed",
                quiet: random(2) === 0,
              }),
            ]
      // one or two renames, each with places at lines of the block in the order of both sides, valid or not
      const renames = Array.from({ length: 1 + random(2) }, (_, at) => {
        const olds = Array.from({ length: 1 + random(3) }, () => 10 + random(oldCount)).toSorted((a, b) => a - b)
        const news = olds.map(() => 10 + random(newCount)).toSorted((a, b) => a - b)
        return rename(
          `old${at}`,
          `new${at}`,
          olds.map((line, place) => [line, 4 + 10 * at, news[place]!, 4 + 10 * at] as const),
          { declared: [random(olds.length)] },
        )
      })
      const diff = textDiff(60, 60, [one], { runs, renames })
      const marks = marksByBlock(diff)[0]!
      const notes = renameNotesOf(diff)
      held.apart += marks.apart.length > 0 ? 1 : 0
      const key = [...notes.byBlock.values()][0]!.key
      for (const mode of ["inline", "unified", "split"] as const) {
        for (const open of [[], [key]]) {
          const parts = blockParts(one, 0, marks, { ...options(mode, open), renames: notes })
          const shown = [new Map<number, number>(), new Map<number, number>()]
          let noteRow = -1
          let once = 0
          for (let row = 0; row < parts.rowCount; row++) {
            const value = partsRow(parts, row)
            if (value.kind === "note") {
              // the note of the renames; a run that holds no site of them has the note of its kind
              noteRow = value.remark?.renames == null ? noteRow : row
              continue
            }
            once += value.kind === "once" ? 1 : 0
            for (const side of [OLD, NEW]) {
              const line = value.lines[side]
              if (line != null) {
                shown[side]!.set(line, (shown[side]!.get(line) ?? 0) + 1)
              }
            }
          }
          const context = `round ${round} ${mode} ${open.length === 0 ? "closed" : "open"}`
          held.cut += noteRow > 0 ? 1 : 0
          held.quiet += once > 0 ? 1 : 0
          // one note, and every new line once
          expect(noteRow, context).toBeGreaterThanOrEqual(0)
          expect(
            [...shown[NEW]!].toSorted((a, b) => a[0] - b[0]),
            context,
          ).toEqual(Array.from({ length: newCount }, (_, at) => [10 + at, 1]))
          // every old line once, but the lines of a quiet run that Inline shows by its new lines
          const hidden = (line: number) => once > 0 && runs.some(it => it.oldStart <= line && line < it.oldEnd)
          for (let line = 10; line < 10 + oldCount; line++) {
            expect(shown[OLD]!.get(line) ?? 0, `${context} old ${line}`).toBe(hidden(line) ? 0 : 1)
          }
          // the row of a new line is where it shows, and the note is the row right above the new line of its site
          for (let line = 10; line < 10 + newCount; line++) {
            const value = partsRow(parts, partsRowOfLine(parts, NEW, line))
            expect(value.kind !== "note" && value.lines[NEW] === line, `${context} new ${line}`).toBe(true)
          }
          // the first new line under the note is the new line of its site: no other new line stands between them
          const site = [...notes.byBlock.values()][0]!.lines
          let under = noteRow + 1
          while (under < parts.rowCount && partsRow(parts, under).lines[NEW] == null) {
            under++
          }
          expect(under < parts.rowCount ? partsRow(parts, under).lines[NEW] : null, `${context} under the note`).toBe(site[1])
        }
      }
    }
    // the rounds held notes inside the lines of a block, names on two lines of no pair, and lines shown once
    expect([held.cut > 100, held.apart > 20, held.quiet > 50], JSON.stringify(held)).toEqual([true, true, true])
  })

  it("keeps the note of a run of a renamed name that the marks name no rename for, which opens it as any quiet run", () => {
    const unnamed = textDiff(20, 20, [block(3, 4, 3, 4)], { runs: renamedRuns(true).slice(0, 1) })
    expect(rowsOf(unnamed, 0, "inline")).toEqual(["remark renamed closed quiet 1+0", "once _,3 quiet renamed"])
    expect(rowsOf(unnamed, 0, "inline", [3])).toEqual(["remark renamed open quiet 0+0", "deleted 3,_ quiet renamed", "added _,3 quiet renamed"])
    // and so do the rows that name no rename (the pairs of the widest row of Inline)
    const quiet = local(true)
    expect(rowsOf(quiet, 1, "inline", [], NO_RENAME_NOTES)).toEqual(["remark renamed closed quiet 1+0", "once _,6 quiet renamed"])
  })
})

describe("the ends of the moves between two files of a subject", () => {
  const rowsOf = moveRowsOf
  // a blank line and the four lines of a function leave the end of the ledger (the old lines 53..58): the four lines
  // stand in the report, file 1 of the subject, from its line 17
  const left = block(53, 58, 44, 44)
  const ledger = textDiff(58, 44, [left], { subject: subjectMarks({ movedOut: [fileMove(54, 4, 1, 17)] }) })
  // the report adds a blank line and those four lines after its last line: they stood in the ledger, file 0, from its line 54
  const stands = block(16, 16, 16, 21)
  const report = textDiff(16, 21, [stands], { subject: subjectMarks({ movedIn: [fileMove(17, 4, 0, 54)] }) })

  it("gives the block of the lines an end of the move, which names the file and the lines of the other end; none for lines no block holds whole", () => {
    const [from] = marksByBlock(ledger)
    expect(from!.sources.map(end => [end.at, end.lines, end.other, end.otherBlock, end.file])).toEqual([["source", [54, 58], [17, 21], -1, 1]])
    const [to] = marksByBlock(report)
    expect(to!.destinations.map(end => [end.at, end.lines, end.other, end.otherBlock, end.file])).toEqual([["destination", [17, 21], [54, 58], -1, 0]])
    // the move as a move within a file is said: the old lines of the file they left, the new lines of the file they
    // stand in, exact lines
    expect([from!.sources[0]!.move, to!.destinations[0]!.move]).toEqual([move(54, 17, 4), move(54, 17, 4)])
    // lines that reach out of their block, lines of no block, no line, and lines of a block the policy ignores
    for (const stray of [fileMove(55, 4, 1, 17), fileMove(10, 2, 1, 17), fileMove(54, 0, 1, 17), fileMove(30, 1, 1, 17)]) {
      const marks = marksByBlock(textDiff(58, 44, [block(30, 31, 30, 31, { kind: "ignored" }), left], { subject: subjectMarks({ movedOut: [stray] }) }))
      expect(marks.map(it => it.sources.length)).toEqual([0, 0])
    }
    // beside a move within the file, the ends of a side of a block in the order of its lines
    const both = textDiff(40, 40, [block(5, 5, 5, 9), block(20, 26, 24, 24)], { moves: [move(24, 5, 2)], subject: subjectMarks({ movedOut: [fileMove(20, 3, 2, 7)] }) })
    expect(marksByBlock(both)[1]!.sources.map(end => [end.lines, end.file])).toEqual([
      [[20, 23], 2],
      [[24, 26], null],
    ])
    // a declaration the index says moved whole: its name, and the lines of the other end in its layout there
    const declared = textDiff(58, 44, [left], { subject: subjectMarks({ movedOut: [{ ...fileMove(54, 4, 1, 17), declaration: { name: "reportTitle", otherLen: 7 } }] }) })
    expect(marksByBlock(declared)[0]!.sources.map(end => [end.lines, end.other, end.file, end.declaration])).toEqual([[[54, 58], [17, 24], 1, "reportTitle"]])
    expect(from!.sources[0]!.declaration).toBeUndefined()
    // a diff of no subject, and a subject that says nothing of the file, add no end
    expect([marksByBlock(textDiff(58, 44, [left]))[0]!.sources, marksByBlock(textDiff(58, 44, [left], { subject: subjectMarks() }))[0]!.sources]).toEqual([[], []])
  })

  it("shows every line of both ends in every layout, Inline too, after a note that opens nothing", () => {
    const from = marksByBlock(ledger)[0]!
    // where the lines left: the blank line of no move, the note, the four lines; in one column and in two
    const stacked = ["deleted 53,_", "note moved 54-54 source file 1", "deleted 54,_ source", "deleted 55,_ source", "deleted 56,_ source", "deleted 57,_ source"]
    expect(rowsOf(blockParts(left, 0, from, options("inline")))).toEqual(stacked)
    expect(rowsOf(blockParts(left, 0, from, options("unified")))).toEqual(stacked)
    expect(rowsOf(blockParts(left, 0, from, options("split")))).toEqual(stacked.map(row => row.replace("deleted", "changed")))
    // a note of Inline that hides nothing is no note that opens, whatever the open notes say
    const inline = blockParts(left, 0, from, options("inline", [54]))
    expect([rowsOf(inline), partsNotes(inline), partsNoteOfRow(inline, 3)]).toEqual([stacked, [], null])
    // every old line has its own row: none is found at the note
    expect([53, 54, 57, 58].map(line => partsRowOfLine(inline, OLD, line))).toEqual([0, 2, 5, 6])
    // where they stand now: the same, by the new lines
    const to = marksByBlock(report)[0]!
    const added = ["added _,16", "note moved 16-16 destination file 0", "added _,17 destination", "added _,18 destination", "added _,19 destination", "added _,20 destination"]
    expect(rowsOf(blockParts(stands, 0, to, options("inline")))).toEqual(added)
    expect(rowsOf(blockParts(stands, 0, to, options("unified")))).toEqual(added)
    expect(rowsOf(blockParts(stands, 0, to, options("split")))).toEqual(added.map(row => row.replace("added", "changed")))
    // the notes of both ends are notes of a move: what the key of the moves takes, and where a jump lands
    expect([
      partsMoveNotes(inline).map(note => [note.end.at, note.end.file, note.row]),
      partsMoveNotes(blockParts(stands, 0, to, options("inline"))).map(note => note.row),
    ]).toEqual([[["source", 1, 1]], [1]])
  })

  it("reads the lines that left for another file: the reading of their block starts at its first line, a moved one too", () => {
    // Rust takes no start for a move between files: the block is read from its first old line, the blank one
    expect(ledger.marks.starts).toEqual([0, 0, 53])
    const from = marksByBlock(ledger)[0]!
    for (const mode of ["inline", "unified", "split"] as const) {
      expect(partsStopRow(blockParts(left, 0, from, options(mode)), OLD, 53)).toBe(0)
    }
    // a block that is the moved lines alone: the stop is the first of them, the row under the note
    const whole = textDiff(58, 54, [block(54, 58, 54, 54)], { subject: subjectMarks({ movedOut: [fileMove(54, 4, 1, 17)] }) })
    const parts = blockParts(whole.blocks[0]!, 0, marksByBlock(whole)[0]!, options("inline"))
    expect([whole.marks.starts, partsStopRow(parts, OLD, 54), rowsOf(parts)[0]]).toEqual([[0, 0, 54], 1, "note moved 54-54 source file 1"])
    // lines that left for another file before a line that changed: Rust names the new line, and the block is read
    // from the first of the lines that left, the row under their note; the same lines taken by a move within the
    // file end what is read before them, and the block is read from the line after them
    const mixed = block(20, 23, 20, 21)
    const across = textDiff(40, 38, [mixed], { subject: subjectMarks({ movedOut: [fileMove(20, 2, 1, 7)] }) })
    const inside = textDiff(40, 40, [block(5, 5, 5, 7), block(22, 25, 22, 23)], { moves: [move(22, 5, 2)] })
    expect([across.marks.starts, inside.marks.starts]).toEqual([
      [0, 1, 20],
      [0, 1, 5, 1, 1, 22],
    ])
    const read = blockParts(mixed, 0, marksByBlock(across)[0]!, options("unified"))
    expect([rowsOf(read), partsStopRow(read, NEW, 20)]).toEqual([["note moved 20-20 source file 1", "deleted 20,_ source", "deleted 21,_ source", "deleted 22,_", "added _,20"], 1])
    const passed = blockParts(inside.blocks[1]!, 1, marksByBlock(inside)[1]!, options("unified"))
    expect([rowsOf(passed), partsStopRow(passed, NEW, 22)]).toEqual([["note moved 22-22 source", "deleted 22,_ source", "deleted 23,_ source", "deleted 24,_", "added _,22"], 3])
  })

  it("draws no end of a move between two files in a block a view plays down, and every end elsewhere", () => {
    const diff = textDiff(40, 40, [block(2, 4, 2, 2), block(20, 24, 22, 22)], { subject: subjectMarks({ movedOut: [fileMove(2, 2, 1, 7), fileMove(20, 4, 1, 30)] }) })
    const marks = marksByBlock(diff)
    const played = new Set([0])
    expect([drawnMarks(marks[0]!, 0, played).sources, drawnMarks(marks[1]!, 1, played)]).toEqual([[], marks[1]])
    expect(drawnMarks(marks[0]!, 0, new Set())).toBe(marks[0])
  })

  it("cuts the lines of a block at an end of a move between two files as at an end of any move: each side up to it, the note, its lines, then the rest", () => {
    // five old lines and three new ones; the old lines 21 and 22 stand in another file now
    const changed = block(20, 25, 20, 23)
    const diff = textDiff(40, 38, [changed], { subject: subjectMarks({ movedOut: [fileMove(21, 2, 3, 0)] }) })
    expect(rowsOf(blockParts(changed, 0, marksByBlock(diff)[0]!, options("unified")))).toEqual([
      "deleted 20,_",
      "added _,20",
      "added _,21",
      "added _,22",
      "note moved 21-21 source file 3",
      "deleted 21,_ source",
      "deleted 22,_ source",
      "deleted 23,_",
      "deleted 24,_",
    ])
    // every line of the block has one row, in every layout
    for (const mode of ["inline", "unified", "split"] as const) {
      const parts = blockParts(changed, 0, marksByBlock(diff)[0]!, options(mode))
      const lines = Array.from({ length: parts.rowCount }, (_, row) => partsRow(parts, row)).flatMap(row => (row.kind === "note" ? [] : [row.lines]))
      expect([lines.flatMap(([line]) => line ?? []).toSorted((a, b) => a - b), lines.flatMap(([, line]) => line ?? []).toSorted((a, b) => a - b)]).toEqual([
        [20, 21, 22, 23, 24],
        [20, 21, 22],
      ])
    }
  })
})

describe("a quiet run the index proved after the rows were shown", () => {
  // two calls of a function another file declares, each on a line of its own block: runs of the kind `renamed`
  const blocks = () => [block(4, 5, 4, 5, { inlinePairs: true }), block(10, 11, 10, 11, { inlinePairs: true })]
  const used = rename(
    "openStore",
    "connectStore",
    [
      [4, 16, 4, 16],
      [10, 16, 10, 16],
    ],
    { local: false, declared: [] },
  )
  const runs = (quiet: boolean) => [run(4, 5, 4, 5, { kind: "renamed", quiet }), run(10, 11, 10, 11, { kind: "renamed", quiet })]
  const proofs = (proof: "asked" | "confirmed", quiet: boolean, confirmed: number[]) =>
    textDiff(20, 20, blocks(), { runs: runs(quiet), subject: subjectMarks({ used: [{ rename: used, file: 2, line: 18, proof }], confirmed }) })
  /** The lines of the rows of a block of a diff in a layout: each by its kind, its lines and its tint; a note as "note" */
  const linesOf = (diff: TextDiff, index: number, mode: DiffViewMode) => {
    const parts = blockParts(diff.blocks[index]!, index, marksByBlock(diff)[index]!, { ...options(mode), renames: renameNotesOf(diff) })
    return Array.from({ length: parts.rowCount }, (_, row) => {
      const value = partsRow(parts, row)
      return value.kind === "note"
        ? `note${value.remark?.open == null ? "" : " opens"}${value.remark?.quiet === true ? " quiet" : ""}`
        : `${value.kind} ${value.lines.map(line => line ?? "_").join(",")} ${value.tint}`
    })
  }

  it("keeps every line of the run in every layout, played down, with no note that opens: the row of the pair in Inline too", () => {
    const asked = proofs("asked", false, [])
    // the answer of the index, the kind switched on: the two runs quiet, and listed as confirmed
    const confirmed = proofs("confirmed", true, [0, 1])
    expect(marksByBlock(confirmed).map(it => [...it.kept])).toEqual([confirmed.marks.runs, confirmed.marks.runs])
    for (const mode of ["inline", "unified", "split"] as const) {
      // the same rows of the same lines before and after the answer: the tint alone differs
      const before = [linesOf(asked, 0, mode), linesOf(asked, 1, mode)]
      const after = [linesOf(confirmed, 0, mode), linesOf(confirmed, 1, mode)]
      expect(after).toEqual(before.map(rows => rows.map(row => row.replace(/ (full|pale)$/, " quiet").replace(/^note$/, "note quiet"))))
    }
    expect(linesOf(confirmed, 0, "inline")).toEqual(["note quiet", "inline 4,4 quiet"])
    expect(linesOf(confirmed, 1, "inline")).toEqual(["inline 10,10 quiet"])
    // no note opens them, and the one of the rename hides none of their lines
    const notes = renameNotesOf(confirmed)
    expect([
      [...notes.byBlock.values()].map(note => note.hides),
      partsNotes(blockParts(confirmed.blocks[0]!, 0, marksByBlock(confirmed)[0]!, { ...options("inline"), renames: notes })),
    ]).toEqual([[[]], []])
    // the pair of each run is a row of two lines whatever notes are open: the widest row of Inline counts it
    expect(inlinePairsOf(confirmed.blocks[1]!, 1, marksByBlock(confirmed)[1]!).map(pair => pair.lines)).toEqual([[10, 10]])
  })

  it("shows a quiet run of the same kind once in Inline where the index did not prove it late: the pair says it with the content", () => {
    // the same runs quiet without the list of the subject (a local the scopes of the file prove): Inline shows each once
    const local = textDiff(20, 20, blocks(), {
      runs: runs(true),
      renames: [
        rename("total", "sum", [
          [4, 16, 4, 16],
          [10, 16, 10, 16],
        ]),
      ],
    })
    expect([linesOf(local, 0, "inline"), linesOf(local, 1, "inline")]).toEqual([["note opens quiet", "once _,4 quiet"], ["once _,10 quiet"]])
    // a number of the list that names no run keeps nothing
    const stray = textDiff(20, 20, blocks(), { runs: runs(true), subject: subjectMarks({ confirmed: [7] }) })
    expect(marksByBlock(stray)[0]!.kept.size).toBe(0)
  })
})

describe("the notes of the renames another file of the subject declares", () => {
  const blocks = () => [block(4, 5, 4, 5, { inlinePairs: true }), block(10, 11, 10, 11, { inlinePairs: true }), block(14, 15, 14, 15, { inlinePairs: true })]
  const sites = [
    [4, 16, 4, 16],
    [10, 16, 10, 16],
  ] as const
  const open = rename("openStore", "connectStore", sites, { local: false, declared: [] })
  const labels = [run(4, 5, 4, 5, { kind: "renamed", quiet: false }), run(10, 11, 10, 11, { kind: "renamed", quiet: false })]
  const using = (proof: "asked" | "confirmed" | "usesOutside") =>
    textDiff(20, 20, blocks(), { runs: labels, subject: subjectMarks({ used: [{ rename: open, file: 2, line: 18, proof }] }) })

  /** One block of two lines: the call of a function another file declares on the first, a local of the file renamed on the second */
  const usingBesideLocal = () =>
    textDiff(20, 20, [block(4, 6, 4, 6, { inlinePairs: true })], {
      renames: [rename("total", "sum", [[5, 2, 5, 2]])],
      subject: subjectMarks({ used: [{ rename: rename("openStore", "connectStore", [[4, 16, 4, 16]], { local: false, declared: [] }), file: 2, line: 18, proof: "confirmed" }] }),
    })

  it("gives a name another file declares one note, at its first site in the rows, with the file and the line of its declaration and what the index proved", () => {
    const notes = renameNotesOf(using("asked"))
    expect([...notes.byBlock].map(([at, note]) => [at, note.key, note.renames, note.reach, note.lines, note.hides])).toEqual([
      [0, 4, [open], [{ proof: "asked", declared: { file: 2, line: 18, moved: null }, across: null }], [4, 4], []],
    ])
    // both its runs are named by that note: neither has a note of its own
    const diff = using("confirmed")
    const confirmed = renameNotesOf(diff)
    const [note] = [...confirmed.byBlock.values()]
    expect([diff.marks.runs.map(one => confirmed.ofRun.get(one) === note), note!.reach]).toEqual([
      [true, true],
      [{ proof: "confirmed", declared: { file: 2, line: 18, moved: null }, across: null }],
    ])
    // a section that does not show the first block: the note at the next site
    expect([...renameNotesOf(using("asked"), index => index !== 0).byBlock.keys()]).toEqual([1])
  })

  it("says of a name the file declares what the index proved of it and how far it reaches in the subject, by its number among the renames of the file, and nothing of a local", () => {
    // a local of the first block, and a function the file declares in the third, which the index was asked of: three
    // places in two files of the subject hold it
    const total = rename("total", "sum", [[4, 2, 4, 2]])
    const declared = rename("openStore", "connectStore", [[14, 4, 14, 4]], { local: false })
    const diff = textDiff(20, 20, blocks(), { renames: [total, declared], subject: subjectMarks({ declared: [renameCheck(1, "usesOutside", 3, 2)] }) })
    expect([...renameNotesOf(diff).byBlock].map(([at, note]) => [at, note.renames.map(it => it.new), note.reach])).toEqual([
      [0, ["sum"], [{ proof: null, declared: null, across: null }]],
      [2, ["connectStore"], [{ proof: "usesOutside", declared: null, across: { sites: 3, files: 2 } }]],
    ])
    // the renames of a file and the names it uses in one note, each with what the subject says of it, in the order of their sites
    const mixed = usingBesideLocal()
    expect([...renameNotesOf(mixed).byBlock.values()].map(note => [note.renames.map(it => it.new), note.reach])).toEqual([
      [
        ["connectStore", "sum"],
        [
          { proof: "confirmed", declared: { file: 2, line: 18, moved: null }, across: null },
          { proof: null, declared: null, across: null },
        ],
      ],
    ])
  })

  it("names each rename of a note at its site in the rows, the place a jump to its declaration leaves: never an earlier site that a review plays down or a section does not show", () => {
    const jumps = (notes: ReturnType<typeof renameNotesOf>) => [...notes.byBlock.values()].map(note => [note.sites, note.renames.map((_, at) => renameJumpOf(note, at))])
    // a file that names the function on line `first` (an import on line 2, a block of the imports alone, or a call) and calls it on line 30
    const report = (first: number, extra: Parameters<typeof textDiff>[3] = {}) => {
      const used = rename(
        "openStore",
        "connectStore",
        [
          [first, 13, first, 13],
          [30, 16, 30, 16],
        ],
        { local: false, declared: [] },
      )
      return textDiff(40, 40, [block(first, first + 1, first, first + 1, { inlinePairs: true }), block(30, 31, 30, 31, { inlinePairs: true })], {
        runs: [run(first, first + 1, first, first + 1, { kind: "renamed", quiet: false }), run(30, 31, 30, 31, { kind: "renamed", quiet: false })],
        subject: subjectMarks({ used: [{ rename: used, file: 2, line: 18, proof: "asked" }] }),
        ...extra,
      })
    }
    const atCall = { file: 2, line: 18, name: "connectStore", from: 30 }
    // every row shown: the note at the import, the first site, and the jump leaves it
    expect(jumps(renameNotesOf(report(2)))).toEqual([[[2], [{ ...atCall, from: 2 }]]])
    // a review that plays the imports down has the note at the call: the jump leaves the call, never the import above
    expect(jumps(renameNotesOf(report(2, { importBlocks: [0] }), () => true, new Set([0])))).toEqual([[[30], [atCall]]])
    // a section that does not show the block of the first site: the note, and the place the jump leaves, at the call it shows
    expect(jumps(renameNotesOf(report(10), index => index === 1))).toEqual([[[30], [atCall]]])
    // a note of two renames: each at its own site; a name the file declares itself has no declaration elsewhere to jump to
    const mixed = usingBesideLocal()
    expect(jumps(renameNotesOf(mixed))).toEqual([
      [
        [4, 5],
        [{ file: 2, line: 18, name: "connectStore", from: 4 }, null],
      ],
    ])
    const [note] = [...renameNotesOf(mixed).byBlock.values()]
    expect(renameJumpOf(note!, 2)).toBeNull()
  })
})
