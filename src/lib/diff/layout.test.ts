// @spec spec/commit/commit-files.spec.md
import { describe, expect, it } from "vite-plus/test"
import { DiffLayout, visualLinePrefix } from "@/lib/diff/layout"
import type { LayoutFile } from "@/lib/diff/layout"
import { rowHeightOf } from "@/lib/diff/metrics"
import { buildFileRows, EXPANDED, NEW, OLD, rowOfLine } from "@/lib/diff/rows"
import { visualLineCount, wrapBreaks } from "@/lib/diff/wrap"
import type { TextMetrics } from "@/lib/diff/wrap"
import { block, textDiff } from "@/test-support/diff-blocks"

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

  it("keeps an anchor in a header, and finds a folded line in its fold", () => {
    const layout = new DiffLayout([file("a", buildFileRows(ONE_CHANGE, "unified"))], METRICS)
    expect(layout.anchorAt(12)).toEqual({ key: "a", offset: 12 })
    expect(layout.scrollTopOf({ key: "a", offset: 12 })).toBe(12)
    expect(layout.scrollTopOf({ key: "gone", offset: 12 })).toBeNull()
    expect(layout.scrollTopOf({ key: "a", line: { side: NEW, line: 3 }, offset: 4 })).toBe(36 + 4)
  })
})
