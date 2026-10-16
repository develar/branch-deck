// @spec spec/commit/commit-row.spec.md
import { describe, expect, it } from "vite-plus/test"
import { findIssueKey, getCommitMessageBody, groupFor, hasCommitMessageBody, sanitizeBranchName, splitSubjectPrefix, stripLeadingIssueKey } from "@/lib/commit"

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

  it.each([
    "Add login",
    "feat(auth): add login",
    "Fix bug (critical)",
    "Fix JIRA-456",
    "abc-123 Fix",
    "ABC123-456 Fix",
    "ABC-12x Fix",
    "feat!: ABC-1 Fix",
    "Add login\n(feature-auth) in the body",
    "Add login\nABC-1 in the body",
    "fixup! Regular message",
    "fixup!(feat) Fix",
    "()",
  ])("returns null for %j", message => {
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

describe("stripLeadingIssueKey", () => {
  it.each([
    ["IJPL-1 Fix the cache", "Fix the cache"],
    ["IJPL-1: Fix the cache", "Fix the cache"],
    ["IJPL-1 - Fix the cache", "Fix the cache"],
    ["IJPL-1 – Fix the cache", "Fix the cache"],
    ["[IJPL-1] Fix the cache", "Fix the cache"],
    ["IJPL-1, IJPL-2: Fix the cache", "IJPL-2: Fix the cache"],
    ["IJPL-1", ""],
  ])("drops the key the row shows from %j", (text, rest) => {
    expect(stripLeadingIssueKey(text, "IJPL-1")).toBe(rest)
  })

  it("keeps a key in the middle, another key, a longer key and a key that is a part of the sentence", () => {
    expect(stripLeadingIssueKey("Fix IJPL-1 in the cache", "IJPL-1")).toBe("Fix IJPL-1 in the cache")
    expect(stripLeadingIssueKey("IJPL-2 Fix the cache", "IJPL-1")).toBe("IJPL-2 Fix the cache")
    expect(stripLeadingIssueKey("IJPL-12 Fix the cache", "IJPL-1")).toBe("IJPL-12 Fix the cache")
    expect(stripLeadingIssueKey("TEST-1 and PROD-9: Database optimization", "TEST-1")).toBe("TEST-1 and PROD-9: Database optimization")
  })
})

describe("groupFor with stacked branches", () => {
  it.each([
    ["(auth/login) Add form", { branch: "login", origin: "prefix", parent: "auth", parentSource: "prefix" }],
    ["(auth/login/oauth) Add OAuth", { branch: "oauth", origin: "prefix", parent: "login", parentSource: "prefix" }],
    ["(login) Add form", { branch: "login", origin: "prefix", parent: null }],
    ["( my auth / log in ) Add form", { branch: "log-in", origin: "prefix", parent: "my-auth", parentSource: "prefix" }],
    ["(auth/) Add auth", { branch: "auth", origin: "prefix", parent: null }],
    ["(/login) Add form", { branch: "login", origin: "prefix", parent: null }],
    ["fixup! (auth/login) Add form", { branch: "login", origin: "prefix", parent: "auth", parentSource: "prefix" }],
    ["(a//b) Odd", { branch: "b", origin: "prefix", parent: null, invalid: true }],
    ["ABC-1: Fix", { branch: "ABC-1", origin: "issue", parent: null }],
  ])("reads %j as the backend stack_path does", (message, group) => {
    expect(groupFor(message, true)).toEqual(group)
  })

  it("keeps a slash in a flat name without stacked branches", () => {
    expect(groupFor("(auth/login) Add form")).toEqual({ branch: "auth/login", origin: "prefix" })
  })

  it("groups a prefix without a branch name nowhere", () => {
    expect(groupFor("(/) Nothing", true)).toBeNull()
  })
})

describe("groupFor with stored parents", () => {
  const issueGroups = { "ABC-2": "parser" }
  const stackParents = { login: "auth", "ABC-1": "login", parser: "auth" }
  const group = (message: string, stacks = true) => groupFor(message, stacks, issueGroups, stackParents)

  it("takes the stack parent of a branch whose prefix names no parent", () => {
    expect(group("(login) Add form")).toEqual({ branch: "login", origin: "prefix", parent: "auth", parentSource: "store" })
    expect(group("ABC-1 Fix login")).toEqual({ branch: "ABC-1", origin: "issue", parent: "login", parentSource: "store" })
    expect(group("ABC-2 Parse")).toEqual({ branch: "parser", origin: "issueGroup", parent: "auth", parentSource: "store" })
  })

  it("lets a parent level of the prefix declare the parent", () => {
    expect(group("(docs/login) Add form")).toEqual({ branch: "login", origin: "prefix", parent: "docs", parentSource: "prefix" })
  })

  it("leaves a branch without a line a root, and ignores the map without stacked branches", () => {
    expect(group("(docs) Write")).toEqual({ branch: "docs", origin: "prefix", parent: null })
    expect(group("(login) Add form", false)).toEqual({ branch: "login", origin: "prefix" })
  })
})

describe("groupFor with issue groups", () => {
  const issueGroups = { "ABC-1": "parser", "ABC-2": "parser" }

  it.each([
    ["ABC-1 Add grammar", { branch: "parser", origin: "issueGroup" }],
    ["fix(api): ABC-2 Fix grammar", { branch: "parser", origin: "issueGroup" }],
    ["fixup! ABC-1 Add grammar", { branch: "parser", origin: "issueGroup" }],
    ["XYZ-3 Other issue", { branch: "XYZ-3", origin: "issue" }],
    ["(lexer) ABC-1 Add tokens", { branch: "lexer", origin: "prefix" }],
  ])("groups %j as the backend does: prefix, then issue group, then issue key", (message, group) => {
    expect(groupFor(message, false, issueGroups)).toEqual(group)
  })

  it("puts the issue group of a key in the stack forest as a root", () => {
    expect(groupFor("ABC-1 Add grammar", true, issueGroups)).toEqual({ branch: "parser", origin: "issueGroup", parent: null })
  })
})
