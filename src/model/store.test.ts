// The page's side of a session on the fake sessions of the backend (`src/test-support/fake-sessions.ts`): the store
// applies the patches of the session it subscribed to, and asks a sync of it
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test"
import { internalError } from "@/lib/api-error"
import type { StackBranchUpdate, TableChanges, Version } from "@/lib/bindings"
import { resetDemand } from "@/model/demand"
import { closeSession, openSession, requestSessionSync, setSessionWindowFocus, whenSessionReady, whenSessionViewAt } from "@/model/store"
import { branchSyncStore, getBranch, getStackBranch, setBranchPushing, toRemoteStatus, whenSynced } from "@/stores/branch-sync"
import { emptyView, fakeRemoteStatus, fakeSessions, grouping } from "@/test-support/fake-sessions"

const { toasts, archived, syncRecords, prefixLoads } = vi.hoisted(() => ({
  toasts: [] as { type?: string; title: string; description?: string }[],
  /** How often the page resolved the prefix of the repository again (`refreshBranchPrefix`), and the prefix it shows then */
  prefixLoads: { count: 0, prefix: "team" },
  /** Each `updateFromArchivedNames` of the archived branches: the names, and the ones marked new */
  archived: { names: [] as [readonly string[], readonly string[]][], settled: 0 },
  /** What each sync that ended recorded of its repository (`repositoryState`) */
  syncRecords: [] as { path: string; record: unknown }[],
}))

vi.mock("@tauri-apps/api/core", () => import("@/test-support/channel-mock"))
vi.mock("@/lib/bindings", async () => ({ commands: (await import("@/test-support/fake-sessions")).sessionCommands() }))
vi.mock("@/lib/toast-message", () => ({ showToast: (toast: { type?: string; title: string; description?: string }) => toasts.push(toast) }))
vi.mock("@/stores/repository", () => ({
  getSelectedProject: () => ({ path: "/repo" }),
  getSelectedRepositoryPath: () => "/repo",
  refreshBranchPrefix: async () => {
    prefixLoads.count++
  },
  getEffectiveBranchPrefix: () => prefixLoads.prefix,
}))
vi.mock("@/stores/settings/repositorySettings", () => ({ updateProject: () => {} }))
vi.mock("@/stores/settings/repositoryState", () => ({
  updateRepositoryState: (path: string, update: (state: object) => unknown) => syncRecords.push({ path, record: update({}) }),
  // the repository state is what the syncs of the test recorded
  getRepositoryState: (path: string) => syncRecords.findLast(it => it.path === path)?.record,
}))
vi.mock("@/stores/landed", () => ({ hideAfterSync: () => {}, setLandedStatus: () => {} }))
vi.mock("@/stores/unapplied-branches", () => ({ setUnappliedBranches: () => {}, resetUnappliedBranches: () => {} }))
vi.mock("@/stores/archived-branches", () => ({
  updateFromArchivedNames: (names: readonly string[], marked: readonly string[]) => archived.names.push([names, marked]),
  updateFromIntegrationInfos: () => {},
  settleArchivedPlaceholders: () => {
    archived.settled++
  },
  resetArchivedBranches: () => {},
}))

/** The window subscribed to the session of `repositoryPath`, its view in the store */
async function open(repositoryPath = "/repo"): Promise<void> {
  openSession(repositoryPath)
  await whenSessionReady()
}

/** Sends `changes` as a patch of the session of /repo and waits until the store holds its version */
async function send(changes: TableChanges, repositoryPath = "/repo"): Promise<Version> {
  const { next } = fakeSessions.send(repositoryPath, changes)
  await vi.waitFor(() => expect(branchSyncStore.getState().viewVersion).toEqual(next))
  return next
}

/** The sync request the session got last, once it reached it */
async function nextSync(count: number) {
  await vi.waitFor(() => expect(fakeSessions.syncs).toHaveLength(count))
  return fakeSessions.syncs[count - 1]!
}

const running: TableChanges = { sync: { kind: "running", step: "grouping" } }
const idle: TableChanges = { sync: { kind: "idle" } }

beforeEach(() => {
  fakeSessions.reset()
  branchSyncStore.setState(branchSyncStore.getInitialState(), true)
  toasts.length = 0
  archived.names.length = 0
  archived.settled = 0
  syncRecords.length = 0
  prefixLoads.count = 0
  prefixLoads.prefix = "team"
})

// a request left waiting would hold the tests after it
afterEach(async () => {
  for (const sync of fakeSessions.syncs) {
    sync.answer()
  }
  closeSession()
  resetDemand()
  await whenSynced()
})

describe("the view of the session", () => {
  it("names only the repository: the session resolves the prefix, and a sync waits for no load of the page", async () => {
    await open()
    const synced = requestSessionSync("manual")
    const sync = await nextSync(1)
    expect(fakeSessions.subscribeCalls).toEqual([{ repositoryPath: "/repo" }])
    expect(sync.params).toEqual({ repositoryPath: "/repo", reason: "manual" })
    await send(grouping("a", "b"))
    sync.answer()
    await synced
    expect(branchSyncStore.getState()).toMatchObject({ branchOrder: ["a", "b"], syncError: null, syncedRepositoryPath: "/repo" })
  })

  it("takes the snapshot when a patch of another epoch comes: the session was opened again", async () => {
    await open()
    await send(grouping("a"))
    // the session went away while idle and opened again: its patches start on a new epoch
    fakeSessions.reopen("/repo", { ...emptyView({ epoch: 2, seq: 0 }), order: ["c"] })
    fakeSessions.send("/repo", grouping("c"))
    await vi.waitFor(() => expect(branchSyncStore.getState().viewVersion).toEqual({ epoch: 2, seq: 1 }))
    expect(fakeSessions.snapshotCalls).toEqual(["/repo"])
    expect(branchSyncStore.getState().branchOrder).toEqual(["c"])
    expect(getBranch("a")).toBeUndefined()
  })

  it("takes the snapshot when the base of a patch is missing, and drops a patch the snapshot already has", async () => {
    await open()
    const first = fakeSessions.send("/repo", grouping("a"))
    await vi.waitFor(() => expect(branchSyncStore.getState().viewVersion).toEqual(first.next))
    // a patch the window missed: the next one is on a base it doesn't have
    fakeSessions.views.set("/repo", { ...fakeSessions.view("/repo"), version: { epoch: 1, seq: 2 } })
    const gap = fakeSessions.send("/repo", { ...grouping("a", "b"), sync: idle.sync })
    await vi.waitFor(() => expect(branchSyncStore.getState().viewVersion).toEqual(gap.next))
    expect(fakeSessions.snapshotCalls).toEqual(["/repo"])
    expect(branchSyncStore.getState().branchOrder).toEqual(["a", "b"])
    // delivered again (a late copy): the view already has it
    fakeSessions.deliver("/repo", first)
    expect(branchSyncStore.getState().viewVersion).toEqual(gap.next)
    expect(fakeSessions.snapshotCalls).toHaveLength(1)
  })

  it("keeps the patches that come before the subscription answered, and applies them on its view", async () => {
    fakeSessions.holdSubscriptions = true
    openSession("/repo")
    await vi.waitFor(() => expect(fakeSessions.subscriptions).toHaveLength(1))
    // the view at the subscription is before this patch: the channel delivers it before the answer
    const patch = fakeSessions.send("/repo", grouping("a"))
    expect(branchSyncStore.getState().viewVersion).toBeNull()
    fakeSessions.releaseSubscriptions()
    await whenSessionReady()
    expect(branchSyncStore.getState().viewVersion).toEqual(patch.next)
    expect(branchSyncStore.getState().branchOrder).toEqual(["a"])
    expect(fakeSessions.snapshotCalls).toEqual([])
  })

  it("subscribes again when the snapshot finds the session gone, keeping the table until the new view comes", async () => {
    await open()
    await send(grouping("a"))
    // the session closed (its worktree went away) and opened again
    fakeSessions.snapshotAnswer = null
    fakeSessions.reopen("/repo", { ...emptyView({ epoch: 2, seq: 0 }), order: ["c"] })
    fakeSessions.holdSubscriptions = true
    fakeSessions.deliver("/repo", { base: { epoch: 2, seq: 0 }, next: { epoch: 2, seq: 1 }, changes: idle })
    await vi.waitFor(() => expect(fakeSessions.subscribeCalls).toHaveLength(2))
    expect(branchSyncStore.getState().branchOrder).toEqual(["a"])
    fakeSessions.releaseSubscriptions()
    await whenSessionReady()
    expect(branchSyncStore.getState().branchOrder).toEqual(["c"])
  })

  it("forgets the table of the session it leaves: a late patch of it reaches nothing", async () => {
    await open()
    await send(grouping("a"))
    openSession("/other")
    await whenSessionReady()
    expect(branchSyncStore.getState().branchOrder).toEqual([])
    expect(fakeSessions.subscriptions.map(it => it.repositoryPath)).toEqual(["/other"])
    fakeSessions.send("/repo", grouping("a", "b"))
    expect(branchSyncStore.getState().branchOrder).toEqual([])
  })

  it("keeps the subscription of a second window independent of the first's: its own view, its own count of the runs", async () => {
    await open()
    await send(grouping("a"))
    // the review window of a branch loads the modules afresh: its own store and table, the same backend
    vi.resetModules()
    const second = await import("@/model/store")
    const secondTable = (await import("@/stores/branch-sync")).branchSyncStore
    second.openSession("/repo")
    await second.whenSessionReady()
    expect(fakeSessions.subscriptions.map(it => it.repositoryPath)).toEqual(["/repo", "/repo"])
    // the snapshot at its subscription
    expect(secondTable.getState().branchOrder).toEqual(["a"])
    // a run asked by the first window reaches both, and each counts it once it ended
    await send(running)
    await send(grouping("a", "b"))
    const ended = await send(idle)
    await vi.waitFor(() => expect(secondTable.getState().viewVersion).toEqual(ended))
    expect(secondTable.getState().branchOrder).toEqual(["a", "b"])
    expect([branchSyncStore.getState().syncCount, secondTable.getState().syncCount]).toEqual([1, 1])
    // the first window switches to another repository: the second keeps its subscription and the patches
    openSession("/other")
    await whenSessionReady()
    const { next } = fakeSessions.send("/repo", grouping("a", "b", "c"))
    await vi.waitFor(() => expect(secondTable.getState().viewVersion).toEqual(next))
    expect(secondTable.getState().branchOrder).toEqual(["a", "b", "c"])
    expect(branchSyncStore.getState().branchOrder).toEqual([])
    // the close of the second ends only its own subscription
    second.closeSession()
    expect(fakeSessions.subscriptions.map(it => it.repositoryPath)).toEqual(["/other"])
  })
})

describe("a sync of the session", () => {
  it("keeps Sync busy from the request to the last patch of the run that serves it, and counts the runs that ended", async () => {
    await open()
    // the view of the subscription settled the placeholders of the archived branches once already
    archived.settled = 0
    const synced = requestSessionSync("manual")
    // busy from the click, before the run's first patch
    expect(branchSyncStore.getState().isSyncing).toBe(true)
    const sync = await nextSync(1)
    await send({ ...running, ...grouping("a") })
    // the answer comes before the run's last patch: the request waits for it
    fakeSessions.views.set("/repo", { ...fakeSessions.view("/repo"), version: { epoch: 1, seq: 2 } })
    sync.answer()
    let ended = false
    void synced.then(() => {
      ended = true
    })
    await new Promise(resolve => setTimeout(resolve, 0))
    expect(ended).toBe(false)
    expect(branchSyncStore.getState().isSyncing).toBe(true)
    fakeSessions.views.set("/repo", { ...fakeSessions.view("/repo"), version: { epoch: 1, seq: 1 } })
    await send(idle)
    expect(await synced).toEqual({ outcome: "synced", version: { epoch: 1, seq: 2 } })
    expect(branchSyncStore.getState()).toMatchObject({ isSyncing: false, syncCount: 1, syncedRepositoryPath: "/repo" })
    // every sync teaches the usual time of a sync, with the branch count
    expect(syncRecords).toEqual([{ path: "/repo", record: expect.objectContaining({ lastBranchCount: 1, updateTimings: { sync: [expect.any(Number)] } }) }])
    // once, after the run: a placeholder still waiting got no result
    expect(archived.settled).toBe(1)
  })

  it("tells the session the focus of the window, and marks the check of a gained one that found nothing changed", async () => {
    // no repository: the window names none
    expect(await setSessionWindowFocus(true)).toBeNull()
    expect(fakeSessions.focusCalls).toEqual([])
    await open()
    const checked = setSessionWindowFocus(true)
    const check = await nextSync(1)
    expect(check.params).toEqual({ repositoryPath: "/repo", reason: "focus" })
    // Sync is not busy for the check: a run it starts shows in the view
    expect(branchSyncStore.getState().isSyncing).toBe(false)
    check.answer("unchanged")
    expect(await checked).toEqual({ outcome: "unchanged", version: { epoch: 1, seq: 0 } })
    expect(branchSyncStore.getState().inputsCheckedAt).not.toBeNull()
    expect(syncRecords).toEqual([])
    // a lost focus, and a gained one while the backend's Auto-sync on Focus is off: told, nothing to wait for
    expect(await setSessionWindowFocus(false)).toBeNull()
    fakeSessions.syncsOnFocus = false
    expect(await setSessionWindowFocus(true)).toBeNull()
    expect(fakeSessions.focusCalls.map(it => it.focused)).toEqual([true, false, true])
    expect(fakeSessions.syncs).toHaveLength(1)
    // a lost focus asks for no sync: its failure leaves Sync as it is, a gained one's names itself
    fakeSessions.focusFailure = { ...internalError("Cannot sync without a branch prefix"), code: "no_branch_prefix" }
    expect(await setSessionWindowFocus(false)).toBeNull()
    expect(branchSyncStore.getState().syncError).toBeNull()
    expect(await setSessionWindowFocus(true)).toBeNull()
    expect(branchSyncStore.getState().syncError).toBe("Cannot sync without a branch prefix")
  })

  it("tells its session what the window shows by its subscription, again to a new one, with the focus of the window", async () => {
    await open()
    const first = fakeSessions.subscriptions[0]!.id
    await vi.waitFor(() => expect(fakeSessions.demandCalls.at(-1)).toMatchObject({ subscription: first, demand: { focused: false } }))
    void setSessionWindowFocus(true)
    await vi.waitFor(() => expect(fakeSessions.demandCalls.at(-1)).toMatchObject({ subscription: first, demand: { focused: true } }))
    for (const sync of fakeSessions.syncs) {
      sync.answer("unchanged")
    }

    openSession("/repo")
    await whenSessionReady()
    const second = fakeSessions.subscriptions.at(-1)!.id
    expect(second).not.toBe(first)
    await vi.waitFor(() => expect(fakeSessions.demandCalls.at(-1)).toMatchObject({ subscription: second, demand: { focused: true } }))
  })

  it("records the sync a focus started, Sync busy from its answer to the run's last patch", async () => {
    await open()
    const focused = setSessionWindowFocus(true)
    const check = await nextSync(1)
    // the run's two patches are still on their way when the answer comes
    fakeSessions.views.set("/repo", { ...fakeSessions.view("/repo"), version: { epoch: 1, seq: 2 } })
    check.answer()
    await vi.waitFor(() => expect(branchSyncStore.getState().isSyncing).toBe(true))
    fakeSessions.views.set("/repo", { ...fakeSessions.view("/repo"), version: { epoch: 1, seq: 0 } })
    await send({ ...running, ...grouping("a") })
    await send(idle)
    expect(await focused).toEqual({ outcome: "synced", version: { epoch: 1, seq: 2 } })
    expect(branchSyncStore.getState()).toMatchObject({ isSyncing: false, syncCount: 1, syncedRepositoryPath: "/repo", inputsCheckedAt: null })
    expect(syncRecords).toEqual([{ path: "/repo", record: expect.objectContaining({ lastBranchCount: 1 }) }])
  })

  it("is no failure when deferred: no error, the table as it was, an info toast only for a sync the user asked for", async () => {
    await open()
    await send(grouping("a"))
    const deferred = { ...internalError("Not synced: a rebase is in progress"), code: "operation_in_progress" } as const
    const quiet = requestSessionSync("afterRewrite")
    ;(await nextSync(1)).fail(deferred)
    expect(await quiet).toBeNull()
    const manual = requestSessionSync("manual")
    ;(await nextSync(2)).fail(deferred)
    await manual
    expect(branchSyncStore.getState()).toMatchObject({ syncError: null, branchOrder: ["a"], isSyncing: false })
    expect(toasts).toEqual([{ type: "info", title: "Not synced: a rebase is in progress", description: "The branches sync once git finished it" }])

    // the view says why it waits, in the words of the toast, until the next run
    await send({ sync: { kind: "deferred", error: deferred } })
    expect(branchSyncStore.getState()).toMatchObject({
      syncError: null,
      syncDeferred: { title: "Not synced: a rebase is in progress", description: "The branches sync once git finished it" },
    })
    const shown = branchSyncStore.getState().syncDeferred
    await send({ branches: { a: { sync: { status: "Unchanged", processedCount: 1, hasError: false } } } })
    expect(branchSyncStore.getState().syncDeferred).toBe(shown)
    await send(running)
    expect(branchSyncStore.getState().syncDeferred).toBeNull()
    await send({ sync: { kind: "deferred", error: { ...deferred, code: "detached_head", message: "Not synced: HEAD is detached" } } })
    expect(branchSyncStore.getState().syncDeferred).toEqual({ title: "Not synced: HEAD is detached", description: "Check out a branch to sync its commits" })
  })

  it("names the step of the running sync, and none once it ended", async () => {
    await open()
    expect(branchSyncStore.getState().syncStep).toBeNull()
    await send(running)
    expect(branchSyncStore.getState().syncStep).toBe("grouping")
    await send({ sync: { kind: "running", step: "building" } })
    expect(branchSyncStore.getState().syncStep).toBe("building")
    await send(idle)
    expect(branchSyncStore.getState().syncStep).toBeNull()
  })

  it("resolves the prefix again when the git config names another one than the session's, quietly", async () => {
    // a repository synced before: the window syncs the session of the new prefix as it opens it (`useSyncOnOpen`)
    await open()
    const first = requestSessionSync("manual")
    await send(grouping("a"))
    ;(await nextSync(1)).answer()
    await first
    const changed = { ...internalError('The branch prefix of the repository is "team" now'), code: "prefix_changed" } as const
    const manual = requestSessionSync("manual")
    ;(await nextSync(2)).fail(changed)
    expect(await manual).toBeNull()
    // nothing failed meanwhile, and nothing asked again
    expect(prefixLoads.count).toBe(1)
    expect(fakeSessions.syncs).toHaveLength(2)
    expect(branchSyncStore.getState()).toMatchObject({ syncError: null, branchOrder: ["a"], isSyncing: false })
    expect(toasts).toEqual([])
    expect(syncRecords).toHaveLength(1)
  })

  it("asks the user's own sync of a repository never synced again once the page has the new prefix, Sync busy throughout", async () => {
    await open()
    const changed = { ...internalError('The branch prefix of the repository is "team" now'), code: "prefix_changed" } as const
    const manual = requestSessionSync("manual")
    ;(await nextSync(1)).fail(changed)
    // the run resolved the prefix again in Rust: the request asked again reaches the session of the new one
    const again = await nextSync(2)
    expect(again.params).toEqual({ repositoryPath: "/repo", reason: "manual" })
    expect(prefixLoads.count).toBe(1)
    expect(branchSyncStore.getState().isSyncing).toBe(true)
    await send(grouping("a"))
    again.answer()
    expect(await manual).toEqual({ outcome: "synced", version: { epoch: 1, seq: 1 } })
    expect(branchSyncStore.getState()).toMatchObject({ syncError: null, branchOrder: ["a"], isSyncing: false, syncedRepositoryPath: "/repo" })
    expect(syncRecords).toEqual([{ path: "/repo", record: expect.objectContaining({ lastBranchCount: 1 }) }])

    // a sync nobody asked of the window, and a repository left without a prefix, ask nothing again
    syncRecords.length = 0
    const quiet = requestSessionSync("afterRewrite")
    ;(await nextSync(3)).fail(changed)
    expect(await quiet).toBeNull()
    prefixLoads.prefix = ""
    const unnamed = requestSessionSync("manual")
    ;(await nextSync(4)).fail(changed)
    expect(await unnamed).toBeNull()
    expect(fakeSessions.syncs).toHaveLength(4)
    expect(prefixLoads.count).toBe(3)
  })

  it("subscribes again when a sync answers another epoch than its view's: the actor of the session died, and the sync opened it again", async () => {
    await open()
    await send(grouping("a"))
    // the subscription of the window went with the actor: no patch of the new session reaches it
    fakeSessions.dropSession("/repo", { ...emptyView({ epoch: 2, seq: 0 }), order: ["c"] })
    const synced = requestSessionSync("manual")
    const sync = await nextSync(1)
    fakeSessions.send("/repo", grouping("c"))
    expect(branchSyncStore.getState().branchOrder).toEqual(["a"])
    sync.answer()
    expect(await synced).toEqual({ outcome: "synced", version: { epoch: 2, seq: 1 } })
    // the session open now has another epoch than the view's: the window's subscription went with the actor
    expect(fakeSessions.snapshotCalls).toEqual(["/repo"])
    expect(fakeSessions.subscribeCalls).toHaveLength(2)
    expect(branchSyncStore.getState()).toMatchObject({ viewVersion: { epoch: 2, seq: 1 }, branchOrder: ["c"], isSyncing: false, syncedRepositoryPath: "/repo" })
    expect(syncRecords).toEqual([{ path: "/repo", record: expect.objectContaining({ lastBranchCount: 1 }) }])
    // the next run's patches reach the window on its new subscription
    await send({ ...running, ...grouping("c", "d") })
    expect(branchSyncStore.getState().branchOrder).toEqual(["c", "d"])
  })

  it("keeps Sync busy for the requests still in flight when it subscribes again: the subscription made again counts them", async () => {
    await open()
    await send(grouping("a"))
    const first = requestSessionSync("manual")
    const second = requestSessionSync("manual")
    const queued = await nextSync(2)
    const sync = fakeSessions.syncs[0]!
    fakeSessions.dropSession("/repo", { ...emptyView({ epoch: 2, seq: 0 }), order: ["c"] })
    fakeSessions.send("/repo", grouping("c"))
    sync.answer()
    expect(await first).toEqual({ outcome: "synced", version: { epoch: 2, seq: 1 } })
    expect(fakeSessions.subscribeCalls).toHaveLength(2)
    expect(branchSyncStore.getState()).toMatchObject({ viewVersion: { epoch: 2, seq: 1 }, branchOrder: ["c"], isSyncing: true })
    queued.answer()
    await second
    expect(branchSyncStore.getState().isSyncing).toBe(false)
  })

  it("drops the late answer of the session of the prefix it left: its subscription to the new one holds, Sync busy for the request of the new one", async () => {
    await open()
    await send(grouping("a"))
    const old = requestSessionSync("manual")
    const oldSync = await nextSync(1)
    const oldVersion = fakeSessions.view("/repo").version
    // a prefix written in Settings: the window opens the session of the new prefix, in its own epoch, and syncs it
    fakeSessions.reopen("/repo", { ...emptyView({ epoch: 2, seq: 0 }), order: ["c"] })
    openSession("/repo")
    await whenSessionReady()
    const opened = requestSessionSync("open")
    const openSync = await nextSync(2)
    // the run of the old prefix had passed its check: it ends under the old prefix and answers its epoch
    oldSync.answer("synced", oldVersion)
    expect(await old).toEqual({ outcome: "synced", version: oldVersion })
    expect(fakeSessions.snapshotCalls).toEqual(["/repo"])
    expect(fakeSessions.subscribeCalls).toHaveLength(2)
    expect(fakeSessions.subscriptions).toHaveLength(1)
    expect(branchSyncStore.getState()).toMatchObject({ viewVersion: { epoch: 2, seq: 0 }, isSyncing: true, syncedRepositoryPath: null })
    expect(syncRecords).toEqual([])
    // the run of the new session ends: its answer is recorded, its patches reach the window on the subscription kept
    await send(grouping("c", "d"))
    openSync.answer()
    expect(await opened).toEqual({ outcome: "synced", version: { epoch: 2, seq: 1 } })
    expect(branchSyncStore.getState()).toMatchObject({ branchOrder: ["c", "d"], isSyncing: false, syncedRepositoryPath: "/repo" })
    expect(syncRecords).toEqual([{ path: "/repo", record: expect.objectContaining({ lastBranchCount: 2 }) }])
  })

  it("names the failure of a refused request, and of a run the failed state of the view", async () => {
    await open()
    const refused = requestSessionSync("manual")
    ;(await nextSync(1)).fail(internalError("boom"))
    await refused
    expect(branchSyncStore.getState().syncError).toBe("boom")
    // the next run clears it, and its own failure names itself, the table kept
    await send({ ...running, ...grouping("a") })
    expect(branchSyncStore.getState().syncError).toBeNull()
    await send({ sync: { kind: "failed", error: internalError("Cannot read HEAD") } })
    expect(branchSyncStore.getState()).toMatchObject({ syncError: "Cannot read HEAD", branchOrder: ["a"], syncCount: 1 })
  })

  it("subscribes first when asked before the window subscribed, and again after a failed subscription, whose failure it names", async () => {
    // a click while the page still loads the prefix: no subscription yet
    const synced = requestSessionSync("manual")
    await vi.waitFor(() => expect(fakeSessions.subscribeCalls).toEqual([{ repositoryPath: "/repo" }]))
    ;(await nextSync(1)).answer()
    expect(await synced).not.toBeNull()
    // a repository without a prefix then: its subscription fails, the request says why and syncs nothing; the welcome
    // card's state, so nothing is logged as an error
    fakeSessions.subscribeFailure = { ...internalError("Cannot sync without a branch prefix"), code: "no_branch_prefix" }
    const logged = vi.spyOn(console, "error")
    openSession("/repo")
    await whenSessionReady()
    expect(logged).not.toHaveBeenCalled()
    logged.mockRestore()
    expect(await requestSessionSync("manual")).toBeNull()
    expect(fakeSessions.syncs).toHaveLength(1)
    expect(branchSyncStore.getState().syncError).toBe("Cannot sync without a branch prefix")
    // the prefix is set: the next request subscribes again and syncs
    fakeSessions.subscribeFailure = null
    const again = requestSessionSync("manual")
    ;(await nextSync(2)).answer()
    expect(await again).not.toBeNull()
    expect(fakeSessions.subscribeCalls).toHaveLength(4)
    expect(branchSyncStore.getState().syncError).toBeNull()
  })

  it("leaves a request of the session the window left to it: Sync of the next one is not busy, nothing of it is recorded", async () => {
    await open()
    const old = requestSessionSync("manual")
    const sync = await nextSync(1)
    openSession("/other")
    await whenSessionReady()
    expect(branchSyncStore.getState().isSyncing).toBe(false)
    fakeSessions.send("/repo", { ...running, ...grouping("a") })
    sync.answer()
    await old
    expect(branchSyncStore.getState()).toMatchObject({ isSyncing: false, branchOrder: [], syncedRepositoryPath: null, syncCount: 0 })
    expect(syncRecords).toEqual([])
  })
})

describe("the table of the view", () => {
  it("applies a patch as its channel delivers it, on no timer of the page: a window not shown yet, whose timers WebKit throttles, holds it at once", async () => {
    await open()
    const { next } = fakeSessions.send("/repo", grouping("a"))
    // no await: neither a timer nor a microtask lies between the message and the store
    expect(branchSyncStore.getState().viewVersion).toEqual(next)
    expect(branchSyncStore.getState().branchOrder).toEqual(["a"])
  })

  it("keeps the records of the branches a patch leaves alone, and of the commits of a branch whose state alone changed", async () => {
    await open()
    await send(grouping("a", "b"))
    const { branches, branchOrder } = branchSyncStore.getState()
    // a quiet resync: Rust keeps what the rows show, only the state of the run changes
    await send(running)
    expect(branchSyncStore.getState().branches).toBe(branches)
    expect(branchSyncStore.getState().branchOrder).toBe(branchOrder)
    // a's result: its record is new, its commits the same; b untouched
    await send({ branches: { a: { sync: { status: "Unchanged", processedCount: 1, hasError: false } } } })
    expect(getBranch("a")).toMatchObject({ status: "Unchanged", statusText: "unchanged", processedCount: 1 })
    expect(getBranch("a")!.commits).toBe(branches.a!.commits)
    expect(getBranch("b")).toBe(branches.b)
    // a copy of a's commit changes its commits
    await send({ branches: { a: { copies: [{ hash: "a1-new", status: "Created" }] } } })
    expect(getBranch("a")!.commits).toEqual([expect.objectContaining({ originalHash: "a1", hash: "a1-new", status: "Created" })])
  })

  it("shows the remote status every patch brings, also while a push of the branch runs: Rust orders the push's outcome against the syncs", async () => {
    await open()
    await send({ ...grouping("a", "b"), branches: { ...grouping("a", "b").branches, a: { ...grouping("a").branches!.a!, remote: { status: fakeRemoteStatus("a", 1) } } } })
    const known = getBranch("a")!.remoteStatus
    expect(known).toEqual(toRemoteStatus(fakeRemoteStatus("a", 1)))
    // a patch without a remote part keeps the status the branch shows, the same object
    await send({ branches: { a: { sync: { status: "Unchanged", processedCount: 1, hasError: false } } } })
    expect(getBranch("a")!.remoteStatus).toBe(known)
    // the spinner of a push (one that waits for a sync too) is the page's; the status is the session's
    setBranchPushing("a", true, "/repo")
    await send({ branches: { a: { remote: { status: fakeRemoteStatus("a", 2, "moved") } }, b: { remote: { status: fakeRemoteStatus("b", 3) } } } })
    expect(getBranch("a")!.remoteStatus).toMatchObject({ remoteTip: "moved", commitsAhead: 2 })
    expect(getBranch("b")!.remoteStatus!.commitsAhead).toBe(3)
    setBranchPushing("a", false, "/repo")
  })

  it("lets a command whose result the session applied wait for the patch of the version it answers", async () => {
    await open()
    await send(grouping("a"))
    const at = fakeSessions.view("/repo").version
    const next = { epoch: at.epoch, seq: at.seq + 1 }
    let reached = false
    const waiting = whenSessionViewAt("/repo", next).then(() => (reached = true))
    await Promise.resolve()
    expect(reached).toBe(false)
    await send({ branches: { a: { remote: { status: fakeRemoteStatus("a", 0, "pushed") } } } })
    await waiting
    expect(getBranch("a")!.remoteStatus!.remoteTip).toBe("pushed")
    // no version (no session was open) and another repository wait for nothing
    await whenSessionViewAt("/repo", null)
    await whenSessionViewAt("/other", { epoch: at.epoch, seq: at.seq + 5 })
  })

  it("keeps the stack branches the patches report until one changes or removes them", async () => {
    await open()
    const stack = (root: string, changes: Partial<StackBranchUpdate> = {}): StackBranchUpdate => ({
      root,
      branchName: `user/stack/${root}`,
      members: [root, "b"],
      commitCount: 2,
      state: "Built",
      tip: `${root}-tip`,
      blockedBy: null,
      conflict: null,
      error: null,
      remote: fakeRemoteStatus(`user/stack/${root}`, 2),
      ...changes,
    })
    await send({ ...grouping("a", "b"), stacks: { a: stack("a"), x: stack("x", { members: [], state: "Stale", error: "Not rebuilt" }) } })
    expect(getStackBranch("a")).toMatchObject({ branchName: "user/stack/a", members: ["a", "b"], state: "Built", tip: "a-tip" })
    expect(getStackBranch("a")!.remoteStatus!.commitsAhead).toBe(2)
    expect(getStackBranch("x")).toMatchObject({ state: "Stale", members: [] })
    const before = branchSyncStore.getState().stackBranches
    await send({ branches: { a: { sync: { status: "Unchanged", processedCount: 1, hasError: false } } } })
    expect(branchSyncStore.getState().stackBranches).toBe(before)
    await send({ stacks: { a: stack("a", { state: "Blocked", blockedBy: { member: "b", reason: "Conflict" } }) }, removedStacks: ["x"] })
    expect(getStackBranch("a")).toMatchObject({ state: "Blocked", blockedBy: { member: "b", reason: "Conflict" }, tip: "a-tip" })
    expect(getStackBranch("x")).toBeUndefined()
    closeSession()
    expect(getStackBranch("a")).toBeUndefined()
  })

  it("splits the order into the rows of each section by the section of each branch's structure, the view sends no second list", async () => {
    await open()
    await send(grouping("a", "b", "c"))
    const { rows, branchOrder } = branchSyncStore.getState()
    // nothing parked: the main rows are the order itself
    expect(rows).toEqual({ main: ["a", "b", "c"], parked: [] })
    expect(rows.main).toBe(branchOrder)
    // a patch of no structure keeps the rows, the same object
    await send({ branches: { a: { sync: { status: "Unchanged", processedCount: 1, hasError: false } } } })
    expect(branchSyncStore.getState().rows).toBe(rows)
    await send({ branches: { b: { structure: { parked: { note: "", with: null }, section: "parked" } } } })
    expect(branchSyncStore.getState().rows).toEqual({ main: ["a", "c"], parked: ["b"] })
    await send({ branches: { b: { structure: {} } } })
    expect(branchSyncStore.getState().rows).toEqual({ main: ["a", "b", "c"], parked: [] })
  })

  it("counts a change of what the table of the backend is made of, not of the state of a run, of a build or of a copy", async () => {
    await open()
    await send({ ...grouping("a"), branches: { a: { ...grouping("a").branches!.a!, copies: [{ hash: "a1-new", status: "Created" }] } } })
    const revision = () => branchSyncStore.getState().tableRevision
    const commits = () => branchSyncStore.getState().commitsRevision
    const [before, commitsBefore] = [revision(), commits()]
    // a sync that changes nothing: the run, the branch and its copy say Unchanged
    await send(running)
    await send({ branches: { a: { sync: { status: "Unchanged", processedCount: 1, hasError: false }, copies: [{ hash: "a1-new", status: "Unchanged" }] } } })
    await send(idle)
    expect([revision(), commits()]).toEqual([before, commitsBefore])
    // a new copy, a moved remote branch: another table; only the copy changed the commits the conflict fix is for
    await send({ branches: { a: { copies: [{ hash: "a1-newer", status: "Created" }] } } })
    expect([revision(), commits()]).toEqual([before + 1, commitsBefore + 1])
    await send({ branches: { a: { remote: { status: fakeRemoteStatus("a", 1) } } } })
    expect([revision(), commits()]).toEqual([before + 2, commitsBefore + 1])
    // a pushed stack branch and a deleted archived branch: their sections, not the commits
    const stack: StackBranchUpdate = {
      root: "a",
      branchName: "user/stack/a",
      members: ["a"],
      commitCount: 1,
      state: "Built",
      tip: "a-tip",
      blockedBy: null,
      conflict: null,
      error: null,
      remote: null,
    }
    await send({ stacks: { a: stack } })
    expect(commits()).toBe(commitsBefore + 2)
    await send({ stacks: { a: { ...stack, remote: fakeRemoteStatus("user/stack/a", 0) } }, archived: { names: [] } })
    expect([revision(), commits()]).toEqual([before + 4, commitsBefore + 2])
  })

  it("marks new only the branches a patch archived: a mark the user dismissed stays dismissed", async () => {
    await open()
    await send({ archived: { names: ["x"], newlyArchived: ["x"] } })
    await send({ archived: { names: ["x", "y"], newlyArchived: ["x", "y"] } })
    expect(archived.names.slice(-2)).toEqual([
      [["x"], ["x"]],
      [["x", "y"], ["y"]],
    ])
  })
})
