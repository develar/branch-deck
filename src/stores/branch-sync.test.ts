import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type { GroupedBranchInfo, RemoteStatusUpdate, SyncEvent } from "@/lib/bindings"
import {
  branchSyncStore,
  getBranch,
  getBranchPushProgress,
  getBranchTableRows,
  isBranchPushing,
  resetBranchSync,
  setBranchPushing,
  setBranchPushProgress,
  setBranchRemoteStatus,
  syncBranches,
  syncIfChanged,
  toRemoteStatus,
  whenSynced,
} from "@/stores/branch-sync"

// a function runs between the events, while the sync still streams them
const { syncEvents, syncResult, toasts, inputs, held, repository, projectUpdates, settledPlaceholders } = vi.hoisted(() => ({
  /** What the sync inputs fingerprint answers, and how often it was asked; `onRead` runs during a read */
  inputs: { fingerprint: "f1", reads: 0, onRead: null as (() => void) | null },
  /** While enabled, each sync waits after its events until a test finishes it: the syncs a test lets overlap */
  held: { enabled: false, calls: [] as { request: { repositoryPath: string }; channel: { onmessage: (event: SyncEvent) => void }; finish: (result?: unknown) => void }[] },
  /** The repository a sync request names: the one selected */
  repository: { path: "/repo" },
  projectUpdates: [] as { path: string; update: unknown }[],
  settledPlaceholders: { count: 0 },
  syncEvents: { current: [] as (SyncEvent | (() => void))[] },
  /** What the sync answers after its events: ok unless a test says otherwise */
  syncResult: { current: { status: "ok", data: null } as unknown },
  toasts: [] as { type?: string; title: string; description?: string }[],
}))

vi.mock("@/lib/toast-message", () => ({ showToast: (toast: { type?: string; title: string; description?: string }) => toasts.push(toast) }))

vi.mock("@tauri-apps/api/core", () => ({
  Channel: class {
    onmessage: (event: unknown) => void = () => {}
  },
}))

vi.mock("@/lib/bindings", () => ({
  commands: {
    syncBranches: async (request: { repositoryPath: string }, channel: { onmessage: (event: SyncEvent) => void }) => {
      for (const event of syncEvents.current) {
        if (typeof event === "function") {
          event()
        } else {
          channel.onmessage(event)
        }
      }
      if (held.enabled) {
        return new Promise(resolve => {
          held.calls.push({ request, channel, finish: (result = { status: "ok", data: null }) => resolve(result) })
        })
      }
      return syncResult.current
    },
    getSyncInputsFingerprint: async () => {
      inputs.reads++
      const fingerprint = inputs.fingerprint
      inputs.onRead?.()
      return { status: "ok", data: fingerprint }
    },
  },
}))

vi.mock("@/stores/repository", () => ({
  getLoadingPromise: () => null,
  getSelectedProject: () => null,
  vcsRequestFactory: { createRequest: () => ({ repositoryPath: repository.path, branchPrefix: "user" }) },
}))

vi.mock("@/stores/settings/repositorySettings", () => ({ updateProject: (path: string, update: unknown) => projectUpdates.push({ path, update }) }))

vi.mock("@/stores/archived-branches", () => ({
  updateFromArchivedNames: () => {},
  updateFromIntegrationInfo: () => {},
  updateFromIntegrationInfos: () => {},
  settleArchivedPlaceholders: () => {
    settledPlaceholders.count++
  },
  resetArchivedBranches: () => {},
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

describe("sync on focus", () => {
  beforeEach(() => {
    branchSyncStore.setState(branchSyncStore.getInitialState(), true)
    syncEvents.current = [branchesGrouped]
    syncResult.current = { status: "ok", data: null }
    inputs.fingerprint = "f1"
  })

  it("syncs only when what a sync reads changed since the last sync, one read otherwise", async () => {
    // never synced: a sync
    await syncIfChanged()
    expect(branchSyncStore.getState().syncCount).toBe(1)
    expect(branchSyncStore.getState().syncedInputs).toEqual({ repositoryPath: "/repo", branchPrefix: "user", fingerprint: "f1" })

    // nothing changed: no sync, the check is told
    const reads = inputs.reads
    await syncIfChanged()
    expect(branchSyncStore.getState().syncCount).toBe(1)
    expect(inputs.reads).toBe(reads + 1)
    expect(branchSyncStore.getState().inputsCheckedAt).not.toBeNull()

    // a commit in the IDE: a sync, which forgets the check
    inputs.fingerprint = "f2"
    await syncIfChanged()
    expect(branchSyncStore.getState().syncCount).toBe(2)
    expect(branchSyncStore.getState().inputsCheckedAt).toBeNull()
  })
})

describe("one sync at a time", () => {
  beforeEach(() => {
    branchSyncStore.setState(branchSyncStore.getInitialState(), true)
    syncEvents.current = [branchesGrouped]
    syncResult.current = { status: "ok", data: null }
    inputs.fingerprint = "f1"
    inputs.onRead = null
    repository.path = "/repo"
    projectUpdates.length = 0
    settledPlaceholders.count = 0
    toasts.length = 0
    held.calls.length = 0
    held.enabled = true
  })

  // a test that leaves a sync held would hold every test after it
  afterEach(async () => {
    held.enabled = false
    while (held.calls.length > 0) {
      held.calls.shift()!.finish()
      await vi.waitFor(() => expect(held.calls.length > 0 || !branchSyncStore.getState().isSyncing).toBe(true))
    }
    await whenSynced()
  })

  /** The next held sync, once its request reached the backend (the inputs are read first) */
  async function nextCall(count: number) {
    await vi.waitFor(() => expect(held.calls).toHaveLength(count))
    return held.calls[count - 1]!
  }

  it("runs one quiet follow-up for every request made while a sync runs, Sync busy until it ends", async () => {
    let resolved = 0
    const first = syncBranches({ manual: true }).then(() => resolved++)
    const call1 = await nextCall(1)
    // a rewrite's resync, its Undo and a fetch while it runs
    const second = syncBranches({ autoScroll: false, autoExpand: false }).then(() => resolved++)
    const third = syncBranches({ autoScroll: false, autoExpand: false }).then(() => resolved++)
    expect(held.calls).toHaveLength(1)
    expect(branchSyncStore.getState().isSyncing).toBe(true)

    call1.finish()
    const call2 = await nextCall(2)
    expect(branchSyncStore.getState().syncCount).toBe(1)
    // never a frame between the two in which the branches look synced
    expect(branchSyncStore.getState().isSyncing).toBe(true)
    expect(resolved).toBe(0)

    call2.finish()
    await Promise.all([first, second, third])
    expect(resolved).toBe(3)
    expect(held.calls).toHaveLength(2)
    expect(branchSyncStore.getState().isSyncing).toBe(false)
    expect(branchSyncStore.getState().syncCount).toBe(2)
  })

  it("lets a push wait for the end of the follow-up, not of the first sync", async () => {
    void syncBranches()
    const call1 = await nextCall(1)
    void syncBranches({ autoScroll: false, autoExpand: false })
    let synced = false
    const waiting = whenSynced().then(() => {
      synced = true
    })
    call1.finish()
    const call2 = await nextCall(2)
    expect(synced).toBe(false)
    call2.finish()
    await waiting
    expect(synced).toBe(true)
  })

  it("drops the events of a run a newer one replaced, and of a run of the repository the window left", async () => {
    void syncBranches()
    const call1 = await nextCall(1)
    void syncBranches({ autoScroll: false, autoExpand: false })
    call1.finish()
    const call2 = await nextCall(2)
    // a message of the first run delivered after its invoke resolved
    call1.channel.onmessage({ type: "remoteStatusUpdate", data: remoteStatusUpdate("a", 3) })
    call2.finish()
    await whenSynced()
    expect(getBranch("a")?.remoteStatus).toBeNull()

    void syncBranches()
    const call3 = await nextCall(3)
    resetBranchSync()
    call3.channel.onmessage(branchesGrouped)
    call3.finish()
    await whenSynced()
    expect(branchSyncStore.getState().branchOrder).toEqual([])
    expect(branchSyncStore.getState().syncedRepositoryPath).toBeNull()
  })

  it("keeps the outcome of the last run: a failure fixed by the follow-up is gone, a failure of the follow-up stays", async () => {
    void syncBranches()
    let call = await nextCall(1)
    void syncBranches({ autoScroll: false, autoExpand: false })
    call.finish({ status: "error", error: { message: "boom", kind: null } })
    call = await nextCall(2)
    call.finish()
    await whenSynced()
    expect(branchSyncStore.getState().syncError).toBeNull()

    void syncBranches()
    call = await nextCall(3)
    void syncBranches({ autoScroll: false, autoExpand: false })
    call.finish()
    call = await nextCall(4)
    call.finish({ status: "error", error: { message: "boom", kind: null } })
    await whenSynced()
    expect(branchSyncStore.getState().syncError).toBe("boom")
  })

  it("ends the chain when a run throws", async () => {
    const done = syncBranches()
    const call = await nextCall(1)
    call.finish(Promise.reject(new Error("channel closed")))
    await done
    expect(branchSyncStore.getState().isSyncing).toBe(false)
    expect(branchSyncStore.getState().syncError).toBe("channel closed")
  })

  it("drops the follow-up once another repository is selected and resolves its callers, recording the sync for its own", async () => {
    void syncBranches()
    const call = await nextCall(1)
    const followUp = syncBranches({ autoScroll: false, autoExpand: false })
    repository.path = "/other"
    call.finish()
    await followUp
    expect(held.calls).toHaveLength(1)
    expect(branchSyncStore.getState().isSyncing).toBe(false)
    expect(projectUpdates.map(it => it.path)).toEqual(["/repo"])
  })

  it("settles the archived placeholders once, after the last run", async () => {
    void syncBranches()
    const call1 = await nextCall(1)
    void syncBranches({ autoScroll: false, autoExpand: false })
    call1.finish()
    const call2 = await nextCall(2)
    expect(settledPlaceholders.count).toBe(0)
    call2.finish()
    await whenSynced()
    expect(settledPlaceholders.count).toBe(1)
  })

  it("checks once after a sync what a focus during it found changed: a sync only when the inputs changed", async () => {
    // synced with f1
    void syncBranches()
    ;(await nextCall(1)).finish()
    await whenSynced()

    // a focus during a sync that read f1: nothing changed, no second sync
    void syncBranches()
    let call = await nextCall(2)
    void syncIfChanged()
    call.finish()
    await whenSynced()
    await vi.waitFor(() => expect(branchSyncStore.getState().inputsCheckedAt).not.toBeNull())
    expect(held.calls).toHaveLength(2)

    // a commit in the IDE while a sync that read f1 ran: the check after it syncs
    void syncBranches()
    call = await nextCall(3)
    inputs.fingerprint = "f2"
    void syncIfChanged()
    call.finish()
    ;(await nextCall(4)).finish()
    await whenSynced()
    expect(branchSyncStore.getState().syncedInputs?.fingerprint).toBe("f2")
  })

  it("syncs nothing on a focus whose read a whole sync of the same inputs overlapped", async () => {
    held.enabled = false
    await syncBranches()
    const reads = inputs.reads
    // the read answers "f2" (a commit); a sync that read "f2" too runs and ends before the answer arrives
    inputs.fingerprint = "f2"
    let overlapping: Promise<void> = Promise.resolve()
    inputs.onRead = () => {
      inputs.onRead = null
      overlapping = syncBranches()
    }
    await syncIfChanged()
    await overlapping
    expect(inputs.reads).toBe(reads + 2)
    expect(branchSyncStore.getState().syncCount).toBe(2)
  })
})

describe("deferred sync", () => {
  beforeEach(() => {
    branchSyncStore.setState(branchSyncStore.getInitialState(), true)
    syncEvents.current = []
    toasts.length = 0
    syncResult.current = { status: "error", error: { message: "Not synced: a rebase is in progress", kind: "operationInProgress" } }
  })

  it("is no failure: no error, the branches as they were, an info toast only for a sync the user asked for", async () => {
    await syncBranches()
    expect(branchSyncStore.getState().syncError).toBeNull()
    expect(toasts).toEqual([])
    await syncBranches({ manual: true })
    expect(branchSyncStore.getState().syncError).toBeNull()
    expect(toasts).toEqual([{ type: "info", title: "Not synced: a rebase is in progress", description: "The branches sync once git finished it" }])
    syncResult.current = { status: "ok", data: null }
  })
})

describe("quiet resync", () => {
  beforeEach(() => {
    branchSyncStore.setState(branchSyncStore.getInitialState(), true)
  })

  it("keeps what the row of an unchanged branch shows while the sync rebuilds it, and resets a changed or failed one", async () => {
    syncEvents.current = [
      branchesGrouped,
      { type: "commitSynced", data: { branchName: "a", commitHash: "a1", newHash: "a1-new", status: "Created" } },
      { type: "branchStatusUpdate", data: { branchName: "a", status: "Created", error: null } },
      { type: "branchStatusUpdate", data: { branchName: "b", status: "Error", error: { Generic: "boom" } } },
    ]
    await syncBranches()
    expect(getBranch("a")).toMatchObject({ status: "Created", commits: [{ hash: "a1-new" }] })

    // the next sync groups a as it was and b anew, and reports neither yet: a shows its last state until its result,
    // b, which failed, starts over
    syncEvents.current = [{ type: "branchesGrouped", data: { branches: [group("a"), group("b")], baselineBranch: "origin/master" } }]
    await syncBranches()
    expect(getBranch("a")).toMatchObject({ status: "Created", commits: [{ hash: "a1-new" }] })
    expect(getBranch("b")).toMatchObject({ status: "Syncing", hasError: false })

    // a changed branch starts over too
    const changed: GroupedBranchInfo = { ...group("a"), commits: [...group("a").commits, { ...group("a").commits[0]!, originalHash: "a2" }] }
    syncEvents.current = [{ type: "branchesGrouped", data: { branches: [changed, group("b")], baselineBranch: "origin/master" } }]
    await syncBranches()
    expect(getBranch("a")!.status).toBe("Syncing")
  })
})

describe("sync during a push", () => {
  beforeEach(() => {
    branchSyncStore.setState(branchSyncStore.getInitialState(), true)
    syncEvents.current = [branchesGrouped]
  })

  it("keeps the push state of a branch", async () => {
    await syncBranches()
    setBranchPushing("a", true)
    // mark one to prove the second sync rebuilt the records from its grouping
    branchSyncStore.setState(state => ({ branches: { ...state.branches, a: { ...state.branches.a!, summary: "marked" } } }))

    await syncBranches()

    expect(getBranch("a")!.summary).toBe("subject")
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
    syncEvents.current = [branchesGrouped, { type: "remoteStatusUpdate", data: remoteStatusUpdate("a", 2) }, { type: "remoteStatusUpdate", data: remoteStatusUpdate("b", 3) }]

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
    return getBranchTableRows().map(row => (row.kind === "issue" ? `i:${row.members.length}:${row.issueKey}` : `${row.showIssueKey ? "c" : row.depth > 0 ? "n" : "b"}:${row.name}`))
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
