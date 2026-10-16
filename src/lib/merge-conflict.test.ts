// @spec spec/commit/commit-row.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { FileDiff, MissingCommit } from "@/lib/bindings"
import { filterConflictHunks, formatConflictTitle, formatMissingCommitsReason, formatMissingCommitsTitle, getMissingCommitOwners } from "@/lib/merge-conflict"

describe("formatConflictTitle", () => {
  it("names the subject without its grouping prefix and the branch it is copied onto", () => {
    expect(formatConflictTitle("(bug-fix) Implement secure password hashing\n\nBody", "master")).toBe("“Implement secure password hashing” does not apply cleanly onto master")
  })

  it("keeps an autosquash marker of the subject", () => {
    expect(formatConflictTitle("fixup! (auth) Add login", "auth")).toBe("“fixup! Add login” does not apply cleanly onto auth")
  })

  it('says "the baseline" before the first sync names it', () => {
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

function missing(hash: string, branch: string | null): MissingCommit {
  return { hash, subject: hash, message: hash, authorTime: 0, committerTime: 0, author: "", filesTouched: [], fileDiffs: [], branch }
}

describe("getMissingCommitOwners", () => {
  // a <- b <- c, and d on its own
  const bases: Record<string, string> = { b: "a", c: "b" }
  const baseOf = (name: string) => bases[name] ?? null

  it("groups the commits by branch, the most first, and finds the branch built on all the others", () => {
    const owners = getMissingCommitOwners([missing("1", "a"), missing("2", "c"), missing("3", "c"), missing("4", null)], baseOf)
    expect(owners).toEqual({
      branches: [
        { branch: "c", commitIds: ["2", "3"] },
        { branch: "a", commitIds: ["1"] },
      ],
      unassigned: ["4"],
      stackTop: "c",
    })
    expect(formatMissingCommitsTitle(owners)).toBe("3 commits of c and the branch it is built on changed these files first, as did 1 unassigned commit")
  })

  it("has no stack top when the branches are on two stacks", () => {
    const owners = getMissingCommitOwners([missing("1", "b"), missing("2", "d")], baseOf)
    expect(owners.stackTop).toBeNull()
    expect(formatMissingCommitsTitle(owners)).toBe("2 commits of 2 branches changed these files first")
  })

  it("names a single branch", () => {
    expect(formatMissingCommitsTitle(getMissingCommitOwners([missing("1", "a")], baseOf))).toBe("1 commit of a changed these files first")
  })

  it("says that no commit outside the branch changed the files", () => {
    expect(formatMissingCommitsTitle(getMissingCommitOwners([], baseOf))).toBe("No commit outside the branch changed these files first")
    expect(formatMissingCommitsTitle(getMissingCommitOwners([missing("1", null)], baseOf))).toBe("1 unassigned commit changed these files first")
  })
})

describe("formatMissingCommitsReason", () => {
  const owners = getMissingCommitOwners([missing("1", "a"), missing("2", "a")], () => null)

  it("says what the branch is built on", () => {
    expect(formatMissingCommitsReason(owners, "login", "auth")).toBe("login is built on auth, which doesn't have them.")
    expect(
      formatMissingCommitsReason(
        getMissingCommitOwners([missing("1", null)], () => null),
        "login",
        null,
      ),
    ).toBe("login is built on the baseline, which doesn't have it.")
  })

  it("says nothing for a conflict with the amended commit", () => {
    expect(formatMissingCommitsReason(owners, "login", undefined)).toBeNull()
  })
})
