// @spec spec/branch/branch-review.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { BranchReviewSegment, Interdiff, ReviewCommit, ReviewPlan } from "@/lib/bindings"
import { SINCE_REVIEW, isSingleCommit, keepCommit, localSinceReview, pullRequestItems, rangePosition, segmentItems, shownInterdiff, shownKey } from "@/lib/range-review"

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

describe("the local Since Your Review", () => {
  // the interdiff of a branch marked reviewed at r1..r3, oldest first: r1 amended (c1), r2 unchanged (c2), c4 added
  const interdiff: Interdiff = {
    reviewedBase: "base",
    reviewedTip: "r3",
    commits: [
      { id: "c1", reviewedId: "r1", subject: "Add the token", state: "rewritten", changes: { newOnly: [], reviewedOnly: [] } },
      { id: "c2", reviewedId: "r2", subject: "Style", state: "unchanged", changes: null },
      { id: "c4", reviewedId: null, subject: "Log out", state: "added", changes: null },
      { id: null, reviewedId: "r3", subject: "Typo", state: "dropped", changes: null },
    ],
    scope: null,
  }
  const branch = [segment("auth", [commit("c4", "Log out"), commit("c2", "Style"), commit("c1", "Add the token")])]
  const words = (items: ReturnType<typeof segmentItems>) =>
    items.map(item => (item.kind === "scope" ? `${item.label} ${item.detail}` : `${item.commit.subject}${item.since === undefined ? "" : ` (${item.since})`}`))

  it("takes the interdiff only of the part's own branch, and only of a plan of the head on screen", () => {
    const plan: ReviewPlan = { branch: "user/virtual/auth", base: "base", head: "c4", interdiff, api: [], risk: [], order: [], indexNote: null }
    expect(shownInterdiff(plan, true, "c4")).toBe(interdiff)
    // a part of several branches: the pair is on the base of the upper branch alone, the lower commits are no news
    expect(shownInterdiff(plan, false, "c4")).toBeNull()
    // the plan before a sync moved the head, kept while the next is read
    expect(shownInterdiff(plan, true, "c5")).toBeNull()
    expect(shownInterdiff(null, true, "c4")).toBeNull()
  })

  it("offers no scope for a branch never marked, nor when nothing was added or rewritten", () => {
    expect(localSinceReview(null)).toBeNull()
    expect(localSinceReview({ ...interdiff, commits: [interdiff.commits[1]!], scope: { base: "c2", head: "c2" } })).toBeNull()
  })

  it("marks the added and the rewritten commits, and lists only them, adjacent or not, under the scope", () => {
    const since = localSinceReview(interdiff)!
    expect(since.newestFirst).toEqual(["c4", "c1"])
    expect(since.scope).toBeNull()
    expect(words(segmentItems(branch, since))).toEqual(["All Changes 3 commits", "Since Your Review 2 commits", "Log out (New)", "Style", "Add the token (Rewritten)"])
    expect(words(segmentItems(branch, since, true))).toEqual(["All Changes 3 commits", "Since Your Review 2 commits", "Log out (New)", "Add the token (Rewritten)"])
  })

  it("keeps the plain range of the commits added on top, and the column of one new commit", () => {
    const onTop = localSinceReview({ ...interdiff, commits: [interdiff.commits[1]!, interdiff.commits[2]!], scope: { base: "c2", head: "c4" } })!
    expect(onTop.scope).toEqual({ base: "c2", head: "c4" })
    const items = segmentItems([segment("auth", [commit("c4", "Log out")])], onTop)
    expect(words(items)).toEqual(["All Changes 1 commit", "Since Your Review 1 commit", "Log out (New)"])
    // two scopes to choose from: the column stays
    expect(isSingleCommit(items)).toBe(false)
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
