// @spec spec/branch/branch-review.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { BranchReviewSegment, ReviewCommit } from "@/lib/bindings"
import { SINCE_REVIEW, isSingleCommit, keepCommit, pullRequestItems, rangePosition, segmentItems, shownKey } from "@/lib/range-review"

function commit(id: string, subject: string, authorTime = 1): ReviewCommit {
  return { id, subject, message: subject, author: "Test User", authorTime, committerTime: authorTime }
}

function segment(name: string, commits: ReviewCommit[]): BranchReviewSegment {
  return { name, base: `${name}-base`, head: commits[0]!.id, commits }
}

describe("segmentItems", () => {
  it("lists all changes, then the commits of the reviewed branch first, each branch headed by its name", () => {
    const items = segmentItems([segment("auth", [commit("a1", "Add the token")]), segment("login", [commit("l2", "Style"), commit("l1", "Add the form")])])
    expect(items.map(item => (item.kind === "scope" ? `${item.label} ${item.detail}` : `${item.group ?? ""}:${item.commit.subject}`))).toEqual([
      "All Changes 3 commits",
      "login:Style",
      ":Add the form",
      "auth:Add the token",
    ])
    expect(isSingleCommit(items)).toBe(false)
    // one branch: no headings; one commit: nothing to choose
    expect(isSingleCommit(segmentItems([segment("auth", [commit("a1", "Add the token")])]))).toBe(true)
  })
})

describe("keepCommit", () => {
  it("keeps the commit by its id, else the one a sync rebuilt in its place", () => {
    const before = [segment("login", [commit("l3", "fixup! Add", 5), commit("l2", "fixup! Add", 5), commit("l1", "Add", 4)])]
    expect(keepCommit(before, before, "l2")).toBe("l2")
    expect(keepCommit(before, before, null)).toBeNull()
    // rebuilt: new ids; the second of two twins stays the second
    const after = [segment("login", [commit("m3", "fixup! Add", 5), commit("m2", "fixup! Add", 5), commit("m1", "Add", 4)])]
    expect(keepCommit(before, after, "l2")).toBe("m2")
    expect(keepCommit(before, after, "l1")).toBe("m1")
    // gone: all changes
    expect(keepCommit(before, [segment("login", [commit("n1", "Other", 9)])], "l1")).toBeNull()
  })
})

describe("rangePosition", () => {
  it("counts the commits, or names the scope", () => {
    const items = segmentItems([segment("login", [commit("l2", "Style"), commit("l1", "Add the form")])])
    expect(rangePosition(items, "l1")).toEqual({ commit: { index: 1, count: 2 } })
    expect(rangePosition(items, "all")).toEqual({ scope: "All Changes" })
    // one commit: no column, nothing to place
    expect(rangePosition(segmentItems([segment("auth", [commit("a1", "Add the token")])]), "all")).toEqual({})
  })
})

describe("pullRequestItems", () => {
  const commits = [commit("c1", "First"), commit("c2", "Second"), commit("c3", "Third")]

  it("offers what changed since the user's review, the reviewed commit under the boundary", () => {
    const items = pullRequestItems(commits, "c1")
    expect(items.map(item => (item.kind === "scope" ? `${item.label} ${item.detail}` : `${item.divider ?? ""}:${item.commit.subject}`))).toEqual([
      "All Changes 3 commits",
      "Since Your Review 2 commits",
      ":Third",
      ":Second",
      "Your review · c1:First",
    ])
  })

  it("has nothing since a review of the head, or of no commit of the range", () => {
    expect(pullRequestItems(commits, "c3").map(item => item.key)).toEqual(["all", "c3", "c2", "c1"])
    expect(pullRequestItems(commits, "elsewhere").map(item => item.key)).toEqual(["all", "c3", "c2", "c1"])
    // the scope that left falls back to all changes
    expect(shownKey(pullRequestItems(commits, "c3"), SINCE_REVIEW)).toBe("all")
    expect(shownKey(pullRequestItems(commits, "c1"), SINCE_REVIEW)).toBe(SINCE_REVIEW)
  })
})
