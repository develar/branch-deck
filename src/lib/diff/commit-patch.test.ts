// @spec spec/commit/commit-files.spec.md
import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type { BlobContents, CommitFile, CommitPatch } from "@/lib/bindings"
import { getCommitFileDiff, getCommitFileDiffs, loadCommitFileContents } from "@/lib/diff/commit-patch"

const { calls } = vi.hoisted(() => ({
  calls: { requests: [] as string[][], blobs: [] as BlobContents[], errors: [] as string[] },
}))

vi.mock("@/lib/bindings", () => ({
  commands: {
    getBlobContents: async ({ objectIds }: { objectIds: string[] }) => {
      calls.requests.push(objectIds)
      return { status: "ok", data: calls.blobs }
    },
  },
}))
vi.mock("@/stores/repository", () => import("@/test-support/selected-repository-mock"))
vi.mock("@/lib/command", () => import("@/test-support/invoke-command-mock"))
vi.mock("@/lib/errors", () => ({
  notifyError: (title: string) => calls.errors.push(title),
  getErrorDetails: String,
}))

beforeEach(() => {
  calls.requests = []
  calls.blobs = []
  calls.errors = []
})

const OLD_ID = "1".repeat(40)
const NEW_ID = "2".repeat(40)
const ZERO_ID = "0".repeat(40)

const PATCH: CommitPatch = {
  commitId: "c".repeat(40),
  files: [
    { patch: `diff --git a/bin.dat b/bin.dat\nnew file mode 100644\nindex ${ZERO_ID}..${NEW_ID}\nBinary files /dev/null and b/bin.dat differ\n`, omittedLines: null },
    { patch: `diff --git a/gone.txt b/gone.txt\ndeleted file mode 100644\nindex ${OLD_ID}..${ZERO_ID}\n--- a/gone.txt\n+++ /dev/null\n@@ -1 +0,0 @@\n-gone\n`, omittedLines: null },
    { patch: `diff --git a/huge.txt b/huge.txt\nindex ${OLD_ID}..${NEW_ID} 100644\n--- a/huge.txt\n+++ b/huge.txt\n`, omittedLines: 12345 },
    {
      patch: `diff --git a/src/Old.kt b/src/New.kt\nsimilarity index 90%\nrename from src/Old.kt\nrename to src/New.kt\nindex ${OLD_ID}..${NEW_ID} 100644\n--- a/src/Old.kt\n+++ b/src/New.kt\n@@ -10,3 +10,3 @@ class A {\n a\n-b\n+B\n c\n`,
      omittedLines: null,
    },
  ],
}

function file(path: string, status: CommitFile["status"], oldPath: string | null = null, binary = false): CommitFile {
  return { path, status, oldPath, additions: 1, deletions: 1, binary, oldId: null, newId: null, submodule: false }
}

describe("getCommitFileDiff", () => {
  it("finds the section of each file by its path after the commit", () => {
    const [binary, deleted, huge, renamed] = getCommitFileDiffs(PATCH, [
      file("bin.dat", "added", null, true),
      file("gone.txt", "deleted"),
      file("huge.txt", "modified"),
      file("src/New.kt", "renamed", "src/Old.kt"),
    ])
    expect(binary!.fileDiff?.hunks).toHaveLength(0)
    expect(deleted!.fileDiff?.type).toBe("deleted")
    expect(huge!.omittedLines).toBe(12345)
    expect(huge!.fileDiff?.hunks).toHaveLength(0)
    expect(renamed!.fileDiff?.prevName).toBe("src/Old.kt")
    expect(renamed!.fileDiff?.hunks[0]?.additionStart).toBe(10)
  })

  it("parses a patch once: the same diff for the same file, which an expansion fills in place", () => {
    const renamed = file("src/New.kt", "renamed", "src/Old.kt")
    expect(getCommitFileDiff(PATCH, renamed).fileDiff).toBe(getCommitFileDiff(PATCH, renamed).fileDiff)
  })

  it("has no diff for a file the patch does not list", () => {
    expect(getCommitFileDiff(PATCH, file("other.txt", "modified")).fileDiff).toBeUndefined()
  })
})

describe("loadCommitFileContents", () => {
  it("reads both sides by the blob ids of the patch", async () => {
    calls.blobs = [
      { kind: "text", objectId: OLD_ID, content: "old\n" },
      { kind: "text", objectId: NEW_ID, content: "new\n" },
    ]
    const { fileDiff } = getCommitFileDiff(PATCH, file("src/New.kt", "renamed", "src/Old.kt"))
    const files = await loadCommitFileContents(fileDiff!)
    expect(calls.requests).toEqual([[OLD_ID, NEW_ID]])
    expect(files).toEqual({
      oldFile: { name: "src/Old.kt", contents: "old\n", cacheKey: OLD_ID },
      newFile: { name: "src/New.kt", contents: "new\n", cacheKey: NEW_ID },
    })
  })

  it("does not ask for the side a file does not exist on", async () => {
    calls.blobs = [{ kind: "text", objectId: OLD_ID, content: "gone\n" }]
    const { fileDiff } = getCommitFileDiff(PATCH, file("gone.txt", "deleted"))
    const files = await loadCommitFileContents(fileDiff!)
    expect(calls.requests).toEqual([[OLD_ID]])
    expect(files.newFile.contents).toBe("")
  })

  it("shows why a side cannot be read, and fails the expansion", async () => {
    calls.blobs = [
      { kind: "binary", objectId: OLD_ID },
      { kind: "text", objectId: NEW_ID, content: "" },
    ]
    const { fileDiff } = getCommitFileDiff(PATCH, file("src/New.kt", "renamed", "src/Old.kt"))
    await expect(loadCommitFileContents(fileDiff!)).rejects.toThrow("the file is binary")
    expect(calls.errors).toEqual(["Cannot show the unchanged lines of src/New.kt"])
  })
})
