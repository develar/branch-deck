// @spec spec/commit/commit-row.spec.md
import { describe, expect, it } from "vite-plus/test"
import { hasCommitMessageBody, splitSubjectPrefix } from "@/lib/commit"

describe("splitSubjectPrefix", () => {
  it.each([
    ["feat: add retry", "feat:", "add retry"],
    ["feat(model): switch the model", "feat(model):", "switch the model"],
    ["fix!: drop the old API", "fix!:", "drop the old API"],
    ["docs air: describe the format", "docs air:", "describe the format"],
  ])("splits the conventional-commit prefix off %j", (subject, prefix, text) => {
    expect(splitSubjectPrefix(subject)).toEqual({ prefix, text })
  })

  it("returns a grouping prefix without the parentheses", () => {
    expect(splitSubjectPrefix("(feature-auth) Add login")).toEqual({ prefix: "feature-auth", text: "Add login" })
  })

  it.each(["Add connection retry logic", "Fix: a capitalised word is not a type", "http://example.com is down"])("keeps %j without a prefix", subject => {
    expect(splitSubjectPrefix(subject)).toEqual({ prefix: null, text: subject })
  })
})

describe("hasCommitMessageBody", () => {
  it("is true for a message with lines after the subject", () => {
    expect(hasCommitMessageBody("Subject", "Subject\n\nBody")).toBe(true)
  })

  it("is false for a message that equals the subject", () => {
    expect(hasCommitMessageBody("Subject", "Subject")).toBe(false)
  })

  it("is false for a one-line message that differs from a stripped subject", () => {
    expect(hasCommitMessageBody("Add login", "(feature-auth) Add login")).toBe(false)
  })

  it("is false for an empty message", () => {
    expect(hasCommitMessageBody("", "")).toBe(false)
  })
})
