// @spec spec/commit/commit-row.spec.md
import { describe, expect, it } from "vite-plus/test"
import { composeResult, middleLineCount, resolvedLines, resultSourceOf, resultTextId } from "@/lib/diff/file-merge-model"
import { buildLineTable } from "@/lib/diff/line-table"
import { buildMergeRows, MIDDLE, rowAt } from "@/lib/diff/rows"
import { mergeBlock, textMerge } from "@/test-support/diff-blocks"

const LEFT_TEXT = "host\nlimits = { timeout: 10, retries: 1 }\nport\n"
const BASE_TEXT = "host\nlimits = { timeout: 30, retries: 1 }\nport\n"
const RIGHT_TEXT = "host\nlimits = { timeout: 30, retries: 3 }\nport\nverbose\n"

describe("the result of a merge", () => {
  it("takes the side that changed a block, the resolve by words, or the base of a conflict that does not resolve", () => {
    expect(resultSourceOf(mergeBlock([0, 1], [0, 1], [0, 0], { kind: "deleted", leftChanged: false, resolution: "default" }))).toBe("right")
    expect(resultSourceOf(mergeBlock([0, 2], [0, 1], [0, 1], { kind: "modified", rightChanged: false, resolution: "default" }))).toBe("left")
    expect(resultSourceOf(mergeBlock([0, 1], [0, 1], [0, 1], { resolution: "text", resolvedText: "x" }))).toBe("text")
    expect(resultSourceOf(mergeBlock([0, 1], [0, 1], [0, 1]))).toBe("base")
    expect(middleLineCount(mergeBlock([0, 3], [0, 1], [0, 2]), "base")).toBe(1)
    expect(middleLineCount(mergeBlock([0, 3], [0, 1], [0, 2], { resolution: "text", resolvedText: "a\r\nb" }), "result")).toBe(2)
    expect(resolvedLines("")).toEqual([])
  })

  it("composes the result text with the line indices of the middle column", () => {
    const resolved = mergeBlock([1, 2], [1, 2], [1, 2], { resolution: "text", resolvedText: "limits = { timeout: 10, retries: 3 }" })
    const added = mergeBlock([3, 3], [3, 3], [3, 4], { kind: "inserted", leftChanged: false, resolution: "default" })
    const merge = textMerge([4, 4, 5], [resolved, added], { emptyLastLine: { left: true, base: true, right: true } })
    const result = composeResult(merge, [buildLineTable(LEFT_TEXT), buildLineTable(BASE_TEXT), buildLineTable(RIGHT_TEXT)])
    expect(result).toBe("host\nlimits = { timeout: 10, retries: 3 }\nport\nverbose\n")
    const table = buildLineTable(result)
    const rows = buildMergeRows(merge, "result")
    const middleLines = Array.from({ length: rows.rowCount }, (_, row) => rowAt(rows, row)).map(row => (row.kind === "fold" ? null : row.lines[MIDDLE]))
    expect(middleLines).toEqual([0, 1, 2, 3])
    expect(table.lineCount).toBe(5)
  })

  it("gives each merge object its own result id", () => {
    const merge = textMerge([1, 1, 1], [])
    expect(resultTextId(merge)).toBe(resultTextId(merge))
    expect(resultTextId(textMerge([1, 1, 1], []))).not.toBe(resultTextId(merge))
  })
})
