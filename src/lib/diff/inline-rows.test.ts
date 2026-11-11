// @spec openspec/specs/commit/commit-files/spec.md
import { describe, expect, it } from "vite-plus/test"
import { inlineRowText, widestInlineRow } from "@/lib/diff/inline-rows"
import { buildLineTable } from "@/lib/diff/line-table"
import { block, move, run, textDiff } from "@/test-support/diff-blocks"

describe("inlineRowText", () => {
  it("is the new line with the deleted parts of the old line before the inserted parts", () => {
    expect(inlineRowText("val a = string", "val a = name", [8, 14, 2], [8, 12, 2])).toBe("val a = stringname")
    // a pure insertion: the row is the new line
    expect(inlineRowText("val a = state", "val a = state + 1", [13, 13, 0], [13, 17, 0])).toBe("val a = state + 1")
    // ranges that do not align (no engine pairs such lines): the whole old line before the whole new one
    expect(inlineRowText("abcde", "abXde", [0, 1, 2, 2, 3, 2], [2, 3, 2])).toBe("abcdeabXde")
  })
})

describe("widestInlineRow", () => {
  it("counts the row of a similar pair wider than both its lines, a tab to its next stop", () => {
    const oldText = "class A {\n\tval name: FallbackTimeout = 1\n}\n"
    const newText = "class A {\n\tval name: RetryDelay = 1\n}\n"
    const pair = block(1, 2, 1, 2, { inlinePairs: true, oldRanges: [[11, 26, 2]], newRanges: [[11, 21, 2]] })
    // the tab is 4 columns, `val name: ` 10, the deleted and the inserted name 15 and 10, ` = 1` 4
    expect(widestInlineRow(textDiff(4, 4, [pair]), buildLineTable(oldText), buildLineTable(newText), 4)).toBe(43)
  })

  it("counts no row of a block that is no similar pair, that the policy ignores, or that deletes nothing", () => {
    const tables = [buildLineTable("a long old line\n"), buildLineTable("a line\n")] as const
    const widest = (extra: Parameters<typeof block>[4]) => widestInlineRow(textDiff(2, 2, [block(0, 1, 0, 1, extra)]), tables[0], tables[1], 4)
    // Unified rows: each line on its own
    expect(widest({ inlinePairs: false, oldRanges: [[2, 11, 1]], newRanges: [[2, 2, 1]] })).toBe(0)
    expect(widest({ inlinePairs: true, kind: "ignored", oldRanges: [[2, 11, 1]], newRanges: [[2, 2, 1]] })).toBe(0)
    // nothing deleted: the row is the new line
    expect(widest({ inlinePairs: true, oldRanges: [[2, 2, 0]], newRanges: [[2, 6, 0]] })).toBe(0)
    expect(widest({ inlinePairs: true, oldRanges: [[2, 11, 1]], newRanges: [[2, 2, 1]] })).toBe(15)
  })

  it("counts the row of an edited line of a move, the pair of its old line and its new one where the line stands now", () => {
    // `val name = fallback` leaves the old line 2 for the new line 0, its value edited on the way
    const tables = [buildLineTable("a\nb\nval name = fallback\n"), buildLineTable("val name = retry\na\nb\n")] as const
    const blocks = [block(0, 0, 0, 1), block(2, 3, 3, 3)]
    const edited = move(2, 0, 1, { edited: [0], oldRanges: [[11, 19, 2]], newRanges: [[11, 16, 2]] })
    // `val name = ` 11, the deleted and the inserted value 8 and 5
    expect(widestInlineRow(textDiff(4, 4, blocks, { moves: [edited] }), tables[0], tables[1], 4)).toBe(24)
    // a line that moved as it was is its new line, which the widest line of the text counts
    expect(widestInlineRow(textDiff(4, 4, blocks, { moves: [move(2, 0, 1)] }), tables[0], tables[1], 4)).toBe(0)
  })

  it("counts the pairs of a block of the imports alone as a review with its plan shows it too, which draws no move there", () => {
    // `import a.Service` becomes `import a.LegacyService` in place, and its old form is added again two lines below:
    // a pair of the engine, and to the marks a move of the old line 0 to the new line 2
    const tables = [buildLineTable("import a.Service\nimport b\nimport c\n"), buildLineTable("import a.LegacyService\nimport b\nimport a.Service\nimport c\n")] as const
    const pair = block(0, 1, 0, 1, { inlinePairs: true, oldRanges: [[9, 9, 0]], newRanges: [[9, 15, 0]] })
    const renamed = block(0, 1, 0, 1, { inlinePairs: true, oldRanges: [[9, 16, 2]], newRanges: [[9, 22, 2]] })
    const widest = (first: typeof pair, importBlocks: number[]) =>
      widestInlineRow(textDiff(4, 5, [first, block(2, 2, 2, 3)], { importBlocks, moves: [move(0, 2, 1)] }), tables[0], tables[1], 4)
    // outside a review the move takes the old line of the pair: no row of two lines. A review with its plan plays the
    // blocks of the imports alone down and draws no move there, so the pair is a row: `import a.` 9, the deleted and
    // the inserted name 7 and 13
    expect([widest(renamed, []), widest(renamed, [0, 1]), widest(renamed, [1])]).toEqual([0, 29, 29])
    // an edited line of a move into such a block is the row of its two lines outside the review alone: both count
    const edited = move(1, 0, 1, { edited: [0], oldRanges: [[9, 16, 2]], newRanges: [[9, 22, 2]] })
    const blocks = [block(0, 0, 0, 1), block(1, 2, 2, 2)]
    const moved = [buildLineTable("import b\nimport a.Service\nimport c\n"), buildLineTable("import a.LegacyService\nimport b\nimport c\n")] as const
    expect(widestInlineRow(textDiff(4, 4, blocks, { importBlocks: [0, 1], moves: [edited] }), moved[0], moved[1], 4)).toBe(29)
    // a pair that deletes nothing is its new line in either reading
    expect(widest(pair, [0, 1])).toBe(0)
  })

  it("counts no row of a pair in a quiet run, which Inline shows once by its new line", () => {
    const tables = [buildLineTable("a long old line\nb old\n"), buildLineTable("a line\nb\n")] as const
    const pairs = block(0, 2, 0, 2, {
      inlinePairs: true,
      oldRanges: [
        [2, 11, 1],
        [1, 5, 1],
      ],
      newRanges: [
        [2, 2, 1],
        [1, 1, 1],
      ],
    })
    // both lines are rows of a pair: the first is the widest
    expect(widestInlineRow(textDiff(3, 3, [pairs]), tables[0], tables[1], 4)).toBe(15)
    // the first line in a quiet run: the row of the second pair alone
    expect(widestInlineRow(textDiff(3, 3, [pairs], { runs: [run(0, 1, 0, 1)] }), tables[0], tables[1], 4)).toBe(5)
  })
})
