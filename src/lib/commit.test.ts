// @spec spec/commit/commit-row.spec.md
import { describe, expect, it } from "vitest"
import { getCommitMessageBody, getGroupingPrefix, hasCommitMessageBody, splitSubjectPrefix } from "@/lib/commit"

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

  it.each([
    "Add connection retry logic",
    "Fix: a capitalised word is not a type",
    "http://example.com is down",
  ])("keeps %j without a prefix", (subject) => {
    expect(splitSubjectPrefix(subject)).toEqual({ prefix: null, text: subject })
  })
})

describe("getCommitMessageBody", () => {
  it("skips the blank lines after the subject", () => {
    expect(getCommitMessageBody("Subject\n\n\nFirst body line\nsecond body line")).toBe("First body line\nsecond body line")
  })

  it("returns an empty body for a one-line message", () => {
    expect(getCommitMessageBody("Subject")).toBe("")
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

describe("getGroupingPrefix", () => {
  it("returns the grouping prefix of the subject without the parentheses", () => {
    expect(getGroupingPrefix("(feature-auth) Add login\n\nBody (not-a-prefix) text")).toBe("feature-auth")
  })

  it("skips the blank lines before the subject", () => {
    expect(getGroupingPrefix("\n(feature-auth) Add login")).toBe("feature-auth")
  })

  it.each(["Add login", "feat(auth): add login", "(feature-auth)", "Add login\n(feature-auth) in the body"])("returns null for %j", (message) => {
    expect(getGroupingPrefix(message)).toBeNull()
  })
})
