// @spec openspec/specs/commit/commit-files/spec.md
import { describe, expect, it } from "vite-plus/test"
import { DiffLayout, endsWithFooter, EPILOGUE_KEY, FOOTER_MIN_ROWS, moveHandOf, moveNoteOfKey, PROLOGUE_KEY, SCROLLBAR_HEIGHT, visualLinePrefix } from "@/lib/diff/layout"
import type { LayoutFile } from "@/lib/diff/layout"
import { rowHeightOf } from "@/lib/diff/metrics"
import { buildFileRows, EXPANDED, NEW, OLD, rowOfLine } from "@/lib/diff/rows"
import { visualLineCount, wrapBreaks } from "@/lib/diff/wrap"
import type { TextMetrics } from "@/lib/diff/wrap"
import { block, move, run, textDiff } from "@/test-support/diff-blocks"

const METRICS = { rowHeight: 20, headerHeight: 36 }
/** One px per column; a CJK ideograph two columns wide */
const TEXT: TextMetrics = { columnWidth: 1, tabSize: 4, widthOf: codePoint => (codePoint >= 0x3000 ? 2 : 1) }
const ONE_CHANGE = textDiff(40, 40, [block(20, 21, 20, 21)])

function file(key: string, rows: LayoutFile["rows"], extra: Partial<LayoutFile> = {}): LayoutFile {
  return { key, rows, notice: false, ...extra }
}

describe("rowHeightOf", () => {
  it("takes the ascent and the descent of the font times IntelliJ's line spacing, on the pixel grid of the zoom", () => {
    // JetBrains Mono at 13 px
    const font = { ascent: 13.26, descent: 3.9, columnWidth: 7.8 }
    expect(rowHeightOf(font, 1)).toBe(21)
    expect(rowHeightOf(font, 2)).toBe(20.5)
    // 75% on a Retina screen: each row starts on a whole device pixel
    const zoomed = rowHeightOf(font, 1.5)
    expect(zoomed * 1.5).toBe(31)
  })
})

describe("wrapBreaks", () => {
  it("keeps a line that fits, and breaks after the last space or punctuation mark that fits", () => {
    expect(wrapBreaks("val a = 1", 20, TEXT)).toEqual([])
    expect(wrapBreaks("aaaa bbbb cccc", 10, TEXT)).toEqual([10])
    expect(wrapBreaks("call(first,second)", 12, TEXT)).toEqual([11])
    expect(visualLineCount("aaaa bbbb cccc dddd", 10, TEXT)).toBe(2)
  })

  it("breaks a word longer than the box at its edge", () => {
    expect(wrapBreaks("x".repeat(25), 10, TEXT)).toEqual([10, 20])
  })

  it("starts a wrapped part at the indent of its line, at most half the box", () => {
    // 4 columns of indent: the parts after the first have 6 columns
    expect(wrapBreaks("    aaa bbb ccc", 10, TEXT)).toEqual([8, 12])
    // an indent over half the box is dropped: with it, "cccc" would need a third line
    expect(wrapBreaks(`${" ".repeat(6)}aa bb cccc`, 10, TEXT)).toEqual([9])
  })

  it("expands a tab to its stop and gives a wide code point its measured width", () => {
    expect(wrapBreaks("\tab", 6, TEXT)).toEqual([])
    expect(wrapBreaks("\tabc", 6, TEXT)).toEqual([3])
    expect(wrapBreaks("日本語日本語", 6, TEXT)).toEqual([3])
  })
})

describe("DiffLayout", () => {
  it("lays out the headers, the notices and the rows arithmetically, before any text", () => {
    const rows = buildFileRows(ONE_CHANGE, "unified")
    const layout = new DiffLayout([file("a", rows), file("b", null, { notice: true }), file("c", rows)], METRICS)
    const height = 36 + rows.rowCount * 20
    expect(Array.from(layout.offsets)).toEqual([0, height, height + 56, 2 * height + 56])
    expect(layout.fileAt(height + 10)).toBe(1)
    expect(layout.fileAt(height + 56)).toBe(2)
    expect(layout.rowTop(2, 3)).toBe(height + 56 + 36 + 60)
    expect(layout.rowAt(2, height + 56 + 36 + 65)).toBe(3)
    expect(layout.rowAt(1, height + 10)).toBe(-1)
  })

  it("reports a card once both its top and its bottom were in view, at once or one scroll after the other, and never a card whose diff is not read", () => {
    const rows = buildFileRows(ONE_CHANGE, "unified")
    const height = 36 + rows.rowCount * 20
    const layout = new DiffLayout([file("a", rows), file("tall", buildFileRows(ONE_CHANGE, "unified", [EXPANDED, EXPANDED])), file("pending", null), file("c", rows)], METRICS)
    const tall = layout.fileHeight(1)
    const edges = new Map<string, number>()
    const ready = (index: number) => index !== 2
    // the first card whole in a view of two cards; the top of the tall card only
    expect(layout.passedCards(0, height + 50, edges, ready)).toEqual([0])
    // again: reported once
    expect(layout.passedCards(0, height + 50, edges, ready)).toEqual([])
    // the tall card, taller than the view, passes as its bottom comes into view; the pending card never
    expect(tall).toBeGreaterThan(height + 50)
    expect(layout.passedCards(height + tall - 40, height + 50, edges, ready)).toEqual([1])
    expect(layout.passedCards(layout.offsets[2]!, height + 200, edges, ready)).toEqual([3])
    expect(edges.get("pending")).toBeUndefined()
  })

  it("gives a wrapped row its visual lines", () => {
    const rows = buildFileRows(textDiff(0, 3, [block(0, 0, 0, 3)]), "unified")
    const layout = new DiffLayout([file("a", rows, { visualLines: visualLinePrefix(3, row => [1, 2, 1][row]!) })], METRICS)
    expect([layout.rowTop(0, 2), layout.rowHeight(0, 1), layout.height]).toEqual([36 + 60, 40, 36 + 80])
    expect(layout.rowAt(0, 36 + 20 + 30)).toBe(1)
  })

  it("keeps the line at the top across an expansion above it and across a mode switch", () => {
    const before = new DiffLayout([file("a", buildFileRows(ONE_CHANGE, "unified"))], METRICS)
    const rows = before.files[0]!.rows!
    const scrollTop = before.rowTop(0, rowOfLine(rows, NEW, 22)) + 7
    const anchor = before.anchorAt(scrollTop)!
    expect(anchor).toEqual({ key: "a", line: { side: NEW, line: 22 }, offset: 7 })

    const expanded = new DiffLayout([file("a", buildFileRows(ONE_CHANGE, "unified", [EXPANDED]))], METRICS)
    // the 16 folded lines above are 15 rows more
    expect(expanded.scrollTopOf(anchor)).toBe(scrollTop + 15 * 20)

    const deleted = before.anchorAt(before.rowTop(0, rowOfLine(rows, OLD, 20)))!
    expect(deleted.line).toEqual({ side: OLD, line: 20 })
    const split = new DiffLayout([file("a", buildFileRows(ONE_CHANGE, "split"))], METRICS)
    expect(split.scrollTopOf(deleted)).toBe(split.rowTop(0, 5))
  })

  it("keeps a note of a quiet run at the top by the old line it stands for, and lays a note and a line shown once out as a row each", () => {
    // a call of one line wrapped over three, a quiet run of formatting: Inline shows a note and the three new lines
    const diff = textDiff(10, 12, [block(3, 4, 3, 6)], { runs: [run(3, 4, 3, 6)] })
    const closed = buildFileRows(diff, "inline", [EXPANDED, EXPANDED])
    const layout = new DiffLayout([file("a", closed)], METRICS)
    // the three context rows, the note, the three lines shown once: every row has the row height, before any text
    expect([closed.rowCount, layout.rowsHeight(0), layout.rowTop(0, 3), layout.rowHeight(0, 3)]).toEqual([13, 13 * 20, 36 + 60, 20])
    const atNote = layout.anchorAt(layout.rowTop(0, 3) + 5)!
    expect(atNote).toEqual({ key: "a", line: { side: OLD, line: 3 }, offset: 5 })
    expect(layout.scrollTopOf(atNote)).toBe(layout.rowTop(0, 3) + 5)
    // the note opened: the old line it stood for is the row after it; the top stays at that line
    const opened = new DiffLayout([file("a", buildFileRows(diff, "inline", [EXPANDED, EXPANDED], false, null, new Set([3])))], METRICS)
    expect([opened.files[0]!.rows!.rowCount, opened.scrollTopOf(atNote)]).toEqual([14, opened.rowTop(0, 4) + 5])
    // a line shown once is at the top by its new line, and stays there in Unified, below the old line of its run
    const once = layout.anchorAt(layout.rowTop(0, 5))!
    expect(once.line).toEqual({ side: NEW, line: 4 })
    const unified = new DiffLayout([file("a", buildFileRows(diff, "unified", [EXPANDED, EXPANDED]))], METRICS)
    expect(unified.scrollTopOf(once)).toBe(unified.rowTop(0, 6))
    // with soft wraps a note is one visual line, a line shown once as many as its new line takes
    const wrapped = new DiffLayout([file("a", closed, { visualLines: visualLinePrefix(closed.rowCount, row => (row === 5 ? 2 : 1)) })], METRICS)
    expect([wrapped.rowHeight(0, 3), wrapped.rowHeight(0, 5), wrapped.rowTop(0, 6)]).toEqual([20, 40, 36 + 7 * 20])
  })

  it("keeps the note of the lines a move took at the top by the old line it stands for, and the note of where they stand by the line after it; each a row", () => {
    // three lines leave the old lines 20..23 for the new lines 5..8
    const diff = textDiff(30, 30, [block(5, 5, 5, 8), block(20, 23, 23, 23)], { moves: [move(20, 5, 3)] })
    const whole = (mode: "inline" | "unified", open: number[] = []) =>
      new DiffLayout([file("a", buildFileRows(diff, mode, [EXPANDED, EXPANDED, EXPANDED], false, null, new Set(open)))], METRICS)
    // Inline: 5 lines, the note and the 3 lines that stand there, 15 lines, the note that stands for the lines that left
    const inline = whole("inline")
    expect([inline.files[0]!.rows!.rowCount, inline.rowHeight(0, 5), inline.rowHeight(0, 24)]).toEqual([32, 20, 20])
    const atSource = inline.anchorAt(inline.rowTop(0, 24) + 5)!
    expect(atSource).toEqual({ key: "a", line: { side: OLD, line: 20 }, offset: 5 })
    expect(inline.scrollTopOf(atSource)).toBe(inline.rowTop(0, 24) + 5)
    // opened, and in Unified, the old line has its row after the note: the top stays at that line
    expect([whole("inline", [20]).scrollTopOf(atSource), whole("unified").scrollTopOf(atSource)]).toEqual([inline.rowTop(0, 25) + 5, inline.rowTop(0, 25) + 5])
    // the note of where the lines stand hides no line: it is held by the first of them, and stays where it was
    const atDestination = inline.anchorAt(inline.rowTop(0, 5) + 3)!
    expect(atDestination).toEqual({ key: "a", line: { side: NEW, line: 5 }, offset: -17 })
    expect([inline.scrollTopOf(atDestination), whole("unified").scrollTopOf(atDestination)]).toEqual([inline.rowTop(0, 5) + 3, inline.rowTop(0, 5) + 3])
    // with soft wraps the notes are a visual line each, a line of the move as many as its text takes
    const rows = inline.files[0]!.rows!
    const wrapped = new DiffLayout([file("a", rows, { visualLines: visualLinePrefix(rows.rowCount, row => (row === 7 ? 3 : 1)) })], METRICS)
    expect([wrapped.rowHeight(0, 5), wrapped.rowHeight(0, 7), wrapped.rowHeight(0, 24), wrapped.rowTop(0, 8)]).toEqual([20, 60, 20, 36 + 10 * 20])
  })

  /**
   * Two files with moves, every line shown. `a`: three lines leave the old lines 20..23 for the new lines 5..8, and two
   * the old lines 12..14 for the new lines 28..30; `b`: one line leaves the old line 9 for the new line 2. Rows of 20 px
   * under headers of 36 px
   */
  const movesLayout = () => {
    const two = textDiff(40, 40, [block(5, 5, 5, 8), block(12, 14, 15, 15), block(20, 23, 21, 21), block(30, 30, 28, 30)], { moves: [move(20, 5, 3), move(12, 28, 2)] })
    const one = textDiff(12, 12, [block(2, 2, 2, 3), block(9, 10, 10, 10)], { moves: [move(9, 2, 1)] })
    const levels = [EXPANDED, EXPANDED, EXPANDED, EXPANDED, EXPANDED]
    return new DiffLayout([file("a", buildFileRows(two, "inline", levels)), file("b", buildFileRows(one, "inline", levels))], METRICS)
  }

  it("gives the key of the moves one note: the one at the end the view stands at while it is in view, else the first note of a move in view", () => {
    const layout = movesLayout()
    const notes = layout.files[0]!.rows!.moveNotes
    expect(notes.map(note => [note.end.at, note.row, note.end.lines[0]])).toEqual([
      ["destination", 5, 5],
      ["source", 16, 12],
      ["source", 23, 20],
      ["destination", 31, 28],
    ])
    const taken = (top: number, height: number, hand: Parameters<typeof moveNoteOfKey>[3] = null) => {
      const note = moveNoteOfKey(layout, top, height, hand)
      return note === null ? null : [note.file, note.end.at, note.row]
    }
    // the whole file in view: the first note of a move
    expect(taken(0, 800)).toEqual([0, "destination", 5])
    // a row is in view whole below the header stuck at the top, and above the end of the view
    expect([layout.rowInView(0, 5, 100, 400), layout.rowInView(0, 5, 101, 400), layout.rowInView(0, 16, 0, 376), layout.rowInView(0, 16, 0, 375)]).toEqual([
      true,
      false,
      true,
      false,
    ])
    // the first note under the header no more: the next one in view
    expect(taken(101, 400)).toEqual([0, "source", 16])
    // the end the view stands at, while its note is in view: where a jump landed, or the note N stopped under
    const landed = moveHandOf("a", notes[2]!.end)
    expect(landed).toEqual({ key: "a", side: OLD, line: 20 })
    expect([taken(0, 800, landed), taken(101, 420, landed)]).toEqual([
      [0, "source", 23],
      [0, "source", 23],
    ])
    // its note out of view (row 23 ends at 516): the first note in view again
    expect([taken(0, 515, landed), taken(101, 400, landed)]).toEqual([
      [0, "destination", 5],
      [0, "source", 16],
    ])
    // an end by its side and its line: the same line of the other side is another place, a line of no end is none
    expect([taken(0, 800, { key: "a", side: NEW, line: 20 }), taken(0, 800, { key: "a", side: OLD, line: 21 }), taken(0, 800, { key: "gone", side: OLD, line: 20 })]).toEqual([
      [0, "destination", 5],
      [0, "destination", 5],
      [0, "destination", 5],
    ])
    // below the notes of `a`, the first note of the next file in view; with no note in view, none
    const second = layout.offsets[1]!
    expect(taken(second - 100, 400)).toEqual([1, "destination", 2])
    expect([taken(layout.rowTop(0, 29) - 36, 60), taken(0, 100), moveNoteOfKey(new DiffLayout([], METRICS), 0, 400, null)]).toEqual([null, null, null])
  })

  it("passes a note of a move whose other end the view cannot show: the key takes the next one in view that it can, at the end the view stands at too", () => {
    // the notes of `a` at the rows 5, 16, 23 and 31, the note of `b` after them
    const layout = movesLayout()
    const notes = layout.files[0]!.rows!.moveNotes
    const taken = (shown: (file: number, row: number) => boolean, hand: Parameters<typeof moveNoteOfKey>[3] = null) => {
      const note = moveNoteOfKey(layout, 0, 2000, hand, (at, end) => shown(at, layout.files[at]!.rows!.moveNotes.find(it => it.end === end)!.row))
      return note === null ? null : [note.file, note.row]
    }
    // the first note cannot be shown: the second; none of `a` can: the first of `b`; none at all: no note
    expect([taken((_, row) => row !== 5), taken(at => at === 1), taken(() => false)]).toEqual([[0, 16], [1, 2], null])
    // the end the view stands at is taken first only where it can be shown
    const landed = moveHandOf("a", notes[2]!.end)
    expect([taken(() => true, landed), taken((_, row) => row !== 23, landed)]).toEqual([
      [0, 23],
      [0, 5],
    ])
  })

  it("gives the rows of a file that are in view: whole below the header stuck at the top, and above the end of the view", () => {
    // 41 rows of 20 px under a header of 36 px, then a second file
    const rows = buildFileRows(ONE_CHANGE, "unified", [EXPANDED, EXPANDED])
    const layout = new DiffLayout([file("a", rows), file("b", buildFileRows(ONE_CHANGE, "unified", [EXPANDED, EXPANDED]))], METRICS)
    expect(rows.rowCount).toBe(41)
    expect([layout.rowsInView(0, 0, 400), layout.rowsInView(0, 100, 400), layout.rowsInView(0, 101, 400), layout.rowsInView(0, 100, 415)]).toEqual([
      [0, 18],
      [5, 23],
      [6, 23],
      [5, 23],
    ])
    // every row of the range is in view, and the rows around it are not
    const [from, to] = layout.rowsInView(0, 101, 400)
    expect([layout.rowInView(0, from - 1, 101, 400), layout.rowInView(0, from, 101, 400), layout.rowInView(0, to - 1, 101, 400), layout.rowInView(0, to, 101, 400)]).toEqual([
      false,
      true,
      true,
      false,
    ])
    // a file below the view and one above it have no row in view; a view too low for a row has none either
    const second = layout.offsets[1]!
    const none = ([start, end]: readonly [number, number]) => end - start
    expect([none(layout.rowsInView(1, 0, 400)), none(layout.rowsInView(0, second, 400)), none(layout.rowsInView(0, 100, 50))]).toEqual([0, 0, 0])
    expect(layout.rowsInView(1, second, 400)).toEqual([0, 18])
    expect(none(new DiffLayout([file("empty", null)], METRICS).rowsInView(0, 0, 400))).toBe(0)
  })

  it("keeps a row of words that stands for no line at the top across a new layout, by the row after it: a note of Unified and Split, an opened note, a label, a heading", () => {
    const whole = (diff: ReturnType<typeof textDiff>, mode: "inline" | "unified" | "split", open: number[] = [], units: Parameters<typeof buildFileRows>[4] = null) =>
      new DiffLayout([file("a", buildFileRows(diff, mode, [EXPANDED, EXPANDED], false, units, new Set(open)))], METRICS)
    /** The anchor of a top 3 px into row `row`, and where the same rows laid out again put the top: nothing changed, nothing moves */
    const held = (layout: DiffLayout, row: number) => {
      const top = layout.rowTop(0, row) + 3
      const anchor = layout.anchorAt(top)!
      const again = new DiffLayout(layout.files, METRICS)
      return { anchor, moved: again.scrollTopOf(anchor)! - top }
    }
    // the wrapped call: the note is row 3 in every layout; only the closed note of Inline stands for a line
    const wrapped = textDiff(10, 12, [block(3, 4, 3, 6)], { runs: [run(3, 4, 3, 6)] })
    // Unified: the note, the old line, the three new ones. The row after the note is the old line, 17 px below the top
    expect(held(whole(wrapped, "unified"), 3)).toEqual({ anchor: { key: "a", line: { side: OLD, line: 3 }, offset: -17 }, moved: 0 })
    // Split: the row after the note shows both lines
    expect(held(whole(wrapped, "split"), 3)).toEqual({ anchor: { key: "a", line: { side: NEW, line: 3 }, offset: -17 }, moved: 0 })
    // Inline, the note opened: as Unified
    expect(held(whole(wrapped, "inline", [3]), 3)).toEqual({ anchor: { key: "a", line: { side: OLD, line: 3 }, offset: -17 }, moved: 0 })
    // a comment of two lines worded again, a label: its note hides no line in any layout
    const comment = textDiff(20, 20, [block(5, 7, 5, 7)], { runs: [run(5, 7, 5, 7, { kind: "comments", quiet: false })] })
    for (const mode of ["inline", "unified", "split"] as const) {
      expect([mode, held(whole(comment, mode), 5).moved]).toEqual([mode, 0])
    }
    // the heading of a change in a section of a review, alone in its row and with the words of a run that hides no line
    const units = { marks: new Map([[0, [{ name: "record", words: "body changed" }]]]), elsewhere: new Map<number, string>() }
    expect(held(whole(textDiff(10, 10, [block(3, 4, 3, 4)]), "unified", [], units), 3)).toEqual({ anchor: { key: "a", line: { side: OLD, line: 3 }, offset: -17 }, moved: 0 })
    expect(held(whole(wrapped, "split", [], units), 3).moved).toBe(0)

    // the top in the note of Unified, then Inline: its note stands for the old line, so the top is at the note, never
    // in the row above it
    const atNote = held(whole(wrapped, "unified"), 3).anchor
    const inline = whole(wrapped, "inline")
    expect(inline.scrollTopOf(atNote)).toBe(inline.rowTop(0, 3))
    // and in a layout without the note (the marks of another policy) at the line itself
    const plain = whole(textDiff(10, 12, [block(3, 4, 3, 6)]), "unified")
    expect(plain.scrollTopOf(atNote)).toBe(plain.rowTop(0, 3))
    // a note opened below the top moves nothing at the top; one opened above it moves the top by its old line
    const two = textDiff(20, 24, [block(3, 4, 3, 6), block(10, 11, 12, 15)], { runs: [run(3, 4, 3, 6), run(10, 11, 12, 15)] })
    const closed = whole(two, "inline")
    const second = closed.anchorAt(closed.rowTop(0, 13) + 3)!
    expect(second.line).toEqual({ side: OLD, line: 10 })
    expect(whole(two, "inline", [3]).scrollTopOf(second)).toBe(closed.rowTop(0, 13) + 3 + 20)
  })

  it("keeps an anchor in a header, and finds a folded line in its fold", () => {
    const layout = new DiffLayout([file("a", buildFileRows(ONE_CHANGE, "unified"))], METRICS)
    expect(layout.anchorAt(12)).toEqual({ key: "a", offset: 12 })
    expect(layout.scrollTopOf({ key: "a", offset: 12 })).toBe(12)
    expect(layout.scrollTopOf({ key: "gone", offset: 12 })).toBeNull()
    expect(layout.scrollTopOf({ key: "a", line: { side: NEW, line: 3 }, offset: 4 })).toBe(36 + 4)
  })

  it("keeps a top below the rows of the last file from the end of its rows: rows that come above it move it by as much, and nothing of the file is brought back into view", () => {
    // a view without headers that lies above the viewport of the scroller it is a block of: 12 rows of 20 px, the top
    // of the viewport 75 px below their end
    const embedded = { rowHeight: 20, headerHeight: 0 }
    const rows = buildFileRows(ONE_CHANGE, "unified")
    const layout = new DiffLayout([file("a", rows)], embedded)
    expect([rows.rowCount, layout.height]).toEqual([12, 240])
    const below = layout.anchorAt(315)!
    expect(below).toEqual({ key: "a", below: true, offset: 75 })
    expect(layout.scrollTopOf(below)).toBe(315)
    // right at the end of the rows too; a pixel above it the last row, a fold, holds the top by its first line
    expect([layout.anchorAt(240), layout.anchorAt(239)]).toEqual([
      { key: "a", below: true, offset: 0 },
      { key: "a", line: { side: NEW, line: 25 }, offset: 19 },
    ])
    // the file shows all its lines: 29 rows more above the top, which moves by their height
    const grown = new DiffLayout([file("a", buildFileRows(ONE_CHANGE, "unified", [EXPANDED, EXPANDED]))], embedded)
    expect([grown.height, grown.scrollTopOf(below)]).toEqual([820, 895])
    // of two files the top at the end of the first is held by the file after it, as before; below the last one by its end
    const two = new DiffLayout([file("a", rows), file("b", rows)], embedded)
    expect([two.anchorAt(240)?.key, two.anchorAt(540)]).toEqual(["b", { key: "b", below: true, offset: 60 }])
  })
})

describe("DiffLayout of file cards", () => {
  const CARDS = { rowHeight: 20, headerHeight: 48, cards: { gap: 10, padding: 10, borderBottom: 1, divider: 32 } }

  it("counts the gaps, the space around the cards and the bottom border of each card, so each offset stays exact", () => {
    const rows = buildFileRows(ONE_CHANGE, "unified", [EXPANDED, EXPANDED])
    // a file with rows, a collapsed one (its header alone), one with a notice row
    const layout = new DiffLayout([file("a", rows), file("b", null), file("c", rows, { notice: true })], CARDS)
    const a = 48 + rows.rowCount * 20 + 1
    expect([layout.offsets[0], layout.fileHeight(0), layout.fileHeight(1), layout.offsets[1], layout.offsets[2]]).toEqual([10, a, 49, 10 + a + 10, 10 + a + 10 + 49 + 10])
    expect(layout.rowsOffset(2)).toBe(48 + 20)
    expect(layout.rowTop(0, 3)).toBe(10 + 48 + 3 * 20)
    expect(layout.height).toBe(layout.offsets[2]! + layout.fileHeight(2) + 10)
    // a row of the second card: the first card, its gap and the header
    expect(layout.rowTop(2, 0)).toBe(layout.offsets[2]! + 68)
    // a y in a gap is the file above it
    expect(layout.fileAt(10 + a + 5)).toBe(0)
    // the anchor keeps its line across the layout of the same files
    const anchor = layout.anchorAt(layout.rowTop(2, 5) + 3)!
    expect(layout.scrollTopOf(anchor)).toBe(layout.rowTop(2, 5) + 3)
  })

  it("sticks a header below the space above the first card, so a file at the top keeps that space over its card", () => {
    const rows = buildFileRows(ONE_CHANGE, "unified", [EXPANDED, EXPANDED])
    const layout = new DiffLayout([file("a", rows), file("b", rows)], CARDS)
    expect(layout.stickyTop).toBe(10)
    // the first file at the top is the top of the document; another one has the gap above its card in view
    expect([layout.fileTop(0), layout.fileTop(1)]).toEqual([0, layout.offsets[1]! - 10])
    // the file at the top is the one under the stuck header: the second once its header has the place of the first
    expect(layout.fileAtTop(layout.fileTop(1) - 30)).toBe(0)
    expect(layout.fileAtTop(layout.fileTop(1))).toBe(1)
    // without cards a header sticks at the top
    const plain = new DiffLayout([file("a", rows), file("b", rows)], { rowHeight: 20, headerHeight: 36 })
    expect([plain.stickyTop, plain.fileTop(1)]).toEqual([0, plain.offsets[1]])
  })

  it("shows the last file at the end of the document, whose header can never reach the top, else the file at the top", () => {
    const rows = buildFileRows(ONE_CHANGE, "unified", [EXPANDED, EXPANDED])
    const layout = new DiffLayout([file("a", rows), file("b", rows), file("c", null)], CARDS)
    // a view lower than the last two cards: scrolled to the end, the second card's header is the one at the top
    const view = layout.height - layout.offsets[1]! - 20
    const end = layout.height - view
    expect(layout.fileAtTop(end)).toBe(1)
    expect(layout.fileShownAt(end, view)).toBe(2)
    // a px short of the end still counts as the end; above it the file at the top
    expect(layout.fileShownAt(end - 1, view)).toBe(2)
    expect(layout.fileShownAt(end - 2, view)).toBe(1)
    expect(layout.fileShownAt(0, view)).toBe(0)
  })

  it("puts the opening of a step at its measured height and the divider of a group between the gap and its card: a scroll to the file shows it, a y in it is the file below", () => {
    const rows = buildFileRows(ONE_CHANGE, "unified", [EXPANDED, EXPANDED])
    // a step starts with the first file (an opening of 120 px) and with the fourth (64 px), a group with the second; the
    // fifth starts a step and a group, its opening says both
    const layout = new DiffLayout(
      [file("a", rows, { opening: 120 }), file("b", rows, { divider: true }), file("c", rows), file("d", rows, { opening: 64 }), file("e", rows, { opening: 50, divider: true })],
      CARDS,
    )
    const height = layout.fileHeight(0)
    expect([0, 1, 2, 3, 4, 5].map(index => layout.openingHeight(index))).toEqual([120, 32, 0, 64, 50, 0])
    // the first card below the space above the cards and its opening; each opening after the gap below the card above
    const a = 10 + 120
    const b = a + height + 10 + 32
    const c = b + height + 10
    const d = c + height + 10 + 64
    const e = d + height + 10 + 50
    expect(Array.from(layout.offsets)).toEqual([a, b, c, d, e, e + height + 10])
    expect(layout.height).toBe(layout.offsets[5]! - 10 + 10)
    expect([layout.sectionStart(0), layout.sectionStart(1), layout.sectionStart(2), layout.sectionStart(3)]).toEqual([10, b - 32, c, d - 64])
    // a scroll to a file has its opening and the gap above it in view; one without, the gap alone
    expect([layout.fileTop(0), layout.fileTop(1), layout.fileTop(2), layout.fileTop(3)]).toEqual([0, b - 32 - 10, c - 10, d - 64 - 10])
    // a y in an opening or a divider is the file below it, a y in the gap above it the file above
    expect([layout.fileAt(b - 1), layout.fileAt(b - 32), layout.fileAt(b - 33), layout.fileAt(d - 64), layout.fileAt(d - 65)]).toEqual([1, 1, 0, 3, 2])
    expect([layout.fileAtTop(layout.fileTop(1)), layout.fileAtTop(layout.fileTop(3))]).toEqual([1, 3])
    // the opening of a step is a part of its own; the divider of a group is the place of its card
    expect([layout.partAt(d - 64), layout.partAt(d - 1), layout.partAt(d), layout.partAt(b - 1), layout.partAt(c - 5)]).toEqual(["opening", "opening", "card", "card", "card"])
    // the anchor of a top in an opening keeps its place from the start of the opening across the layout
    const anchor = layout.anchorAt(d - 20)!
    expect(anchor).toEqual({ key: "d", opening: true, offset: 44 })
    expect(layout.scrollTopOf(anchor)).toBe(d - 20)
    // a scroll to a file has the place above its opening, not the last row of the file above: a new height of that
    // file keeps the opening in view whole
    const shown = layout.anchorAt(layout.fileTop(1))!
    expect(shown).toEqual({ key: "b", opening: true, offset: -10 })
    const folded = new DiffLayout([file("a", buildFileRows(ONE_CHANGE, "unified"), { opening: 120 }), ...layout.files.slice(1)], CARDS)
    expect(folded.fileHeight(0)).toBeLessThan(layout.fileHeight(0))
    expect(folded.scrollTopOf(shown)).toBe(folded.fileTop(1))
    // no opening and no divider without cards
    const plain = new DiffLayout([file("a", rows, { opening: 120, divider: true })], { rowHeight: 20, headerHeight: 36 })
    expect([plain.offsets[0], plain.fileTop(0), plain.openingHeight(0), plain.partAt(0)]).toEqual([0, 0, 0, "card"])
  })

  it("keeps a top in an opening from the start of the opening, so an opening that grows grows below it and one that shrinks gives the top to its card", () => {
    const rows = buildFileRows(ONE_CHANGE, "unified", [EXPANDED, EXPANDED])
    const files = (opening: number) => [file("a", rows), file("b", rows, { opening })]
    const layout = new DiffLayout(files(40), CARDS)
    const start = layout.sectionStart(1)
    const inOpening = layout.anchorAt(start + 15)!
    const above = layout.anchorAt(start - 5)!
    const inCard = layout.anchorAt(layout.rowTop(1, 3) + 2)!
    expect([inOpening, above]).toEqual([
      { key: "b", opening: true, offset: 15 },
      { key: "b", opening: true, offset: -5 },
    ])
    // a disclosure in the opening: its start and the top stay, the card below moves down by what it grew
    const grown = new DiffLayout(files(200), CARDS)
    expect([grown.sectionStart(1), grown.offsets[1]]).toEqual([start, layout.offsets[1]! + 160])
    expect([grown.scrollTopOf(inOpening), grown.scrollTopOf(above), grown.scrollTopOf(inCard)]).toEqual([start + 15, start - 5, layout.rowTop(1, 3) + 2 + 160])
    // an opening that became lower than the offset into it: the top of its card
    const shrunk = new DiffLayout(files(10), CARDS)
    expect(shrunk.scrollTopOf(inOpening)).toBe(shrunk.offsets[1])
  })

  it("puts the prologue before the first section, a gap below it, keeps a top in it from the top of the document and one in the gap at the first section", () => {
    const rows = buildFileRows(ONE_CHANGE, "unified", [EXPANDED, EXPANDED])
    const files = [file("a", rows, { opening: 40 }), file("b", rows)]
    const layout = new DiffLayout(files, CARDS, 300)
    // the space above the cards, the prologue, the gap, the opening of the first step, its card
    expect([layout.prologue, layout.sectionStart(0), layout.offsets[0]]).toEqual([300, 10 + 300 + 10, 10 + 300 + 10 + 40])
    expect(layout.height).toBe(layout.offsets[2]! - 10 + 10)
    // the first section shown has the prologue above the view, its gap in view as between two cards
    expect([layout.fileTop(0), layout.cardTop(0)]).toEqual([310, 350])
    expect([layout.partAt(0), layout.partAt(319), layout.partAt(320), layout.partAt(360)]).toEqual(["prologue", "prologue", "opening", "card"])
    expect([layout.fileAt(100), layout.fileAtTop(0)]).toEqual([0, 0])
    expect(layout.partShownAt(0, 200)).toBe("prologue")
    // a top in the prologue keeps its offset from the top as the prologue grows, and is the first section once the
    // prologue is lower than it
    const anchor = layout.anchorAt(100)!
    expect(anchor).toEqual({ key: PROLOGUE_KEY, offset: 100 })
    expect([new DiffLayout(files, CARDS, 500).scrollTopOf(anchor), new DiffLayout(files, CARDS, 50).scrollTopOf(anchor)]).toEqual([100, 10 + 50 + 10])
    // a scroll to the first section is in the gap below the prologue: it stays at the first section as the prologue grows
    const shown = layout.anchorAt(layout.fileTop(0))!
    expect(shown).toEqual({ key: "a", opening: true, offset: -10 })
    const taller = new DiffLayout(files, CARDS, 500)
    expect(taller.scrollTopOf(shown)).toBe(taller.fileTop(0))
    // a prologue alone keeps its height; none without cards, nor of no height
    const alone = new DiffLayout([], CARDS, 300)
    expect([alone.height, alone.anchorAt(20), alone.partAt(20)]).toEqual([320, { key: PROLOGUE_KEY, offset: 20 }, "prologue"])
    expect(new DiffLayout(files, { rowHeight: 20, headerHeight: 36 }, 300).prologue).toBe(0)
    expect(new DiffLayout(files, CARDS, 0).offsets[0]).toBe(10 + 40)
  })

  it("puts the epilogue after the last card, a gap above it, as an opening of the last file, and keeps a top in it from its top", () => {
    const rows = buildFileRows(ONE_CHANGE, "unified", [EXPANDED, EXPANDED])
    const files = [file("a", rows, { opening: 40 }), file("b", rows)]
    const plain = new DiffLayout(files, CARDS)
    const layout = new DiffLayout(files, CARDS, 0, 120)
    // the last card, the gap, the epilogue, the space below the cards; the cards stay where they were
    expect([layout.epilogueTop, layout.height]).toEqual([plain.offsets[2], plain.offsets[2]! + 120 + 10])
    expect([...layout.offsets]).toEqual([...plain.offsets])
    expect([layout.partAt(layout.epilogueTop - 1), layout.partAt(layout.epilogueTop), layout.fileAt(layout.epilogueTop + 50)]).toEqual(["card", "opening", 1])
    // a top in the epilogue keeps its offset from its top, clamped to its end when it is lower
    const anchor = layout.anchorAt(layout.epilogueTop + 30)!
    expect(anchor).toEqual({ key: EPILOGUE_KEY, offset: 30 })
    expect([layout.scrollTopOf(anchor), new DiffLayout(files, CARDS, 0, 20).scrollTopOf(anchor)]).toEqual([layout.epilogueTop + 30, layout.epilogueTop + 20])
    // none without cards, nor without files
    expect([new DiffLayout(files, METRICS, 0, 120).epilogue, new DiffLayout([], CARDS, 0, 120).epilogue]).toEqual([0, 0])
  })

  it("shows a change, a hit of the find and a declaration in a card below the opening of its step: the top of the card is their least top", () => {
    const rows = buildFileRows(ONE_CHANGE, "unified", [EXPANDED, EXPANDED])
    const layout = new DiffLayout([file("a", rows, { opening: 200 }), file("b", rows, { opening: 32 })], CARDS)
    // a scroll to the section shows its opening; one to the card has the opening above the view, the card at the sticky
    // top: the first change of a step lands at the same place whatever the height of its opening
    expect([layout.fileTop(0), layout.cardTop(0), layout.cardTop(1)]).toEqual([0, 200, layout.offsets[1]! - 10])
    const firstRow = layout.rowTop(0, 0) - layout.stickyTop - 48 - 4 * 20
    expect(Math.max(firstRow, layout.cardTop(0)) + layout.stickyTop).toBe(layout.offsets[0])
    // a top in the card of a step is its card, the opening above it read
    expect(layout.partShownAt(layout.cardTop(1), 100)).toBe("card")
    expect(layout.partShownAt(layout.fileTop(1), 100)).toBe("opening")
  })

  it("stops at a card with the heading of its group at the top with it, whole, and at a card below an opening with the opening above the view", () => {
    const rows = buildFileRows(ONE_CHANGE, "unified")
    // a card of no heading, one below an opening of a step, one below the heading of a group, one of both (the opening
    // stands for the heading)
    const layout = new DiffLayout(
      [file("plain", rows), file("step", rows, { opening: 120 }), file("group", rows, { divider: true }), file("both", rows, { opening: 80, divider: true })],
      CARDS,
    )
    expect([0, 1, 2, 3].map(index => layout.stopTop(index))).toEqual([layout.cardTop(0), layout.cardTop(1), layout.fileTop(2), layout.cardTop(3)])
    // the heading of the group starts at the sticky top, its 32 px above the card, the card right below it
    expect(layout.stopTop(2) + layout.stickyTop).toBe(layout.sectionStart(2))
    expect(layout.offsets[2]! - layout.sectionStart(2)).toBe(32)
    // a stop there is in the card, of that file
    expect([layout.fileShownAt(layout.stopTop(2), 100), layout.partShownAt(layout.stopTop(2), 100)]).toEqual([2, "card"])
    // without cards there is no heading: the top of the file
    expect(new DiffLayout([file("a", rows, { divider: true })], { rowHeight: 20, headerHeight: 36 }).stopTop(0)).toBe(0)
  })

  it("ends a card of more than 30 rows and the last card with a line one row high, below the rows, which the offsets count", () => {
    const long = buildFileRows(ONE_CHANGE, "unified", [EXPANDED, EXPANDED])
    const short = buildFileRows(ONE_CHANGE, "unified")
    expect([long.rowCount > FOOTER_MIN_ROWS, short.rowCount > FOOTER_MIN_ROWS]).toEqual([true, false])
    // by the rows alone: a long card, a short one, a collapsed one, and the last card whatever it is
    const ends = [endsWithFooter(long, false), endsWithFooter(short, false), endsWithFooter(null, false), endsWithFooter(null, true), endsWithFooter(short, true)]
    expect(ends).toEqual([true, false, false, true, true])
    const files = [long, short, null].map((rows, index, all) => file(String(index), rows, { footer: endsWithFooter(rows, index === all.length - 1) }))
    const layout = new DiffLayout(files, CARDS)
    expect([layout.footerHeight(0), layout.footerHeight(1), layout.footerHeight(2)]).toEqual([20, 0, 20])
    // the line between the rows and the bottom border; the gap, then the next card after it
    expect(layout.fileHeight(0)).toBe(48 + long.rowCount * 20 + 20 + 1)
    expect(layout.fileHeight(2)).toBe(48 + 20 + 1)
    const second = 10 + layout.fileHeight(0) + 10
    expect(Array.from(layout.offsets)).toEqual([10, second, second + layout.fileHeight(1) + 10, second + layout.fileHeight(1) + 10 + layout.fileHeight(2) + 10])
    // a top in the line is the place above the next card, as a top in the gap is
    const anchor = layout.anchorAt(layout.rowTop(0, long.rowCount) + 5)!
    expect([anchor.key, layout.scrollTopOf(anchor)]).toEqual(["1", layout.rowTop(0, long.rowCount) + 5])
    // no line without cards
    expect(new DiffLayout(files, { rowHeight: 20, headerHeight: 36 }).footerHeight(0)).toBe(0)
  })

  it("scrolls a review past its end by what its view needs for the card of the last file to reach the top, none where the last card fills the view", () => {
    const short = buildFileRows(ONE_CHANGE, "unified")
    const long = buildFileRows(ONE_CHANGE, "unified", [EXPANDED, EXPANDED])
    const view = 400
    const layout = new DiffLayout([file("a", long), file("b", short)], CARDS, 0, 0, view)
    // scrolled to its end, the card of the last file is at the top, the space below it empty
    expect(layout.pastEnd).toBeGreaterThan(0)
    expect(layout.pastEnd).toBe(view - (layout.contentHeight - layout.cardTop(1)))
    expect(layout.height - view).toBe(layout.cardTop(1))
    expect(layout.fileAtTop(layout.height - view)).toBe(1)
    expect(layout.fileShownAt(layout.height - view, view)).toBe(1)
    // a last card as high as the view needs no space; a view that ends at its end has none
    expect(new DiffLayout([file("b", short), file("a", long)], CARDS, 0, 0, view).pastEnd).toBe(0)
    expect(new DiffLayout([file("a", long), file("b", short)], CARDS).pastEnd).toBe(0)
    // below an epilogue its end less a row reaches the top, so the top of its last opening does
    const ending = new DiffLayout([file("a", long), file("b", short)], CARDS, 0, 120, view)
    expect(ending.height - view).toBe(ending.epilogueTop + 120 - 20 - 10)
    // an overview alone: nothing to scroll to
    expect(new DiffLayout([], CARDS, 200, 0, view).pastEnd).toBe(0)
    // a last card below the heading of its group: scrolled to the end, the heading at the top whole, never cut
    const grouped = new DiffLayout([file("a", long), file("b", short, { divider: true })], CARDS, 0, 0, view)
    expect(grouped.height - view).toBe(grouped.stopTop(1))
    expect(grouped.stopTop(1)).toBe(grouped.fileTop(1))
  })

  it("keeps a strip for the scrollbar of a file wider than its columns below its rows, above the line at the end, which the offsets count", () => {
    const rows = buildFileRows(ONE_CHANGE, "unified")
    const files = [file("wide", rows, { scrollbar: true, footer: true }), file("narrow", rows), file("collapsed", null, { scrollbar: true })]
    const layout = new DiffLayout(files, CARDS)
    // the strip under the last row, so the scrollbar never lies over it; a collapsed file shows no rows and no bar
    expect([layout.scrollbarHeight(0), layout.scrollbarHeight(1), layout.scrollbarHeight(2)]).toEqual([SCROLLBAR_HEIGHT, 0, 0])
    expect(layout.fileHeight(0)).toBe(48 + rows.rowCount * 20 + SCROLLBAR_HEIGHT + 20 + 1)
    expect(layout.fileHeight(1)).toBe(48 + rows.rowCount * 20 + 1)
    expect(layout.offsets[1]).toBe(10 + layout.fileHeight(0) + 10)
    // the rows keep their places: the strip is after the last of them
    expect(layout.rowTop(0, rows.rowCount - 1)).toBe(10 + 48 + (rows.rowCount - 1) * 20)
  })
})
