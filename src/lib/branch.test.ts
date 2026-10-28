// @spec spec/branch/branch-row.spec.md
import { describe, expect, it } from "vite-plus/test"
import { formatBranchesAndCommits, getBranchSummary } from "@/lib/branch"

describe("getBranchSummary", () => {
  it("shows the description title as written, prefix and all", () => {
    expect(getBranchSummary("feat: add retry", "vm: the guest agent runs the lane daemon")).toEqual({ prefix: null, text: "vm: the guest agent runs the lane daemon" })
  })

  it("splits the conventional-commit prefix off a subject, to be dimmed", () => {
    expect(getBranchSummary("feat: add retry", null)).toEqual({ prefix: "feat:", text: "add retry" })
  })

  it("leaves out the issue key the row shows already, at which Rust split the title", () => {
    const title = "IJPL-258254 Completion re-points a stale injected original"
    const lead = { key: "IJPL-258254", text: "Completion re-points a stale injected original" }
    expect(getBranchSummary("", title, "IJPL-258254", lead)).toEqual({ prefix: null, text: "Completion re-points a stale injected original" })
    // a key the row doesn't show stays, and so does a title that starts with another key
    expect(getBranchSummary("", title, null, lead)?.text).toBe(title)
    expect(getBranchSummary("", "IJPL-1 Fix the cache", "IJPL-2", { key: "IJPL-1", text: "Fix the cache" })?.text).toBe("IJPL-1 Fix the cache")
    // a title that is the key alone says nothing more
    expect(getBranchSummary("", "IJPL-1", "IJPL-1", { key: "IJPL-1", text: "" })).toBeNull()
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
