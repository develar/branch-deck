// @spec openspec/specs/commit/quiet-changes/spec.md
import { describe, expect, it } from "vite-plus/test"
import {
  blockParts,
  drawnMarks,
  inlinePairsOf,
  marksByBlock,
  NO_MARKS,
  partsLines,
  partsMoveNotes,
  partsNotes,
  partsRow,
  partsRowOfLine,
  partsStopRow,
  readingOf,
  stopsOf,
} from "@/lib/diff/block-parts"
import type { BlockMarks, PartsOptions } from "@/lib/diff/block-parts"
import type { LineRun } from "@/lib/diff/file-diff-model"
import { block, move, run, textDiff } from "@/test-support/diff-blocks"

const OLD = 0
const NEW = 1
const options = (mode: PartsOptions["mode"], open: number[] = []): PartsOptions => ({ mode, tint: "full", open: new Set(open), heading: null })
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
    expect([partsNotes(inline), partsNotes(blockParts(marked, 0, runs, options("unified")))]).toEqual([[{ lines: [2, 5], row: 1 }], []])
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

  it("reads the stops as Rust gives them, and counts them against the blocks of changed lines", () => {
    const diff = textDiff(30, 32, [block(1, 2, 1, 2), block(10, 11, 10, 13), block(20, 21, 22, 23), block(25, 26, 27, 28, { kind: "ignored" })], {
      runs: [run(10, 11, 10, 13)],
      importBlocks: [0],
    })
    expect([...stopsOf(diff)]).toEqual([
      [0, { side: NEW, line: 1 }],
      [2, { side: NEW, line: 22 }],
    ])
    expect([...stopsOf(diff, true).keys()]).toEqual([2])
    expect([readingOf(diff), readingOf(diff, true)]).toEqual([
      { toRead: 2, changes: 3 },
      { toRead: 1, changes: 3 },
    ])
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

describe("the ends of the moves of a block", () => {
  /** The rows of the parts as text: a note by its kind and the lines it stands for, a line by its kind, its lines, its tint and the end of its move */
  const rowsOf = (parts: ReturnType<typeof blockParts>) =>
    Array.from({ length: parts.rowCount }, (_, row) => {
      const value = partsRow(parts, row)
      if (value.kind === "note") {
        const { remark } = value
        return `note ${remark!.kind} ${value.lines[OLD]!.join("-")}${remark!.open === null ? "" : remark!.open ? " open" : " closed"}${remark!.move === null ? "" : ` ${remark!.move.at}`}`
      }
      return `${value.kind} ${value.lines.map(line => line ?? "_").join(",")}${value.tint === "full" ? "" : ` ${value.tint}`}${value.move === null ? "" : ` ${value.move.at}`}`
    })
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
    expect(partsNotes(closed)).toEqual([{ lines: [20, 24], row: 0 }])
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
    // a small generator with a seed (mulberry32): the same blocks, runs and moves in every run of the test
    let seed = 20261111
    const random = (below: number) => {
      seed = (seed + 0x6d2b79f5) | 0
      let mixed = Math.imul(seed ^ (seed >>> 15), 1 | seed)
      mixed = (mixed + Math.imul(mixed ^ (mixed >>> 7), 61 | mixed)) ^ mixed
      return ((mixed ^ (mixed >>> 14)) >>> 0) % below
    }
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
