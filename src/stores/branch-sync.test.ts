// The page's state of the branches beside the session's table: the pushes in flight, the rows, the commits a queued
// rewrite acts on. The table itself comes from the patches of the session (`src/model/store.test.ts`)
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type { Commit } from "@/lib/bindings"
import type { Branch } from "@/stores/branch-sync"
import {
  branchSyncStore,
  getBranchPushProgress,
  getBranchTableRows,
  isBranchPushing,
  isStackPushing,
  resolveCommits,
  setBranchPushing,
  setBranchPushProgress,
  setStackPush,
  wholeStackRootIn,
} from "@/stores/branch-sync"
import { testBranch } from "@/stores/test-branch"

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
  it("keeps the push progress only while the push runs", () => {
    const writing = { percent: 55, transferred: "1.23 KiB", speed: "1.23 MiB/s", line: "Writing objects:  50% (2/4)" }
    setBranchPushing("a", true, "/repo")
    expect(getBranchPushProgress("a")).toBeNull()
    setBranchPushProgress("a", writing, "/repo")
    expect(getBranchPushProgress("a")).toEqual(writing)
    expect(isBranchPushing("a")).toBe(true)

    setBranchPushing("a", false, "/repo")
    // a channel message that arrives after the push ended
    setBranchPushProgress("a", { ...writing, percent: 95 }, "/repo")
    expect(isBranchPushing("a")).toBe(false)
    expect(getBranchPushProgress("a")).toBeNull()
    expect(branchSyncStore.getState().pushingBranches).toEqual({})
  })

  it("keeps the spinner of a push that ends after a switch of the repository off the branch of the same name there", () => {
    showBranches(testBranch("a", null))
    setBranchPushing("a", true, "/repo")
    // another repository opens, with a branch of the same name; it shows no push of it and pushes its own
    repository.path = "/other"
    try {
      expect(isBranchPushing("a")).toBe(false)
      setBranchPushing("a", true, "/other")
      // the push of /repo ends (its remote status goes to the session of /repo): the push of /other still runs
      setBranchPushing("a", false, "/repo")
      expect(isBranchPushing("a")).toBe(true)
      setBranchPushing("a", false, "/other")
      expect(isBranchPushing("a")).toBe(false)
      expect(branchSyncStore.getState().pushingBranches).toEqual({})
    } finally {
      repository.path = "/repo"
    }
  })

  it("drops the progress of a push that ends after a switch: the next push of the branch starts without its percent", () => {
    const writing = { percent: 45, transferred: "1.23 KiB", speed: "1.23 MiB/s", line: "Writing objects:  45% (2/4)" }
    setBranchPushing("a", true, "/repo")
    setBranchPushProgress("a", writing, "/repo")
    repository.path = "/other"
    try {
      // the rows of /other show no progress of it, and its push of the branch of the same name starts at none
      expect(getBranchPushProgress("a")).toBeNull()
      setBranchPushing("a", false, "/repo")
      setBranchPushing("a", true, "/other")
      expect(getBranchPushProgress("a")).toBeNull()
      setBranchPushing("a", false, "/other")
    } finally {
      repository.path = "/repo"
    }
    // back in /repo, a new push of it starts without the old percent
    setBranchPushing("a", true, "/repo")
    expect(getBranchPushProgress("a")).toBeNull()
    setBranchPushing("a", false, "/repo")
  })

  it("keeps a stack push that runs on after a switch off the row of the same name there", () => {
    setStackPush("core", ["auth", "core"], "/repo")
    repository.path = "/other"
    try {
      // the row of core in /other is idle: its own stack push goes
      expect(isStackPushing("core")).toBe(false)
      setStackPush("core", ["core"], "/other")
      setStackPush("core", null, "/repo")
      expect(isStackPushing("core")).toBe(true)
      setStackPush("core", null, "/other")
      expect(branchSyncStore.getState().stackPushes).toEqual({})
    } finally {
      repository.path = "/repo"
    }
  })
})

describe("the whole stack a Safe Push of a branch can land instead", () => {
  afterEach(() => {
    branchSyncStore.setState({ branches: {}, branchOrder: [], stacksEnabled: false, stackBranches: {} })
  })

  it("is the stack that forks and holds the branch, else the straight chain the branch heads", () => {
    const branches = [testBranch("auth", null), testBranch("login", "auth"), testBranch("oauth", "login"), testBranch("session", "auth"), testBranch("docs", null)]
    branchSyncStore.setState({ branches: Object.fromEntries(branches.map(branch => [branch.name, branch])), branchOrder: branches.map(branch => branch.name), stacksEnabled: true })
    // without its stack branch auth heads a chain of its own; login heads none, docs no stack
    expect(["auth", "login", "oauth", "docs"].map(name => wholeStackRootIn(branchSyncStore.getState(), name))).toEqual(["auth", null, null, null])
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
    branchSyncStore.setState({ stackBranches: { auth: stack } })
    expect(["auth", "login", "oauth", "session", "docs"].map(name => wholeStackRootIn(branchSyncStore.getState(), name))).toEqual(["auth", "auth", "auth", "auth", null])
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
