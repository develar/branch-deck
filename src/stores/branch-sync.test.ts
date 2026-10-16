import { beforeEach, describe, expect, it, vi } from "vitest"
import type { GroupedBranchInfo, RemoteStatusUpdate, SyncEvent } from "@/lib/bindings"
import { branchSyncStore, getBranch, getBranchPushProgress, getBranchTableRows, isBranchPushing, setBranchPushing, setBranchPushProgress, setBranchRemoteStatus, syncBranches, toRemoteStatus } from "@/stores/branch-sync"

// a function runs between the events, while the sync still streams them
const { syncEvents } = vi.hoisted(() => ({ syncEvents: { current: [] as (SyncEvent | (() => void))[] } }))

vi.mock("@tauri-apps/api/core", () => ({
  Channel: class {
    onmessage: (event: unknown) => void = () => {}
  },
}))

vi.mock("@/lib/bindings", () => ({
  commands: {
    syncBranches: async (_request: unknown, channel: { onmessage: (event: SyncEvent) => void }) => {
      for (const event of syncEvents.current) {
        if (typeof event === "function") {
          event()
        }
        else {
          channel.onmessage(event)
        }
      }
      return { status: "ok", data: null }
    },
  },
}))

vi.mock("@/stores/repository", () => ({
  getLoadingPromise: () => null,
  getSelectedProject: () => null,
  vcsRequestFactory: { createRequest: () => ({}) },
}))

vi.mock("@/stores/settings/repositorySettings", () => ({ updateProject: () => {} }))

vi.mock("@/stores/archived-branches", () => ({
  updateFromArchivedNames: () => {},
  updateFromIntegrationInfo: () => {},
  updateFromIntegrationInfos: () => {},
  settleArchivedPlaceholders: () => {},
}))

function group(name: string): GroupedBranchInfo {
  return {
    name,
    commits: [{ originalHash: `${name}1`, strippedSubject: "subject", message: "subject", author: "a", authorTime: 1, committerTime: 1 }],
    latestCommitTime: 1,
    summary: "subject",
    allCommitsHaveIssueReferences: false,
    myEmail: null,
    origin: "Prefix",
    issueKey: null,
  }
}

const branchesGrouped: SyncEvent = { type: "branchesGrouped", data: { branches: [group("a"), group("b")], baselineBranch: "origin/master" } }

function remoteStatusUpdate(branchName: string, unpushedCount: number): RemoteStatusUpdate {
  return {
    branchName,
    remoteExists: true,
    unpushedCommits: Array.from({ length: unpushedCount }, (_, index) => `${branchName}${index}`),
    commitsBehind: 0,
    commitsAmended: 0,
    commitsRestacked: 0,
    myUnpushedCount: unpushedCount,
    fastForward: true,
    lastPushTime: 0,
    droppedCommits: [],
    remoteTip: `${branchName}-remote`,
  }
}

describe("sync during a push", () => {
  beforeEach(() => {
    branchSyncStore.setState(branchSyncStore.getInitialState(), true)
    syncEvents.current = [branchesGrouped]
  })

  it("keeps the push state of a branch", async () => {
    await syncBranches()
    setBranchPushing("a", true)
    // a regroup resets every branch; mark one to prove the second sync rebuilt the records
    branchSyncStore.setState(state => ({ branches: { ...state.branches, a: { ...state.branches.a!, status: "Unchanged" } } }))

    await syncBranches()

    expect(getBranch("a")!.status).toBe("Syncing")
    expect(isBranchPushing("a")).toBe(true)
    expect(isBranchPushing("b")).toBe(false)
  })

  it("keeps the push progress only while the push runs", () => {
    const writing = { percent: 55, transferred: "1.23 KiB", speed: "1.23 MiB/s", line: "Writing objects:  50% (2/4)" }
    setBranchPushing("a", true)
    expect(getBranchPushProgress("a")).toBeNull()
    setBranchPushProgress("a", writing)
    expect(getBranchPushProgress("a")).toEqual(writing)
    expect(isBranchPushing("a")).toBe(true)

    setBranchPushing("a", false)
    // a channel message that arrives after the push ended
    setBranchPushProgress("a", { ...writing, percent: 95 })
    expect(isBranchPushing("a")).toBe(false)
    expect(getBranchPushProgress("a")).toBeNull()
    expect(branchSyncStore.getState().pushingBranches).toEqual({})
  })

  it("ignores the remote status computed by the sync while the branch is pushing", async () => {
    await syncBranches()
    const pushed = toRemoteStatus(remoteStatusUpdate("a", 0))
    setBranchRemoteStatus("a", pushed)
    setBranchPushing("a", true)
    syncEvents.current = [
      branchesGrouped,
      { type: "remoteStatusUpdate", data: remoteStatusUpdate("a", 2) },
      { type: "remoteStatusUpdate", data: remoteStatusUpdate("b", 3) },
    ]

    await syncBranches()
    expect(getBranch("a")!.remoteStatus).toEqual(pushed)
    expect(getBranch("b")!.remoteStatus!.commitsAhead).toBe(3)

    setBranchPushing("a", false)
    await syncBranches()
    expect(getBranch("a")!.remoteStatus!.commitsAhead).toBe(2)
  })

  it("keeps the push result when the push ends before the sync applies the status it computed mid-push", async () => {
    await syncBranches()
    setBranchPushing("a", true)
    const pushed = toRemoteStatus(remoteStatusUpdate("a", 0))
    syncEvents.current = [
      branchesGrouped,
      { type: "remoteStatusUpdate", data: remoteStatusUpdate("a", 2) },
      () => {
        setBranchRemoteStatus("a", pushed)
        setBranchPushing("a", false)
      },
    ]

    await syncBranches()
    expect(getBranch("a")!.remoteStatus).toEqual(pushed)
  })

  it("keeps the last known remote status across a regroup", async () => {
    syncEvents.current = [branchesGrouped, { type: "remoteStatusUpdate", data: remoteStatusUpdate("a", 1) }]
    await syncBranches()
    const known = getBranch("a")!.remoteStatus

    syncEvents.current = [branchesGrouped]
    await syncBranches()

    expect(known).not.toBeNull()
    expect(getBranch("a")!.remoteStatus).toBe(known)
    expect(getBranch("b")!.remoteStatus).toBeNull()
  })
})

describe("sync events", () => {
  beforeEach(() => {
    branchSyncStore.setState(branchSyncStore.getInitialState(), true)
  })

  it("applies the events of a sync in one store update", async () => {
    syncEvents.current = [
      branchesGrouped,
      { type: "commitSynced", data: { branchName: "a", commitHash: "a1", newHash: "a1-new", status: "Created" } },
      { type: "commitSynced", data: { branchName: "b", commitHash: "b1", newHash: "b1-new", status: "Created" } },
      { type: "remoteStatusUpdate", data: remoteStatusUpdate("a", 1) },
    ]
    let branchUpdates = 0
    const unsubscribe = branchSyncStore.subscribe((state, previous) => {
      if (state.branches !== previous.branches) {
        branchUpdates++
      }
    })
    await syncBranches()
    unsubscribe()

    expect(branchUpdates).toBe(1)
    expect(getBranch("a")!.commits[0]!.hash).toBe("a1-new")
    expect(getBranch("b")!.processedCount).toBe(1)
    expect(getBranch("a")!.remoteStatus!.commitsAhead).toBe(1)
  })

  it("keeps the record of a branch whose remote status a sync reports unchanged", async () => {
    syncEvents.current = [branchesGrouped, { type: "remoteStatusUpdate", data: remoteStatusUpdate("a", 1) }]
    await syncBranches()
    const branch = getBranch("a")
    setBranchRemoteStatus("a", toRemoteStatus(remoteStatusUpdate("a", 1)))
    expect(getBranch("a")).toBe(branch)
  })

  it("keeps the table rows while only the branches change, not their structure", async () => {
    syncEvents.current = [branchesGrouped]
    await syncBranches()
    const rows = getBranchTableRows()
    branchSyncStore.setState(state => ({ branches: { ...state.branches, a: { ...state.branches.a!, status: "Unchanged" } } }))
    expect(getBranchTableRows()).toBe(rows)
  })
})

describe("branch table rows", () => {
  function grouped(...branches: [name: string, origin: "Prefix" | "Issue", issueKey: string | null][]): SyncEvent {
    return {
      type: "branchesGrouped",
      data: {
        branches: branches.map(([name, origin, issueKey]) => ({ ...group(name), origin, issueKey })),
        baselineBranch: "origin/master",
      },
    }
  }

  async function rowsOf(event: SyncEvent): Promise<string[]> {
    syncEvents.current = [event]
    await syncBranches()
    // b: a branch, n: nested under its issue, c: with an issue chip, i: an issue row
    return getBranchTableRows().map(row => row.kind === "issue" ? `i:${row.branchCount}:${row.issueKey}` : `${row.showIssueKey ? "c" : row.depth > 0 ? "n" : "b"}:${row.name}`)
  }

  it("nests the branches of an issue under the issue branch, wherever it is in the order", async () => {
    const rows = await rowsOf(grouped(["lexer", "Prefix", "ABC-1"], ["other", "Prefix", null], ["ABC-1", "Issue", "ABC-1"], ["parser", "Prefix", "ABC-1"]))
    expect(rows).toEqual(["b:other", "b:ABC-1", "n:lexer", "n:parser"])
  })

  it("heads the branches of an issue without a branch of its own with an issue row", async () => {
    const rows = await rowsOf(grouped(["other", "Prefix", null], ["lexer", "Prefix", "ABC-1"], ["parser", "Prefix", "ABC-1"]))
    expect(rows).toEqual(["b:other", "i:2:ABC-1", "n:lexer", "n:parser"])
  })

  it("shows a single branch of an issue without a branch of its own with a chip", async () => {
    const rows = await rowsOf(grouped(["lexer", "Prefix", "ABC-1"], ["XYZ-2", "Issue", "XYZ-2"]))
    expect(rows).toEqual(["c:lexer", "b:XYZ-2"])
  })
})
