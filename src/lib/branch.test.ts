// @spec spec/branch/branch-row.spec.md
import { describe, expect, it } from "vitest"
import { getBranchSummary } from "@/lib/branch"

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
