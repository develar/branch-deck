// @spec spec/commit/commit-row.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { MissingCommit } from "@/lib/bindings"
import { formatConflictTitle, formatMissingCommitsReason, formatMissingCommitsTitle, getMissingCommitOwners, labelConflictMarkers } from "@/lib/merge-conflict"

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

describe("labelConflictMarkers", () => {
  const merged = "a\n<<<<<<< 40ee0032\nten\n||||||| afd93b3c\nthirty\n=======\nthirty seconds\n>>>>>>> c82db1ff\n<<<<<<<< not a marker\n"

  it("names the sides in words instead of the commit ids git wrote", () => {
    expect(labelConflictMarkers(merged, { current: "origin/master", base: "before the commit", incoming: "“Show the timeout”" })).toBe(
      "a\n<<<<<<< origin/master\nten\n||||||| before the commit\nthirty\n=======\nthirty seconds\n>>>>>>> “Show the timeout”\n<<<<<<<< not a marker\n",
    )
  })
})

function missing(hash: string, branch: string | null): MissingCommit {
  return { hash, subject: hash, message: hash, authorTime: 0, committerTime: 0, author: "", filesTouched: [], branch }
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
