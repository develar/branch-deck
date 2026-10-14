// @spec spec/commit/commit-files.spec.md
import { describe, expect, it } from "vitest"
import type { CommitFile, CommitFiles } from "@/lib/bindings"
import { formatCommitFilePath, formatFileStatus, getChangedPaths, getCommitFileStats } from "@/lib/commit-files"

function file(path: string, oldPath: string | null = null): CommitFile {
  return { path, status: oldPath === null ? "modified" : "renamed", oldPath, additions: 1, deletions: 0, binary: false }
}

function commitFiles(commitId: string, files: CommitFile[]): CommitFiles {
  return { commitId, files, additions: files.length, deletions: 0 }
}

describe("getChangedPaths", () => {
  it("lists each path once, in the order of the commits, a renamed file by its new path", () => {
    const paths = getChangedPaths([
      commitFiles("c1", [file("b.txt"), file("new.txt", "old.txt")]),
      commitFiles("c2", [file("a.txt"), file("b.txt")]),
    ])
    expect(paths).toEqual(["b.txt", "new.txt", "a.txt"])
  })

  it("is empty for commits without files", () => {
    expect(getChangedPaths([commitFiles("c1", [])])).toEqual([])
  })
})

describe("getCommitFileStats", () => {
  it("counts the files and takes the sums of the line counts", () => {
    expect(getCommitFileStats({ commitId: "c1", files: [file("a.txt"), file("b.txt")], additions: 42, deletions: 7 })).toEqual({ count: 2, additions: 42, deletions: 7 })
  })
})

describe("formatCommitFilePath", () => {
  it("shows a rename as old → new", () => {
    expect(formatCommitFilePath(file("src/new.ts", "src/old.ts"))).toBe("src/old.ts → src/new.ts")
  })

  it("shows the path of another file", () => {
    expect(formatCommitFilePath(file("src/a.ts"))).toBe("src/a.ts")
  })
})

describe("formatFileStatus", () => {
  it.each([["added", "added"], ["renamed", "renamed"], ["typeChanged", "type changed"]] as const)("labels %s as %s", (status, label) => {
    expect(formatFileStatus(status)).toBe(label)
  })
})
