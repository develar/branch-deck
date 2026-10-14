// @spec spec/commit/commit-row.spec.md
import { describe, expect, it } from "vitest"
import { findIssueKey, getCommitMessageBody, groupFor, hasCommitMessageBody, sanitizeBranchName, splitSubjectPrefix } from "@/lib/commit"

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

describe("groupFor", () => {
  it("returns the explicit prefix of the subject", () => {
    expect(groupFor("(feature-auth) Add login\n\nBody (not-a-prefix) text")).toEqual({ branch: "feature-auth", origin: "prefix" })
  })

  it("skips the blank lines before the subject", () => {
    expect(groupFor("\n(feature-auth) Add login")).toEqual({ branch: "feature-auth", origin: "prefix" })
  })

  it.each([
    ["(mockito 5.19) Update", "mockito-5.19"],
    ["(---test---) Update", "test"],
    ["(feature-auth)", "feature-auth"],
    ["fixup! (feat) Fix", "feat"],
    ["fixup! squash!  (feat) Fix", "feat"],
    ["((nested)) m", "(nested"],
    ["(feature) ABC-123 Fix", "feature"],
  ])("groups %j into the prefix branch %j", (message, branch) => {
    expect(groupFor(message)).toEqual({ branch, origin: "prefix" })
  })

  it.each([
    ["ABC-123 Fix", "ABC-123"],
    ["ABC-123: Fix", "ABC-123"],
    ["[tag] XYZ-999: Fix", "XYZ-999"],
    ["fix: ABC-123 Fix", "ABC-123"],
    ["[cat] feat(api): JKL-012 Add", "JKL-012"],
    ["fixup! ABC-1 Fix", "ABC-1"],
    ["() ABC-1 Fix", null],
  ])("groups %j into the issue branch %j", (message, branch) => {
    expect(groupFor(message)).toEqual(branch === null ? null : { branch, origin: "issue" })
  })

  it.each(["Add login", "feat(auth): add login", "Fix bug (critical)", "Fix JIRA-456", "abc-123 Fix", "ABC123-456 Fix", "ABC-12x Fix", "feat!: ABC-1 Fix", "Add login\n(feature-auth) in the body", "Add login\nABC-1 in the body", "fixup! Regular message", "fixup!(feat) Fix", "()"])("returns null for %j", (message) => {
    expect(groupFor(message)).toBeNull()
  })
})

describe("sanitizeBranchName", () => {
  it.each([
    ["mockito 5.19", "mockito-5.19"],
    ["---test---", "test"],
    ["a:b?c", "a-b-c"],
    [".hidden.", "hidden"],
    ["/feature/sub/", "feature/sub"],
    ["/.a", ".a"],
    ["a  b", "a-b"],
  ])("sanitizes %j to %j", (name, expected) => {
    expect(sanitizeBranchName(name)).toBe(expected)
  })
})

describe("findIssueKey", () => {
  it("is null for a key that is not at the start", () => {
    expect(findIssueKey("Update (ABC-789)")).toBeNull()
    expect(findIssueKey("prefixABC-123")).toBeNull()
  })
})
