// @spec spec/branch/branch-row.spec.md
import { describe, expect, it } from "vitest"
import { formatBranchesAndCommits, getBranchSummary } from "@/lib/branch"

describe("getBranchSummary", () => {
  it("shows the description title as written, prefix and all", () => {
    expect(getBranchSummary("feat: add retry", "vm: the guest agent runs the lane daemon")).toEqual({ prefix: null, text: "vm: the guest agent runs the lane daemon" })
  })

  it("splits the conventional-commit prefix off a subject, to be dimmed", () => {
    expect(getBranchSummary("feat: add retry", null)).toEqual({ prefix: "feat:", text: "add retry" })
  })

  it("shows nothing without a description and a summary", () => {
    expect(getBranchSummary("", null)).toBeNull()
  })
})

describe("formatBranchesAndCommits", () => {
  it("formats the branches and their commits in one summary", () => {
    expect(formatBranchesAndCommits(22, 152)).toBe("22 branches · 152 commits")
    expect(formatBranchesAndCommits(1, 1)).toBe("1 branch · 1 commit")
  })
})
