import { beforeEach, describe, expect, it, vi } from "vitest"
import type { Branch } from "@/stores/branch-sync"
import { branchSyncStore, getBranchTableRows, getMoveTargets, getStackChildren, getStackDescendants } from "@/stores/branch-sync"
import { testBranch } from "@/stores/test-branch"

vi.mock("@tauri-apps/api/core", () => ({ Channel: Object }))
vi.mock("@/lib/bindings", () => ({ commands: {} }))
vi.mock("@/stores/repository", async () => (await import("@/stores/test-store-mocks")).repositoryMock)
vi.mock("@/stores/settings/repositorySettings", async () => (await import("@/stores/test-store-mocks")).repositorySettingsMock)
vi.mock("@/stores/archived-branches", async () => (await import("@/stores/test-store-mocks")).archivedBranchesMock)

/** `[name, declared parent, issue key]`, newest first */
function setBranches(...specs: [name: string, parent: string | null, issueKey?: string | null][]): void {
  const branches: Record<string, Branch> = {}
  for (const [name, parent, issueKey = null] of specs) {
    branches[name] = testBranch(name, parent, { issueKey, origin: issueKey === name ? "Issue" : "Prefix" })
  }
  branchSyncStore.setState({ branches, branchOrder: specs.map(([name]) => name), stacksEnabled: true })
}

/** `name@depth`, `+` for an issue chip, `#key/count` for an issue row */
function rows(visible: ReadonlySet<string> | null = null): string[] {
  return getBranchTableRows(visible).map(row => row.kind === "issue" ? `#${row.issueKey}/${row.branchCount}` : `${row.name}@${row.depth}${row.showIssueKey ? "+" : ""}`)
}

/**
 * The stack graph: `name:lane`, `<n` the lane a side child forks from, `^` the line up into the node, `v` the line down,
 * `|0,1` the lanes passing the row; `>` before a branch under its issue, `~` after a dimmed one, `name:-` without lanes
 */
function graph(visible: ReadonlySet<string> | null = null): string[] {
  return getBranchTableRows(visible).map((row) => {
    if (row.kind === "issue") {
      return `#${row.issueKey}`
    }
    const lanes = row.lanes
    const place = lanes === null
      ? "-"
      : `${lanes.lane}${lanes.forkFrom === null ? "" : `<${lanes.forkFrom}`}${lanes.up ? "^" : ""}${lanes.down ? "v" : ""}${lanes.through.length > 0 ? `|${lanes.through.join(",")}` : ""}`
    return `${row.familyIndent === 1 ? ">" : ""}${row.name}:${place}${row.dimmed ? "~" : ""}`
  })
}

describe("branch table rows with stacked branches", () => {
  beforeEach(() => {
    branchSyncStore.setState({ branches: {}, branchOrder: [], stacksEnabled: false })
  })

  it("puts each branch under its parent, depth-first, wherever the parent is in the order", () => {
    setBranches(["oauth", "login"], ["docs", null], ["login", "auth"], ["session", "auth"], ["auth", null])
    // the longest chain last: it continues the line of auth
    expect(rows()).toEqual(["docs@0", "auth@0", "session@1", "login@1", "oauth@2"])
    expect(graph()).toEqual(["docs:0", "auth:0v", "session:1<0|0", "login:0^v", "oauth:0^"])
  })

  it("keeps a straight chain on one lane and forks a leaf sibling and a side chain off it", () => {
    setBranches(
      ["review", "workflow"],
      ["workflow", "binds"],
      ["lifecycle", "binds"],
      ["binds", "document"],
      ["air", "gate"],
      ["gate", "document"],
      ["inputs", "document"],
      ["document", "daemon"],
      ["daemon", null],
    )
    expect(graph()).toEqual([
      "daemon:0v",
      "document:0^v",
      "inputs:1<0|0",
      "gate:1<0v|0",
      "air:1^|0",
      "binds:0^v",
      "lifecycle:1<0|0",
      "workflow:0^v",
      "review:0^",
    ])
  })

  it("starts each stack on lane 0, so separate stacks share no line", () => {
    setBranches(["b2", "b"], ["b", null], ["flat", null], ["a2", "a"], ["a", null])
    expect(graph()).toEqual(["b:0v", "b2:0^", "flat:0", "a:0v", "a2:0^"])
  })

  it("has no lanes while no branch is stacked on another", () => {
    setBranches(["parser", null, "ABC-1"], ["lexer", null, "ABC-1"], ["docs", null])
    expect(graph()).toEqual(["#ABC-1", ">parser:-", ">lexer:-", "docs:-"])
  })

  it("keeps a branch whose parent has no row, or whose parents form a cycle, at the top level", () => {
    setBranches(["login", "auth"], ["a", "b"], ["b", "a"])
    expect(rows()).toEqual(["login@0", "a@0", "b@0"])
    expect(graph()).toEqual(["login:-", "a:-", "b:-"])
    // beside a stack, each on lane 0 without a line
    setBranches(["login", "auth"], ["a", "b"], ["b", "a"], ["y", "x"], ["x", null])
    expect(graph()).toEqual(["login:0", "a:0", "b:0", "x:0v", "y:0^"])
  })

  it("forms issue families from the stack roots only and marks a child of another issue", () => {
    setBranches(["parser", null, "ABC-1"], ["lexer", null, "ABC-1"], ["session", "auth", "ABC-1"], ["cookie", "session", "ABC-1"], ["auth", null])
    expect(rows()).toEqual(["#ABC-1/2", "parser@1", "lexer@1", "auth@0", "session@1+", "cookie@2"])
  })

  it("indents a family member and the stack on it one level, with the lanes of the stack inside", () => {
    setBranches(["printer", "parser", "ABC-7"], ["parser", null, "ABC-7"], ["lexer", null, "ABC-7"], ["login", "auth"], ["auth", null])
    expect(rows()).toEqual(["#ABC-7/2", "parser@1", "printer@2", "lexer@1", "auth@0", "login@1"])
    expect(graph()).toEqual(["#ABC-7", ">parser:0v", ">printer:0^", ">lexer:0", "auth:0v", "login:0^"])
  })

  it("shows no chip for a stacked branch named after its issue", () => {
    setBranches(["IJAI-3885", "auth", "IJAI-3885"], ["session", "auth", "ABC-1"], ["auth", null])
    expect(rows()).toEqual(["auth@0", "IJAI-3885@1", "session@1+"])
  })

  it("prunes the rows to the visible branches and the branches they stack on, which are dimmed", () => {
    setBranches(["oauth", "login"], ["docs", null], ["login", "auth"], ["session", "auth"], ["auth", null])
    expect(graph(new Set(["oauth"]))).toEqual(["auth:0v~", "login:0^v~", "oauth:0^"])
    // the lanes are laid out over the rows that remain: login is the only child of auth
    expect(graph(new Set(["oauth", "auth"]))).toEqual(["auth:0v", "login:0^v~", "oauth:0^"])
    expect(graph(new Set(["docs", "session"]))).toEqual(["docs:0", "auth:0v~", "session:0^"])
    // nothing stacked among them: no lanes
    expect(graph(new Set(["docs", "gone"]))).toEqual(["docs:-"])
    expect(graph(new Set())).toEqual([])
  })

  it("prunes an issue family to its visible members", () => {
    setBranches(["parser", null, "ABC-1"], ["lexer", null, "ABC-1"], ["docs", null])
    expect(rows(new Set(["parser", "lexer"]))).toEqual(["#ABC-1/2", "parser@1", "lexer@1"])
    expect(rows(new Set(["parser"]))).toEqual(["parser@0+"])
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
