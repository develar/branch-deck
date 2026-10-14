import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"
import { describeIntegrationCommit, describeMergedBranch, describeUnmergedBranch } from "@/lib/integration-commit"

const time = Date.parse("2026-10-14T10:00:00Z") / 1000
const commit = { id: "0123456789abcdef0123456789abcdef01234567", author: "Jane Doe", committer: "Safe Push Robot", time }

describe("the merge commit of an archived branch", () => {
  beforeEach(() => {
    vi.useFakeTimers()
    vi.setSystemTime(new Date("2026-10-14T12:00:00Z"))
  })
  afterEach(() => {
    vi.useRealTimers()
  })

  it("names the short hash, the committer and how long ago", () => {
    expect(describeIntegrationCommit(commit)).toBe("Merged as 01234567 by Safe Push Robot, 2 hours ago")
  })

  it("names the commit count and the merge commit", () => {
    expect(describeMergedBranch(3, commit)).toBe("3 commits, merged as 01234567 by Safe Push Robot, 2 hours ago")
    expect(describeMergedBranch(1, commit)).toBe("1 commit, merged as 01234567 by Safe Push Robot, 2 hours ago")
  })

  it("says what it knows when the count or the merge commit is unknown", () => {
    expect(describeMergedBranch(0, commit)).toBe("Merged as 01234567 by Safe Push Robot, 2 hours ago")
    expect(describeMergedBranch(2, undefined)).toBe("2 commits")
    expect(describeMergedBranch(0, undefined)).toBeNull()
  })
})

describe("describeUnmergedBranch", () => {
  it("names the merged part, or says that none merged", () => {
    expect(describeUnmergedBranch(1, 2)).toBe("1 of 2 commits merged; the original commits are no longer in HEAD")
    expect(describeUnmergedBranch(0, 1)).toBe("1 commit, none merged; the original commits are no longer in HEAD")
  })
})
