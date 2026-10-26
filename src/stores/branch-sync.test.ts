// The page's state of the branches beside the session's table: the pushes in flight, the rows, the commits a queued
// rewrite acts on. The table itself comes from the patches of the session (`src/model/store.test.ts`)
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type { Commit, RunningOperation } from "@/lib/bindings"
import type { Branch } from "@/stores/branch-sync"
import { branchSyncStore, getBranchTableRows, isBranchPushing, isStackPushing, resolveCommits, wholeStackRootIn } from "@/stores/branch-sync"
import { placed, testBranch } from "@/stores/test-branch"

const { repository } = vi.hoisted(() => ({
  /** The repository the window shows */
  repository: { path: "/repo" },
}))

vi.mock("@tauri-apps/api/core", () => import("@/test-support/channel-mock"))
vi.mock("@/lib/bindings", () => ({ commands: {} }))
vi.mock("@/stores/repository", () => ({
  getSelectedProject: () => ({ path: repository.path }),
  getSelectedRepositoryPath: () => repository.path,
}))

/** The store shows `branches`, in that order */
function showBranches(...branches: Branch[]): void {
  branchSyncStore.setState({ branches: Object.fromEntries(branches.map(branch => [branch.name, branch])), branchOrder: branches.map(branch => branch.name) })
}

beforeEach(() => {
  branchSyncStore.setState(branchSyncStore.getInitialState(), true)
  repository.path = "/repo"
})

describe("a push in flight", () => {
  /** The operation of the session of a push of the stack of `subject` of the branches `refs` (their virtual branches) */
  function stackPush(subject: string, refs: readonly string[]): RunningOperation {
    return {
      id: "op",
      intent: { kind: "push", subject: { kind: "branch", name: subject }, scope: { kind: "withAncestors" }, expectRemote: null, force: false, restartRunning: false },
      resources: refs.map(name => ({ kind: "ref" as const, name: `user/virtual/${name}` })),
      step: { kind: "pushing" },
      origin: { kind: "thisProcess" },
      moves: [],
    }
  }

  it("is an operation of the session: the stack push of a row, the branches it sends, none after a switch of the repository", () => {
    branchSyncStore.setState({ operations: { op: stackPush("core", ["auth", "core"]) } })
    expect(isStackPushing("core")).toBe(true)
    expect(isStackPushing("auth")).toBe(false)
    expect([isBranchPushing("auth"), isBranchPushing("core"), isBranchPushing("ui")]).toEqual([true, true, false])
    // the operations are of the shown repository: another one's branch of the same name is another branch
    expect(isBranchPushing("auth", "/other")).toBe(false)
    branchSyncStore.setState({ operations: {} })
    expect(isStackPushing("core")).toBe(false)
  })
})

describe("the whole stack a Safe Push of a branch can land instead", () => {
  afterEach(() => {
    branchSyncStore.setState({ branches: {}, branchOrder: [], stacksEnabled: false, stackBranches: {} })
  })

  it("is the stack that forks and holds the branch, else the straight chain the branch heads", () => {
    const branches = [testBranch("auth", null), testBranch("login", "auth"), testBranch("oauth", "login"), testBranch("session", "auth"), testBranch("docs", null)]
    const branchOrder = branches.map(branch => branch.name)
    branchSyncStore.setState({ ...placed({ branches: Object.fromEntries(branches.map(branch => [branch.name, branch])), branchOrder }), branchOrder, stacksEnabled: true })
    // the stack forks: without its stack branch it heads no whole stack yet, the sync builds it; docs heads no stack
    expect(["auth", "login", "oauth", "docs"].map(name => wholeStackRootIn(branchSyncStore.getState(), name))).toEqual([null, null, null, null])
    const stack = {
      root: "auth",
      branchName: "user/stack/auth",
      members: ["auth", "login", "oauth", "session"],
      commitCount: 4,
      state: "Built",
      tip: "t",
      blockedBy: null,
      conflict: null,
      error: null,
      remoteStatus: null,
    } as const
    const table = Object.fromEntries(branches.map(branch => [branch.name, branch]))
    branchSyncStore.setState({ ...placed({ branches: table, branchOrder, stackBranches: { auth: stack } }), stackBranches: { auth: stack } })
    expect(["auth", "login", "oauth", "session", "docs"].map(name => wholeStackRootIn(branchSyncStore.getState(), name))).toEqual(["auth", "auth", "auth", "auth", null])
    // a straight chain: its root heads it, its other branches none
    const chain = [testBranch("auth", null), testBranch("login", "auth"), testBranch("oauth", "login")]
    const chainOrder = chain.map(branch => branch.name)
    branchSyncStore.setState({
      ...placed({ branches: Object.fromEntries(chain.map(branch => [branch.name, branch])), branchOrder: chainOrder }),
      branchOrder: chainOrder,
      stackBranches: {},
    })
    expect(["auth", "login", "oauth"].map(name => wholeStackRootIn(branchSyncStore.getState(), name))).toEqual(["auth", null, null])
  })
})

describe("branch table rows", () => {
  /** The rows of `branches` (name, origin, issue key), in that order: b a branch, n nested under its issue, c with an issue chip, i an issue row */
  function rowsOf(...branches: [name: string, origin: "Prefix" | "Issue", issueKey: string | null][]): string[] {
    showBranches(...branches.map(([name, origin, issueKey]) => testBranch(name, null, { origin, issueKey, stack: null })))
    return getBranchTableRows().map(row => (row.kind === "issue" ? `i:${row.members.length}:${row.issueKey}` : `${row.showIssueKey ? "c" : row.depth > 0 ? "n" : "b"}:${row.name}`))
  }

  it("nests the branches of an issue under the issue branch, wherever it is in the order", () => {
    const rows = rowsOf(["lexer", "Prefix", "ABC-1"], ["other", "Prefix", null], ["ABC-1", "Issue", "ABC-1"], ["parser", "Prefix", "ABC-1"])
    expect(rows).toEqual(["b:other", "b:ABC-1", "n:lexer", "n:parser"])
  })

  it("heads the branches of an issue without a branch of its own with an issue row", () => {
    const rows = rowsOf(["other", "Prefix", null], ["lexer", "Prefix", "ABC-1"], ["parser", "Prefix", "ABC-1"])
    expect(rows).toEqual(["b:other", "i:2:ABC-1", "n:lexer", "n:parser"])
  })

  it("shows a single branch of an issue without a branch of its own with a chip", () => {
    const rows = rowsOf(["lexer", "Prefix", "ABC-1"], ["XYZ-2", "Issue", "XYZ-2"])
    expect(rows).toEqual(["c:lexer", "b:XYZ-2"])
  })

  it("keeps the table rows while only the branches change, not their structure", () => {
    showBranches(testBranch("a", null, { stack: null }), testBranch("b", null, { stack: null }))
    const rows = getBranchTableRows()
    branchSyncStore.setState(state => ({ branches: { ...state.branches, a: { ...state.branches.a!, status: "Syncing" } } }))
    expect(getBranchTableRows()).toBe(rows)
  })
})

describe("the commits a rewrite that waited in the queue acts on", () => {
  afterEach(() => {
    branchSyncStore.setState(branchSyncStore.getInitialState(), true)
  })

  function commitOf(originalHash: string, strippedSubject: string, authorTime: number, committerTime: number): Commit {
    return { originalHash, strippedSubject, message: strippedSubject, author: "Ann", authorTime, committerTime }
  }

  function showCommits(commits: Commit[]): void {
    showBranches(testBranch("auth", null, { commits }))
  }

  it("finds a commit whose subject a message rewrite changed by the committer it kept, and one an amend replayed by its subject", () => {
    const tip = commitOf("a1", "Add login", 100, 200)
    const base = commitOf("b1", "Add form", 90, 190)
    // Add Issue Reference gave the tip another id and subject and kept its committer; an amend of the base before it
    // replayed it with a new committer time and the same subject
    showCommits([commitOf("a2", "GH-1 Add login", 100, 200), commitOf("b2", "Add form", 90, 300)])
    expect(resolveCommits([tip, base])?.map(commit => commit.originalHash)).toEqual(["a2", "b2"])
  })

  it("finds no commit by a key two commits share", () => {
    showCommits([commitOf("a2", "GH-1 Add login", 100, 200), commitOf("c2", "GH-1 Add logout", 100, 200)])
    expect(resolveCommits([commitOf("a1", "Add login", 100, 200)])).toBeNull()
  })
})
