// @spec spec/commit/commit-files.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { CommitFile } from "@/lib/bindings"
import type { FileDiff } from "@/lib/diff/file-diff-model"
import { buildFileRows } from "@/lib/diff/rows"
import { fileNotice, lineEndsNotice, withEngineCounts } from "@/lib/diff/view-files"
import type { BlobTextEntry } from "@/stores/blob-texts"
import { block, textDiff } from "@/test-support/diff-blocks"

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
