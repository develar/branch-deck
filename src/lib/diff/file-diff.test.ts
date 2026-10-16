// @spec spec/commit/commit-files.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { FileDiff } from "@/lib/bindings"
import { joinFileDiffHunks, toFileDiffMetadata } from "@/lib/diff/file-diff"

function fileDiff(hunks: string[], oldContent = "", newContent = "", name = "src/a.kt"): FileDiff {
  return {
    oldFile: { fileName: name, fileLang: "kt", content: oldContent },
    newFile: { fileName: name, fileLang: "kt", content: newContent },
    hunks,
  }
}

const GIT_PATCH = [
  "diff --git a/src/a.kt b/src/a.kt",
  "index 1111111..2222222 100644",
  "--- a/src/a.kt",
  "+++ b/src/a.kt",
  "@@ -1,3 +1,3 @@",
  " val a = 1",
  "-val b = 2",
  "+val b = 3",
  " ",
  "",
].join("\n")

describe("joinFileDiffHunks", () => {
  it("keeps a whole git patch as it is, also its last context line of a blank line", () => {
    expect(joinFileDiffHunks(fileDiff([GIT_PATCH]))).toBe(GIT_PATCH)
  })

  it("keeps the file header of hunks that each repeat it once", () => {
    const first = "--- a/src/a.kt\n+++ b/src/a.kt\n@@ -1 +1 @@\n-a\n+b"
    const second = "--- a/src/a.kt\n+++ b/src/a.kt\n@@ -10 +10 @@\n-c\n+d"
    expect(joinFileDiffHunks(fileDiff([first, second]))).toBe("--- a/src/a.kt\n+++ b/src/a.kt\n@@ -1 +1 @@\n-a\n+b\n@@ -10 +10 @@\n-c\n+d\n")
  })

  it("adds a file header to hunks without one", () => {
    expect(joinFileDiffHunks(fileDiff(["@@ -1 +1 @@\n-a\n+b"]))).toBe("--- a/src/a.kt\n+++ b/src/a.kt\n@@ -1 +1 @@\n-a\n+b\n")
  })
})

describe("toFileDiffMetadata", () => {
  it("parses a git patch with its object ids, partial without the contents", () => {
    const metadata = toFileDiffMetadata(fileDiff([GIT_PATCH]))
    expect(metadata?.name).toBe("src/a.kt")
    expect(metadata?.type).toBe("change")
    expect(metadata?.prevObjectId).toBe("1111111")
    expect(metadata?.newObjectId).toBe("2222222")
    expect(metadata?.isPartial).toBe(true)
    expect(metadata?.hunks).toHaveLength(1)
  })

  it("is complete with contents that agree with the hunks, so the unchanged lines can expand", () => {
    const patch = "--- a/src/a.kt\n+++ b/src/a.kt\n@@ -3 +3 @@\n-c\n+C\n"
    const metadata = toFileDiffMetadata(fileDiff([patch], "a\nb\nc\nd\n", "a\nb\nC\nd\n"), { withContents: true })
    expect(metadata?.isPartial).toBe(false)
    expect(metadata?.hunks[0]?.collapsedBefore).toBe(2)
  })

  it("has no hunks for a file without hunks", () => {
    expect(toFileDiffMetadata(fileDiff([]))?.hunks ?? []).toHaveLength(0)
  })
})
