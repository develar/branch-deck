import { beforeEach, describe, expect, it, vi } from "vitest"
import type { Branch } from "@/stores/branch-sync"
import { branchSyncStore, getBranchTableRows, getMoveTargets, getStackChildren, getStackDescendants, getStackIslandRoots } from "@/stores/branch-sync"
import { collapseSideStacks, collapseStacks, layoutBranchTables, pruneCollapsedStacks, sideStackSizes, stackRootOf, stackSizes } from "@/lib/stack-collapse"
import { getCollapsedSideStacks } from "@/stores/stack-collapse"
import { branchRowDivider, laneCountOf } from "@/lib/stack-lanes"
import { testBranch, testRemoteStatus } from "@/stores/test-branch"

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
  return getBranchTableRows(visible).map(row => row.kind === "issue" ? `#${row.issueKey}/${row.members.length}` : `${row.name}@${row.depth}${row.showIssueKey ? "+" : ""}`)
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

  it("runs the divider full width above each top-level unit, from the tree inside a stack or an issue family", () => {
    setBranches(["printer", "parser", "ABC-7"], ["parser", null, "ABC-7"], ["lexer", null, "ABC-7"], ["docs", null], ["login", "auth"], ["auth", null])
    const dividers = getBranchTableRows(null).map((row) => {
      const divider = row.kind === "issue" ? undefined : branchRowDivider(row.depth, row.familyIndent)
      return `${row.kind === "issue" ? `#${row.issueKey}` : row.name}${divider === undefined ? "" : `:${divider.kind}${divider.family ? "+" : ""}`}`
    })
    // the line after the end of a stack, above the lone docs, runs full width
    expect(dividers).toEqual(["#ABC-7", "parser:family", "printer:stack+", "lexer:family", "docs", "auth", "login:stack"])
  })

  it("collapses a stack to its root and keeps the lane count of the expanded table", () => {
    setBranches(["docs", null], ["oauth", "login"], ["session", "auth"], ["login", "auth"], ["auth", null])
    const all = getBranchTableRows(null)
    const { rows: shown, hidden } = collapseStacks(all, new Set(["auth"]))
    expect(shown.map(row => row.kind === "branch" ? row.name : row.issueKey)).toEqual(["docs", "auth"])
    expect(hidden).toEqual(new Map([["auth", 3]]))
    // the table keeps the lane column of the expanded stack, so the names don't move
    expect(laneCountOf(all.map(row => row.kind === "branch" ? row.lanes : null))).toBe(2)
    expect(stackSizes(all)).toEqual(new Map([["auth", 4]]))
  })

  it("marks no root collapsed whose stack has no other row, and finds the root from any row of a stack", () => {
    setBranches(["docs", null], ["printer", "parser", "ABC-7"], ["parser", null, "ABC-7"], ["lexer", null, "ABC-7"], ["login", "auth"], ["auth", null])
    const all = getBranchTableRows(null)
    expect(collapseStacks(all, new Set(["docs", "lexer"])).rows).toBe(all)
    // a stack inside an issue family collapses within it
    expect(collapseStacks(all, new Set(["parser"])).hidden).toEqual(new Map([["parser", 1]]))
    expect(stackRootOf(all, "login")).toBe("auth")
    expect(stackRootOf(all, "printer")).toBe("parser")
    expect(stackRootOf(all, "lexer")).toBe("lexer")
  })

  it("keeps only the collapsed roots that still head a stack", () => {
    setBranches(["docs", null], ["login", "auth"], ["auth", null])
    expect(pruneCollapsedStacks(["auth", "docs", "gone", "login", "auth"], getBranchTableRows(null))).toEqual(["auth"])
  })

  it("indents a family member and the stack on it one level, with the lanes of the stack inside", () => {
    setBranches(["printer", "parser", "ABC-7"], ["parser", null, "ABC-7"], ["lexer", null, "ABC-7"], ["login", "auth"], ["auth", null])
    // the issue row heads the stack on a member too
    expect(rows()).toEqual(["#ABC-7/3", "parser@1", "printer@2", "lexer@1", "auth@0", "login@1"])
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

  it("makes a stack of five branches or forty commits an island, not one in an issue family or on an issue branch that heads one", () => {
    // a chain of five on `a`, a chain of four on `w`
    setBranches(["docs", null], ["a5", "a4"], ["a4", "a3"], ["a3", "a2"], ["a2", "a1"], ["a1", null], ["w4", "w3"], ["w3", "w2"], ["w2", "w1"], ["w1", null])
    expect(getStackIslandRoots()).toEqual(new Set(["a1"]))
    // forty commits make the short stack an island too
    branchSyncStore.setState(state => ({ branches: { ...state.branches, w2: { ...state.branches.w2!, commitCount: 37 } } }))
    expect(getStackIslandRoots()).toEqual(new Set(["a1", "w1"]))
    // the stack of an issue branch with its family, and a stack inside a family, stay in the table
    setBranches(["p5", "p4", "ABC-7"], ["p4", "p3", "ABC-7"], ["p3", "p2", "ABC-7"], ["p2", "p1", "ABC-7"], ["p1", null, "ABC-7"], ["lexer", null, "ABC-7"], ["c3", "c2", "XYZ-1"], ["c2", "XYZ-1"], ["c1", "XYZ-1"], ["XYZ-1", null, "XYZ-1"], ["peer", null, "XYZ-1"])
    expect(rows()).toContain("p1@1")
    expect(getStackIslandRoots()).toEqual(new Set())
  })

  it("moves an island out of the table, whole, and lists the shown rows in the order of the screen", () => {
    setBranches(["docs", null], ["a5", "a4"], ["a4", "a3"], ["a3", "a2"], ["a2", "a1"], ["side", "a1"], ["a1", null], ["login", "auth"], ["auth", null])
    const all = getBranchTableRows(null)
    const islandRoots = getStackIslandRoots()
    const layout = layoutBranchTables(all, islandRoots, { stacks: new Set(), sideStacks: new Set() })
    const names = (list: readonly (typeof all)[number][]) => list.map(row => row.kind === "branch" ? row.name : row.issueKey)
    expect(names(layout.tableAll)).toEqual(["docs", "auth", "login"])
    expect(layout.islands.map(island => [island.root, names(island.rows)])).toEqual([["a1", ["a1", "side", "a2", "a3", "a4", "a5"]]])
    expect(layout.order).toEqual(["docs", "auth", "login", "a1", "side", "a2", "a3", "a4", "a5"])
    // a collapsed island shows no row; a collapsed stack of the table shows its root
    const collapsed = layoutBranchTables(all, islandRoots, { stacks: new Set(["a1", "auth"]), sideStacks: new Set() })
    expect(collapsed.islands[0]!.collapsed).toBe(true)
    expect(collapsed.order).toEqual(["docs", "auth"])
    // under a filter (no collapse) the island shows the matches and the branches they stack on
    const filtered = layoutBranchTables(getBranchTableRows(new Set(["a3"])), islandRoots, null)
    expect(filtered.table.rows).toEqual([])
    expect(filtered.order).toEqual(["a1", "a2", "a3"])
    // no island: the rows are the table, as they are
    expect(layoutBranchTables(all, new Set(), null).tableAll).toBe(all)
  })

  it("collapses a side stack into its root, and keeps a quiet one collapsed until the user expands it", () => {
    setBranches(["a4", "a3"], ["a3", "a2"], ["a2", "a1"], ["side2", "side"], ["side", "a1"], ["leaf", "a1"], ["a1", null])
    const all = getBranchTableRows(null)
    // a side child with a branch on it heads a side stack; a side leaf does not
    expect(sideStackSizes(all)).toEqual(new Map([["side", 2]]))
    const { rows: shown, hidden } = collapseSideStacks(all, new Set(["side"]))
    expect(shown.map(row => row.kind === "branch" ? row.name : row.issueKey)).toEqual(["a1", "leaf", "side", "a2", "a3", "a4"])
    expect(hidden).toEqual(new Map([["side", 1]]))
    // with work for the remote it shows; pushed and up to date it collapses
    expect(getCollapsedSideStacks()).toEqual(new Set())
    branchSyncStore.setState(state => ({
      branches: { ...state.branches, side: { ...state.branches.side!, remoteStatus: testRemoteStatus() }, side2: { ...state.branches.side2!, remoteStatus: testRemoteStatus() } },
    }))
    expect(getCollapsedSideStacks()).toEqual(new Set(["side"]))
  })

  it("offers the branches of the same stack first as move targets", () => {
    setBranches(["docs", null], ["oauth", "login"], ["api", null], ["login", "auth"], ["auth", null])
    expect(getMoveTargets("login", 10)).toEqual(["oauth", "auth", "docs", "api"])
  })
})
