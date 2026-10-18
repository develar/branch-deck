// @spec spec/commit/commit-review.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { CommitReviewWindowData } from "@/lib/commit-review-window"
import { commitReviewWindowKey } from "@/lib/commit-review-window"

function data(overrides: Partial<CommitReviewWindowData>): CommitReviewWindowData {
  const commits = [
    { id: "a1", subject: "first", message: "first", author: "A", authorTime: 0, committerTime: 0 },
    { id: "b2", subject: "second", message: "second", author: "A", authorTime: 0, committerTime: 0 },
  ]
  return { repositoryPath: "/repo", commits, listLabel: "session", index: 0, path: null, openedAt: 1, ...overrides }
}

describe("the review window of a commit", () => {
  it("keeps its review for an open of the commit it shows, and starts over for another commit, list or file", () => {
    const shown = commitReviewWindowKey(data({}))
    // the same commit of the same list, opened again later
    expect(commitReviewWindowKey(data({ openedAt: 2 }))).toBe(shown)
    expect(commitReviewWindowKey(data({ index: 1 }))).not.toBe(shown)
    expect(commitReviewWindowKey(data({ listLabel: "Unassigned" }))).not.toBe(shown)
    // an open at a file goes there, every time
    const atFile = data({ path: "src/a.kt", openedAt: 3 })
    expect(commitReviewWindowKey(atFile)).not.toBe(shown)
    expect(commitReviewWindowKey({ ...atFile, openedAt: 4 })).not.toBe(commitReviewWindowKey(atFile))
  })
})
