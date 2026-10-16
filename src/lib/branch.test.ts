// @spec spec/branch/branch-row.spec.md
import { describe, expect, it } from "vite-plus/test"
import { formatBranchesAndCommits, getBranchSummary, getShownIssueKey } from "@/lib/branch"

describe("getBranchSummary", () => {
  it("shows the description title as written, prefix and all", () => {
    expect(getBranchSummary("feat: add retry", "vm: the guest agent runs the lane daemon")).toEqual({ prefix: null, text: "vm: the guest agent runs the lane daemon" })
  })

  it("splits the conventional-commit prefix off a subject, to be dimmed", () => {
    expect(getBranchSummary("feat: add retry", null)).toEqual({ prefix: "feat:", text: "add retry" })
  })

  it("leaves out the issue key the row shows already, which a title starts with", () => {
    expect(getBranchSummary("", "IJPL-258254 Completion re-points a stale injected original", "IJPL-258254")).toEqual({ prefix: null, text: "Completion re-points a stale injected original" })
    // a key the row doesn't show stays, and so does a title that starts with another key
    expect(getBranchSummary("", "IJPL-258254 Completion re-points a stale injected original")?.text).toBe("IJPL-258254 Completion re-points a stale injected original")
    expect(getBranchSummary("", "IJPL-1 Fix the cache", "IJPL-2")?.text).toBe("IJPL-1 Fix the cache")
    // a title that is the key alone says nothing more
    expect(getBranchSummary("", "IJPL-1", "IJPL-1")).toBeNull()
  })

  it("shows nothing without a description and a summary", () => {
    expect(getBranchSummary("", null)).toBeNull()
  })
})

describe("getShownIssueKey", () => {
  it("is the chip of the row, else the key the name starts with", () => {
    expect(getShownIssueKey("IJAI-4075", null)).toBe("IJAI-4075")
    expect(getShownIssueKey("user-name/archived/2024-01-02/IJAI-4075", null)).toBe("IJAI-4075")
    expect(getShownIssueKey("feature-auth", "JIRA-123")).toBe("JIRA-123")
    expect(getShownIssueKey("build-wrappers", null)).toBeNull()
  })
})

describe("formatBranchesAndCommits", () => {
  it("formats the branches and their commits in one summary", () => {
    expect(formatBranchesAndCommits(22, 152)).toBe("22 branches · 152 commits")
    expect(formatBranchesAndCommits(1, 1)).toBe("1 branch · 1 commit")
  })
})
