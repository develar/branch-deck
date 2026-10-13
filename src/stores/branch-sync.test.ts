import { beforeEach, describe, expect, it, vi } from "vitest"
import type { GroupedBranchInfo, RemoteStatusUpdate, SyncEvent } from "@/lib/bindings"
import { branchSyncStore, getBranch, isBranchPushing, setBranchPushing, setBranchRemoteStatus, syncBranches, toRemoteStatus } from "@/stores/branch-sync"

const { syncEvents } = vi.hoisted(() => ({ syncEvents: { current: [] as SyncEvent[] } }))

vi.mock("@tauri-apps/api/core", () => ({
  Channel: class {
    onmessage: (event: unknown) => void = () => {}
  },
}))

vi.mock("@/lib/bindings", () => ({
  commands: {
    syncBranches: async (_request: unknown, channel: { onmessage: (event: SyncEvent) => void }) => {
      for (const event of syncEvents.current) {
        channel.onmessage(event)
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
}))

function group(name: string): GroupedBranchInfo {
  return {
    name,
    commits: [{ originalHash: `${name}1`, strippedSubject: "subject", message: "subject", author: "a", authorTime: 1, committerTime: 1 }],
    latestCommitTime: 1,
    summary: "subject",
    allCommitsHaveIssueReferences: false,
    myEmail: null,
  }
}

const branchesGrouped: SyncEvent = { type: "branchesGrouped", data: { branches: [group("a"), group("b")], baselineBranch: "origin/master" } }

function remoteStatusUpdate(branchName: string, unpushedCount: number): RemoteStatusUpdate {
  return {
    branchName,
    remoteExists: true,
    unpushedCommits: Array.from({ length: unpushedCount }, (_, index) => `${branchName}${index}`),
    commitsBehind: 0,
    myUnpushedCount: unpushedCount,
    lastPushTime: 0,
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
