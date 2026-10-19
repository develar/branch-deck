// @spec spec/commit/commit-files.spec.md
import { describe, expect, it } from "vite-plus/test"
import { copyFile, copyText } from "@/lib/diff/copy-text"
import type { CopyFile } from "@/lib/diff/copy-text"
import { buildLineTable } from "@/lib/diff/line-table"
import { buildFileRows, NEW, OLD } from "@/lib/diff/rows"
import type { DiffViewMode } from "@/stores/settings/diff-view-settings"
import { block, textDiff } from "@/test-support/diff-blocks"

const OLD_TEXT = "a\nb\nc\nd\n"
const NEW_TEXT = "a\nB\nc\nd\n"
const DIFF = textDiff(4, 4, [block(1, 2, 1, 2, { inlinePairs: true, oldRanges: [[0, 1]], newRanges: [[0, 1]] })])

function file(mode: DiffViewMode): CopyFile {
  return { rows: buildFileRows(DIFF, mode), tables: [buildLineTable(OLD_TEXT), buildLineTable(NEW_TEXT)] }
}

describe("copyText", () => {
  it("copies the lines the rows show from the model, cut at both ends, in the order of the diffs", () => {
    // Unified: a, -b, +B, c, d
    const unified = file("unified")
    expect(copyText([unified], { file: 0, row: 4, offset: 1 }, { file: 0, row: 0, offset: 0 }, false, NEW)).toBe("a\nb\nB\nc\nd")
    expect(copyText([unified], { file: 0, row: 1, offset: 0 }, { file: 0, row: 3, offset: 0 }, false, NEW)).toBe("b\nB\n")
  })

  it("copies one side in Split, and the new line of an Inline row without its deleted parts", () => {
    const split = file("split")
    expect(copyText([split], { file: 0, row: 0, offset: 0 }, { file: 0, row: 3, offset: 1 }, true, OLD)).toBe("a\nb\nc\nd")
    expect(copyText([split], { file: 0, row: 0, offset: 0 }, { file: 0, row: 3, offset: 1 }, true, NEW)).toBe("a\nB\nc\nd")
    expect(copyFile(file("inline"), false, NEW)).toBe("a\nB\nc\nd")
  })

  it("copies across files and leaves out a fold and a text not read yet", () => {
    const folded: CopyFile = {
      rows: buildFileRows(textDiff(20, 20, [block(10, 11, 10, 11)]), "split"),
      tables: [null, buildLineTable(Array.from({ length: 20 }, (_, index) => `n${index}\n`).join(""))],
    }
    const text = copyText([file("split"), folded], { file: 0, row: 3, offset: 0 }, { file: 1, row: 2, offset: 2 }, true, NEW)
    // the first row of the second file is its fold
    expect(text).toBe("d\nn6\nn7")
  })
})
