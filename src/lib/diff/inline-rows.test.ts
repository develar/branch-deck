// @spec openspec/specs/commit/commit-files/spec.md
import { describe, expect, it } from "vite-plus/test"
import { inlineRowText, widestInlineRow } from "@/lib/diff/inline-rows"
import { buildLineTable } from "@/lib/diff/line-table"
import { block, run, textDiff } from "@/test-support/diff-blocks"

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
