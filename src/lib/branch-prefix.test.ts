// @spec spec/repository/onboarding.spec.md
import { describe, expect, it } from "vite-plus/test"
import { isValidBranchPrefix, normalizeBranchPrefix } from "@/lib/branch-prefix"

describe("normalizeBranchPrefix", () => {
  it.each([
    ["jane", "jane"],
    ["jane/", "jane"],
    [" jane// ", "jane"],
    ["team/jane", "team/jane"],
    ["  ", ""],
    [undefined, ""],
  ])("normalizes %j to %j, as the backend names the branch", (input, expected) => {
    expect(normalizeBranchPrefix(input)).toBe(expected)
  })
})

describe("isValidBranchPrefix", () => {
  it("accepts names git takes as a ref part", () => {
    expect(isValidBranchPrefix("jane.doe")).toBe(true)
    expect(isValidBranchPrefix("team/jane")).toBe(true)
  })

  it.each(["", "jane doe", "a..b", "a:b", "a~1", "a^", "a?", "a*", "a[b", "a\\b", "a@{b"])("refuses %j", prefix => {
    expect(isValidBranchPrefix(prefix)).toBe(false)
  })
})
