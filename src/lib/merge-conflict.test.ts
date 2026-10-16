// @spec spec/commit/commit-row.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { FileDiff } from "@/lib/bindings"
import { filterConflictHunks, formatConflictTitle } from "@/lib/merge-conflict"

describe("formatConflictTitle", () => {
  it("names the subject without its grouping prefix and the branch it is copied onto", () => {
    expect(formatConflictTitle("(bug-fix) Implement secure password hashing\n\nBody", "master"))
      .toBe("“Implement secure password hashing” does not apply cleanly onto master")
  })

  it("keeps an autosquash marker of the subject", () => {
    expect(formatConflictTitle("fixup! (auth) Add login", "auth")).toBe("“fixup! Add login” does not apply cleanly onto auth")
  })

  it("says \"the baseline\" before the first sync names it", () => {
    expect(formatConflictTitle("Add login", null)).toBe("“Add login” does not apply cleanly onto the baseline")
  })
})

function fileDiff(fileName: string, hunks: string[]): FileDiff {
  return {
    oldFile: { fileName, fileLang: "kt", content: "" },
    newFile: { fileName, fileLang: "kt", content: "" },
    hunks,
  }
}

describe("filterConflictHunks", () => {
  const conflictHunk = "@@ -1,3 +1,7 @@\n context\n+<<<<<<< HEAD\n+ours\n+=======\n+theirs\n+>>>>>>> 90f6fd48\n"
  const plainHunk = "@@ -10,2 +14,3 @@\n context\n+added\n"

  it("keeps only the hunks with conflict markers", () => {
    const [result] = filterConflictHunks([fileDiff("A.kt", [plainHunk, conflictHunk])])
    expect(result).toEqual({ diff: fileDiff("A.kt", [conflictHunk]), isWholeFile: false })
  })

  it("keeps the whole diff of a file without conflict markers, so no conflicting file disappears", () => {
    const newFile = fileDiff("UserService.kt", [plainHunk])
    const results = filterConflictHunks([newFile, fileDiff("A.kt", [conflictHunk])])
    expect(results).toHaveLength(2)
    expect(results[0]).toEqual({ diff: newFile, isWholeFile: true })
    // the same object: the diff view does not parse it again
    expect(results[0]!.diff).toBe(newFile)
  })
})
