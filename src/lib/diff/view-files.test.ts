// @spec openspec/specs/commit/commit-files/spec.md
import { describe, expect, it } from "vite-plus/test"
import type { CommitFile } from "@/lib/bindings"
import type { FileDiff } from "@/lib/diff/file-diff-model"
import { buildFileRows } from "@/lib/diff/rows"
import { fileNotice, formatHiddenLines, lineEndsNotice, withEngineCounts } from "@/lib/diff/view-files"
import type { BlobTextEntry } from "@/stores/blob-texts"
import { block, run, textDiff } from "@/test-support/diff-blocks"

const OLD_ID = "1".repeat(40)
const NEW_ID = "2".repeat(40)

function gitFile(path = "a.kt"): CommitFile {
  // git numstat counts what the policy ignores too
  return { path, status: "modified", oldPath: null, additions: 9, deletions: 9, binary: false, oldId: OLD_ID, newId: NEW_ID, submodule: false }
}

function textFile(extra: Partial<ReturnType<typeof textDiff>> = {}): FileDiff {
  return {
    path: "a.kt",
    oldPath: null,
    status: "modified",
    oldId: OLD_ID,
    newId: NEW_ID,
    content: { kind: "text", ...textDiff(2, 2, [block(1, 2, 1, 2)]), additions: 1, deletions: 1, ...extra },
    collapse: null,
    reading: { changes: 1, toRead: 1, toReadPastImports: 1 },
  }
}

const read = (text: string): BlobTextEntry => ({ kind: "loaded", blob: { kind: "text", id: "x", text } })
const BOTH_READ = new Map<string, BlobTextEntry>([
  [OLD_ID, read("a\nb\n")],
  [NEW_ID, read("a\nB\n")],
])

function noticeOf(diff: FileDiff | undefined, texts: ReadonlyMap<string, BlobTextEntry> = BOTH_READ): string | null {
  const rows = diff?.content.kind === "text" ? buildFileRows(diff.content, "split") : null
  return fileNotice(diff, rows, texts)
}

describe("withEngineCounts", () => {
  it("takes the line counts of the engine, and none before its diff is read", () => {
    expect(withEngineCounts([gitFile()], [textFile()])[0]).toMatchObject({ additions: 1, deletions: 1 })
    expect(withEngineCounts([gitFile()], undefined)[0]).toMatchObject({ additions: 0, deletions: 0 })
    expect(withEngineCounts([gitFile()], [{ ...textFile(), content: { kind: "binary" } }])[0]).toMatchObject({ additions: 0, deletions: 0 })
  })

  it("counts the lines the whitespace policy hides once the diff is read: of each ignored block the most of a side", () => {
    // a removed blank line and a line indented again, both ignored, beside a change it counts
    const ignored = {
      kind: "text" as const,
      ...textDiff(9, 8, [block(1, 2, 1, 1, { kind: "ignored" }), block(4, 5, 3, 4, { kind: "ignored" }), block(6, 7, 5, 6)]),
      additions: 1,
      deletions: 1,
    }
    const [counted] = withEngineCounts([gitFile()], [{ ...textFile(), content: ignored }])
    expect(counted).toMatchObject({ additions: 1, deletions: 1, hiddenLines: 2 })
    expect(formatHiddenLines(counted!.hiddenLines!)).toBe("2 lines hidden by Whitespace")
    expect(formatHiddenLines(1)).toBe("1 line hidden by Whitespace")
    // none before the diff is read, none of a diff the policy hides nothing of
    expect(withEngineCounts([gitFile()], undefined)[0]!.hiddenLines).toBeUndefined()
    expect(withEngineCounts([gitFile()], [textFile()])[0]!.hiddenLines).toBeUndefined()
  })
})

describe("what is left to read of a file", () => {
  // a wrapped call (a quiet run of formatting), a block of the imports alone and a real change, with what Rust says
  // is left to read of them (crates/file-diffs/src/reading_test.rs counts the same diff)
  const marked = {
    kind: "text" as const,
    ...textDiff(30, 32, [block(1, 2, 1, 2), block(10, 11, 10, 13), block(20, 21, 22, 23)], { runs: [run(10, 11, 10, 13)], importBlocks: [0] }),
    additions: 5,
    deletions: 3,
  }
  const markedFile: FileDiff = { ...textFile(), content: marked, reading: { changes: 3, toRead: 2, toReadPastImports: 1 } }

  it("takes what Rust says is left to read of a file where some change is quiet, and nothing where every change is read", () => {
    const [counted] = withEngineCounts([gitFile()], [markedFile])
    expect(counted).toMatchObject({ additions: 5, deletions: 3, toRead: { toRead: 2, changes: 3 } })
    // every change a stop: no count, the file as it was
    expect(withEngineCounts([gitFile()], [textFile()])[0]).not.toHaveProperty("toRead")
    expect(withEngineCounts([gitFile()], undefined)[0]).not.toHaveProperty("toRead")
    // a file that is no text has no reading
    expect(withEngineCounts([gitFile()], [{ ...textFile(), content: { kind: "binary" }, reading: null }])[0]).not.toHaveProperty("toRead")
    // nothing to read of any change is a count too: "Nothing to read"
    expect(withEngineCounts([gitFile()], [{ ...markedFile, reading: { changes: 3, toRead: 0, toReadPastImports: 0 } }])[0]!.toRead).toEqual({ toRead: 0, changes: 3 })
  })

  it("passes a block of the imports alone too in a review with its plan, as N and P do there", () => {
    const [counted] = withEngineCounts([gitFile()], [markedFile], true)
    expect(counted!.toRead).toEqual({ toRead: 1, changes: 3 })
    // the same stops as the rows of the file have, from the same starts
    const rows = buildFileRows(marked, "inline", [], true)
    expect([rows.changeStarts.length, rows.changes]).toEqual([1, 3])
    expect([buildFileRows(marked, "inline").changeStarts.length, buildFileRows(marked, "inline").changes]).toEqual([2, 3])
  })

  it("counts no block without a line: the stand-in of a use a change left outside is no change", () => {
    const ghost = { kind: "text" as const, ...textDiff(9, 9, [block(4, 4, 4, 4)], { importBlocks: [0] }), additions: 0, deletions: 0 }
    const content = { ...ghost, marks: { ...ghost.marks, starts: [] } }
    expect(
      withEngineCounts([{ ...gitFile(), additions: 0, deletions: 0 }], [{ ...textFile(), content, reading: { changes: 0, toRead: 0, toReadPastImports: 0 } }], true)[0],
    ).not.toHaveProperty("toRead")
    const rows = buildFileRows(content, "inline", [], true)
    expect([rows.changeStarts, rows.changes]).toEqual([[], 0])
  })
})

describe("fileNotice", () => {
  it("says why a file shows no lines: binary, too large, a submodule, a blob git cannot read, a text that cannot be read, no changed lines", () => {
    expect(noticeOf(textFile())).toBeNull()
    expect(noticeOf({ ...textFile(), content: { kind: "binary" } })).toBe("Binary file")
    expect(noticeOf({ ...textFile(), content: { kind: "tooLarge", oldBytes: 9 * 1024 * 1024, newBytes: 0 } })).toBe("Too large to show: 9 MB")
    expect(noticeOf({ ...textFile(), content: { kind: "submodule", oldCommit: "a".repeat(40), newCommit: null } })).toBe("Submodule: aaaaaaaa → none")
    expect(noticeOf({ ...textFile(), content: { kind: "missing" } })).toBe("Git cannot read the file: the clone is partial or shallow")
    expect(noticeOf(textFile(), new Map([[OLD_ID, { kind: "error", message: "gone" }]]))).toBe("Cannot read the file: gone")
    // a text still read gives no reason yet
    expect(noticeOf(textFile(), new Map([[OLD_ID, { kind: "loading" }]]))).toBeNull()
    expect(noticeOf(undefined)).toBe("No changed lines")
    expect(noticeOf(textFile({ blocks: [block(1, 2, 1, 2, { kind: "ignored" })] }))).toBe("No changed lines")
  })

  it("names a change of the line ends, and says so when it is the only change", () => {
    const lineEnds = { old: "lf", new: "crlf", only: false } as const
    expect(noticeOf(textFile({ lineEnds }))).toBe("Line endings: LF → CRLF")
    expect(noticeOf(textFile({ lineEnds: { ...lineEnds, only: true }, blocks: [] }))).toBe("Only line endings changed: LF → CRLF")
    expect(lineEndsNotice(textDiff(1, 1, []))).toBeNull()
  })
})
