// @spec openspec/specs/commit/commit-files/spec.md
import { describe, expect, it } from "vite-plus/test"
import { copyFile, copyText } from "@/lib/diff/copy-text"
import type { CopyFile } from "@/lib/diff/copy-text"
import { buildLineTable } from "@/lib/diff/line-table"
import { buildFileRows, NEW, OLD } from "@/lib/diff/rows"
import type { DiffViewMode } from "@/stores/settings/diff-view-settings"
import { block, run, textDiff } from "@/test-support/diff-blocks"

const OLD_TEXT = "a\nb\nc\nd\n"
const NEW_TEXT = "a\nB\nc\nd\n"
const DIFF = textDiff(4, 4, [block(1, 2, 1, 2, { inlinePairs: true, oldRanges: [[0, 1, 2]], newRanges: [[0, 1, 2]] })])

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

  it("copies what a quiet run shows: its new lines alone in Inline, its old lines too once its note is opened or where every line stays", () => {
    // `call(1, 2)` wrapped over three lines, a quiet run of formatting; its note is a row of words, which a copy leaves out
    const diff = textDiff(3, 5, [block(1, 2, 1, 4)], { runs: [run(1, 2, 1, 4)] })
    const tables = [buildLineTable("a\ncall(1, 2)\nz\n"), buildLineTable("a\ncall(\n  1, 2\n)\nz\n")]
    const shown = (mode: DiffViewMode, open: number[] = []): CopyFile => ({ rows: buildFileRows(diff, mode, [], false, null, new Set(open)), tables })
    expect(copyFile(shown("inline"), false, NEW)).toBe("a\ncall(\n  1, 2\n)\nz")
    expect(copyFile(shown("inline", [1]), false, NEW)).toBe("a\ncall(1, 2)\ncall(\n  1, 2\n)\nz")
    expect(copyFile(shown("unified"), false, NEW)).toBe("a\ncall(1, 2)\ncall(\n  1, 2\n)\nz")
    expect(copyFile(shown("split"), true, OLD)).toBe("a\ncall(1, 2)\nz")
    // a selection from the note to the second line of the run: the note gives no text
    expect(copyText([shown("inline")], { file: 0, row: 1, offset: 0 }, { file: 0, row: 3, offset: 3 }, false, NEW)).toBe("call(\n  1")
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
