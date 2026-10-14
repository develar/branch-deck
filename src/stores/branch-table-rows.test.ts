import { beforeEach, describe, expect, it, vi } from "vitest"
import type { BranchStack } from "@/lib/bindings"
import type { Branch } from "@/stores/branch-sync"
import { branchSyncStore, getBranchTableRows, getMoveTargets, getStackChildren, getStackDescendants } from "@/stores/branch-sync"

vi.mock("@tauri-apps/api/core", () => ({ Channel: Object }))
vi.mock("@/lib/bindings", () => ({ commands: {} }))
vi.mock("@/stores/repository", async () => (await import("@/stores/test-store-mocks")).repositoryMock)
vi.mock("@/stores/settings/repositorySettings", async () => (await import("@/stores/test-store-mocks")).repositorySettingsMock)
vi.mock("@/stores/archived-branches", async () => (await import("@/stores/test-store-mocks")).archivedBranchesMock)

function stack(declaredParent: string | null): BranchStack {
  return {
    declaredParent,
    parentState: declaredParent === null ? null : "Active",
    effectiveBase: declaredParent,
    blockedBy: null,
    problems: [],
    prBaseChanged: false,
    childPrecedesParent: false,
  }
}

/** `[name, declared parent, issue key]`, newest first */
function setBranches(...specs: [name: string, parent: string | null, issueKey?: string | null][]): void {
  const branches: Record<string, Branch> = {}
  for (const [name, parent, issueKey = null] of specs) {
    branches[name] = {
      name,
      commits: [],
      commitIndex: {},
      commitCount: 1,
      status: "Unchanged",
      statusText: "",
      processedCount: 0,
      hasError: false,
      autoExpandRequested: false,
      autoScrollRequested: false,
      latestCommitTime: 0,
      summary: "",
      remoteStatus: null,
      allCommitsHaveIssueReferences: false,
      myEmail: null,
      origin: issueKey === name ? "Issue" : "Prefix",
      issueKey,
      stack: stack(parent),
    }
  }
  branchSyncStore.setState({ branches, branchOrder: specs.map(([name]) => name), stacksEnabled: true })
}

/** `name@depth`, `+` for an issue chip, `#key/count` for an issue row */
function rows(): string[] {
  return getBranchTableRows().map(row => row.kind === "issue" ? `#${row.issueKey}/${row.branchCount}` : `${row.name}@${row.depth}${row.showIssueKey ? "+" : ""}`)
}

describe("branch table rows with stacked branches", () => {
  beforeEach(() => {
    branchSyncStore.setState({ branches: {}, branchOrder: [], stacksEnabled: false })
  })

  it("puts each branch under its parent, depth-first, wherever the parent is in the order", () => {
    setBranches(["oauth", "login"], ["docs", null], ["login", "auth"], ["session", "auth"], ["auth", null])
    expect(rows()).toEqual(["docs@0", "auth@0", "login@1", "oauth@2", "session@1"])
  })

  it("keeps a branch whose parent has no row, or whose parents form a cycle, at the top level", () => {
    setBranches(["login", "auth"], ["a", "b"], ["b", "a"])
    expect(rows()).toEqual(["login@0", "a@0", "b@0"])
  })

  it("forms issue families from the stack roots only and marks a child of another issue", () => {
    setBranches(["parser", null, "ABC-1"], ["lexer", null, "ABC-1"], ["session", "auth", "ABC-1"], ["cookie", "session", "ABC-1"], ["auth", null])
    expect(rows()).toEqual(["#ABC-1/2", "parser@1", "lexer@1", "auth@0", "session@1+", "cookie@2"])
  })

  it("lists the children and the descendants of a branch", () => {
    setBranches(["oauth", "login"], ["login", "auth"], ["session", "auth"], ["auth", null])
    expect(getStackChildren("auth")).toEqual(["login", "session"])
    expect(getStackDescendants("auth").sort()).toEqual(["login", "oauth", "session"])
  })

  it("offers the branches of the same stack first as move targets", () => {
    setBranches(["docs", null], ["oauth", "login"], ["api", null], ["login", "auth"], ["auth", null])
    expect(getMoveTargets("login", 10)).toEqual(["oauth", "auth", "docs", "api"])
  })
})
