// @spec openspec/specs/commit/quiet-changes/spec.md
import { describe, expect, it } from "vite-plus/test"
import { blockParts, inlinePairsOf, partsLines, partsNotes, partsRow, partsRowOfLine, partsStopRow, readingOf, runsByBlock, stopsOf } from "@/lib/diff/block-parts"
import type { PartsOptions } from "@/lib/diff/block-parts"
import { block, run, textDiff } from "@/test-support/diff-blocks"

const OLD = 0
const NEW = 1
const options = (mode: PartsOptions["mode"], open: number[] = []): PartsOptions => ({ mode, tint: "full", open: new Set(open), heading: null })

describe("blockParts", () => {
  // a line added, three lines indented again (quiet), a line deleted, a comment worded again (a label), a line added
  const marked = block(2, 7, 2, 8)
  const runs = [run(2, 5, 3, 6), run(6, 7, 6, 7, { kind: "comments", quiet: false })]

  it("is one part of lines for a block without a run, in the shape of its layout", () => {
    const plain = block(2, 4, 2, 3)
    expect([
      blockParts(plain, 0, [], options("split")).rowCount,
      blockParts(plain, 0, [], options("unified")).rowCount,
      blockParts(plain, 0, [], options("inline")).rowCount,
    ]).toEqual([2, 3, 3])
    // a similar pair is one row in Inline alone; an ignored block shows its new lines in one column
    const pair = block(2, 3, 2, 3, { inlinePairs: true })
    expect([blockParts(pair, 0, [], options("inline")).rowCount, blockParts(pair, 0, [], options("unified")).rowCount]).toEqual([1, 2])
    const ignored = block(2, 4, 2, 3, { kind: "ignored" })
    expect([blockParts(ignored, 0, [], options("unified")).rowCount, blockParts(ignored, 0, [], options("split")).rowCount]).toEqual([1, 2])
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
    const label = [run(7, 8, 1, 2, { kind: "comments", quiet: false })]
    for (const mode of ["inline", "unified", "split"] as const) {
      const cut = blockParts(deletion, 0, label, options(mode))
      // the deleted lines, the note, the comment: the stop is the first deleted line, where the block without a run stops
      expect([cut.parts.length, partsStopRow(cut, NEW, 1), partsStopRow(blockParts(deletion, 0, [], options(mode)), NEW, 1)], mode).toEqual([3, 0, 0])
    }
    // under a heading the first row of lines, never the heading; two labels of a block are one reading
    const heading = { parts: [{ name: "record", words: "body changed" }], label: "record — body changed" }
    const labels = [run(1, 2, 1, 2, { kind: "comments", quiet: false }), run(4, 5, 2, 3, { kind: "comments", quiet: false })]
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
    expect(runsByBlock(diff).map(runs => runs.map(it => it.oldStart))).toEqual([[2, 4], [10]])
    // read once per diff
    expect(runsByBlock(diff)).toBe(runsByBlock(diff))
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
    expect(inlinePairsOf(pairs, 0, [])).toEqual([
      [2, 2],
      [3, 3],
      [4, 4],
    ])
    expect(inlinePairsOf(pairs, 0, [run(3, 4, 3, 4)])).toEqual([
      [2, 2],
      [4, 4],
    ])
    expect(inlinePairsOf(block(2, 5, 2, 5), 0, [])).toEqual([])
  })
})
