// @spec spec/repository/activity.spec.md
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type { LandedBranch, PatronusRobot, UpdateProjectResult } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import type { ActivityEntry } from "@/stores/activity"
import {
  activityStore,
  canUndoActivity,
  clearActivity,
  groupActivityByDay,
  MAX_ACTIVITY_AGE_MS,
  MAX_ACTIVITY_ENTRIES,
  recordActivity,
  recordLanded,
  recordPush,
  recordRewrite,
  recordRobot,
  recordUpdate,
  selectActivity,
  undoActivity,
  UNDO_WINDOW_MS,
} from "@/stores/activity"
import type { ArrayStoreState } from "@/stores/settings/persistent-store"
import { forcePush, testRemoteStatus } from "@/stores/test-branch"
import { conflictState } from "@/test-support/update-conflict"

/** A persistent store as the test creates it: its options and its zustand store */
interface CreatedStore {
  options: { schema: { parse: (value: unknown) => unknown }; isArray: boolean; isMainOnly: boolean }
  store: { getState: () => ArrayStoreState<unknown> }
}

const { created, selection, stored } = vi.hoisted(() => ({
  created: new Map<string, CreatedStore>(),
  selection: { path: "/a" },
  // what an earlier session persisted: the store starts from it
  stored: {
    activity: [{ id: "old", repositoryPath: "/a", time: Date.now() - 1000, kind: "push", title: "Pushed auth", description: "1 commit" }],
  } as Record<string, unknown>,
}))

vi.mock("@tauri-apps/plugin-log", () => ({ error: async () => {} }))
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: async () => {} }))
vi.mock("@/components/ui/toast", () => ({ toast: { add: () => "toast", close: () => {} } }))
// the store starts from what is stored, as a window starts from its snapshot
vi.mock("@/stores/settings/persistent-store", async () => {
  const { createStore } = await import("zustand/vanilla")
  return {
    createPersistentStore: (key: string, options: CreatedStore["options"]) => {
      const store = createStore(() => ({ items: options.schema.parse(stored[key] ?? []) as unknown[] }))
      created.set(key, { options, store })
      return store
    },
  }
})
vi.mock("@/stores/repository", () => ({
  getSelectedProject: () => ({ path: selection.path }),
  useSelectedProject: () => ({ path: selection.path }),
}))

function entries(repositoryPath = "/a"): ActivityEntry[] {
  return selectActivity(activityStore.getState(), repositoryPath)
}

function landedBranch(name: string, ids: string[], remaining = 0): LandedBranch {
  return {
    name,
    commits: ids.map(id => ({ id, subject: `Subject ${id}`, upstream: { id: `up-${id}`, author: "A", committer: "Patronus", time: 100 } })),
    remaining,
    landedAt: 1000,
  }
}

const ROBOT: PatronusRobot = {
  id: "r1",
  name: "Add login",
  status: "running",
  pushMode: "rebaseAutosquash",
  branchName: "auth",
  sourceRevision: "tip",
  targetBranch: "master",
  startTime: 1,
  finishTime: null,
  cancellationReason: null,
  url: "https://patronus.example.com/robots/r1",
}

describe("persistence", () => {
  it("starts from the stored log and parses what is stored or received", () => {
    // the stored entry of an earlier session, before the first record of this one
    const activity = created.get("activity")!
    expect(activity.store.getState().items).toEqual([expect.objectContaining({ id: "old", title: "Pushed auth" })])

    expect(activity.options.isArray).toBe(true)
    expect(activity.options.isMainOnly).toBe(true)
    const registration = activity.options.schema
    expect(registration.parse(undefined)).toEqual([])
    const landed: ActivityEntry = {
      id: "1",
      repositoryPath: "/a",
      time: 1,
      kind: "update",
      title: "auth landed in origin/master",
      details: { type: "landed", upstream: "origin/master", report: [landedBranch("auth", ["c1"])] },
    }
    const undone: ActivityEntry = { id: "2", repositoryPath: "/a", time: 2, kind: "rewrite", title: "Squashed 2 commits", undo: { until: 3, undone: true } }
    expect(registration.parse([landed, undone])).toEqual([landed, undone])
    // an entry of an unknown kind is invalid: the store starts empty then, as for any invalid stored value
    expect(() => registration.parse([{ ...undone, kind: "unknown" }])).toThrow()
    expect(() => registration.parse([{ ...landed, details: { type: "landed", upstream: "origin/master" } }])).toThrow()
  })
})

describe("activity", () => {
  beforeEach(() => {
    activityStore.setState({ items: [] })
    selection.path = "/a"
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it("keeps the log of each repository apart, newest first", () => {
    recordActivity({ kind: "push", title: "Pushed auth" })
    selection.path = "/b"
    recordActivity({ kind: "push", title: "Pushed search" })
    recordActivity({ repositoryPath: "/a", kind: "update", title: "Updated master: fast-forwarded to origin/master" })

    expect(entries("/a").map(entry => entry.title)).toEqual(["Updated master: fast-forwarded to origin/master", "Pushed auth"])
    expect(entries("/b").map(entry => entry.title)).toEqual(["Pushed search"])
    expect(new Set(activityStore.getState().items.map(entry => entry.id)).size).toBe(3)

    clearActivity("/a")
    expect(entries("/a")).toEqual([])
    expect(entries("/b")).toHaveLength(1)
  })

  it("keeps at most 200 entries and none older than 30 days", () => {
    vi.useFakeTimers({ now: 10 * MAX_ACTIVITY_AGE_MS })
    activityStore.setState({
      items: [
        { id: "a", repositoryPath: "/b", time: Date.now() - 1000, kind: "push", title: "recent" },
        { id: "b", repositoryPath: "/a", time: Date.now() - MAX_ACTIVITY_AGE_MS - 1, kind: "push", title: "too old" },
      ],
    })
    recordActivity({ kind: "push", title: "Pushed auth" })
    expect(activityStore.getState().items.map(entry => entry.title)).toEqual(["Pushed auth", "recent"])

    for (let i = 0; i < MAX_ACTIVITY_ENTRIES + 10; i++) {
      recordActivity({ kind: "push", title: `push ${i}` })
    }
    const items = activityStore.getState().items
    expect(items).toHaveLength(MAX_ACTIVITY_ENTRIES)
    expect(items[0]!.title).toBe(`push ${MAX_ACTIVITY_ENTRIES + 9}`)
  })

  it("records nothing without a repository", () => {
    selection.path = ""
    expect(recordActivity({ kind: "push", title: "Pushed auth" })).toBeNull()
    expect(activityStore.getState().items).toEqual([])
  })

  it("offers the Undo of a rewrite for the window of its toast, once", async () => {
    vi.useFakeTimers({ now: 1_000_000 })
    const undo = vi.fn(async () => true)
    const runFromToast = recordRewrite("Squashed 2 commits", undo)
    const entry = entries()[0]!
    expect(entry.undo).toEqual({ until: 1_000_000 + UNDO_WINDOW_MS })
    expect(canUndoActivity(entry)).toBe(true)

    undoActivity(entry)
    // the toast's Undo while the one of Activity runs undoes nothing more
    runFromToast()
    expect(undo).toHaveBeenCalledTimes(1)
    await vi.waitFor(() => expect(entries()[0]!.undo?.undone).toBe(true))
    expect(canUndoActivity(entries()[0]!)).toBe(false)
    runFromToast()
    expect(undo).toHaveBeenCalledTimes(1)

    recordRewrite("Moved 1 commit to search", undo)
    const moved = entries()[0]!
    vi.advanceTimersByTime(UNDO_WINDOW_MS)
    expect(canUndoActivity(moved)).toBe(false)
    undoActivity(moved)
    expect(undo).toHaveBeenCalledTimes(1)
    expect(entries()[0]!.undo?.undone).toBeUndefined()
  })

  it("marks a rewrite undone from its toast once its undo undid it", async () => {
    let finish!: (undone: boolean) => void
    const undo = vi.fn(() => new Promise<boolean>(resolve => (finish = resolve)))
    recordRewrite("Squashed 2 fixups", undo)()
    expect(undo).toHaveBeenCalledTimes(1)
    // the undo may wait for another rewrite in the queue: the entry is not undone before it ran
    expect(entries()[0]!.undo?.undone).toBeUndefined()
    finish(true)
    await vi.waitFor(() => expect(entries()[0]!.undo?.undone).toBe(true))
  })

  it("keeps the Undo of a rewrite whose undo failed, in its toast and in Activity: a second press runs it again", async () => {
    const undo = vi.fn(async () => false)
    const runFromToast = recordRewrite("Moved 1 commit to search", undo)
    runFromToast()
    await vi.waitFor(() => expect(undo).toHaveBeenCalledTimes(1))
    await Promise.resolve()
    const entry = entries()[0]!
    expect(entry.undo?.undone).toBeUndefined()
    expect(canUndoActivity(entry)).toBe(true)
    undo.mockResolvedValueOnce(true)
    undoActivity(entry)
    expect(undo).toHaveBeenCalledTimes(2)
    await vi.waitFor(() => expect(entries()[0]!.undo?.undone).toBe(true))
  })

  it("has no Undo for a rewrite of an earlier session", () => {
    activityStore.setState({
      items: [{ id: "x", repositoryPath: "/a", time: Date.now(), kind: "rewrite", title: "Squashed 2 commits", undo: { until: Date.now() + UNDO_WINDOW_MS } }],
    })
    expect(canUndoActivity(entries()[0]!)).toBe(false)
  })

  it("records the errors the user was told about", () => {
    notifyError("Couldn't push auth", { message: "rejected" })
    expect(entries()[0]).toMatchObject({ kind: "error", title: "Couldn't push auth", description: "rejected" })
  })

  it("keeps the operation a failed command names, for its lines in the log", () => {
    notifyError("Cannot push auth", { message: "rejected", kind: null, operation_id: "01K8ZQ3V6Y4M2N7P9R1S5T0W2X" })
    expect(entries()[0]).toMatchObject({ kind: "error", title: "Cannot push auth", operationId: "01K8ZQ3V6Y4M2N7P9R1S5T0W2X" })

    // an error raised outside a command names none
    notifyError("Cannot open the review", new Error("closed"))
    expect(entries()[0]!.title).toBe("Cannot open the review")
    expect(entries()[0]).not.toHaveProperty("operationId")
  })

  it("records a landed report once, whether the update or the sync after one brings it", () => {
    const update: UpdateProjectResult = {
      check: { branch: "master", upstream: "origin/master", head: "h", upstreamHead: "u", base: "b", state: { status: "clean", kind: "rebase" }, fetchedAt: null },
      applied: { kind: "rebase", newHead: "n", replayed: 2, droppedAlreadyUpstream: 1 },
      landed: [landedBranch("auth", ["c1"])],
    }
    recordUpdate("/a", update)
    expect(entries()[0]).toMatchObject({ kind: "update", title: "auth landed in origin/master", description: "Updated master: rebased 2 commits" })

    // the sync after the update sends the same report, then one with a commit an update elsewhere landed
    recordLanded("/a", [landedBranch("auth", ["c1"])], "origin/master")
    expect(entries()).toHaveLength(1)
    recordLanded("/a", [landedBranch("auth", ["c1"]), landedBranch("search", ["s1"], 1)], "origin/master")
    expect(entries()).toHaveLength(2)
    expect(entries()[0]).toMatchObject({ kind: "landed", title: "1 commit of search landed · 1 stays local", description: "Found by a sync" })
    expect(entries()[0]!.details).toEqual({ type: "landed", upstream: "origin/master", report: [landedBranch("search", ["s1"], 1)] })
  })

  it("records a refused update, not one with nothing to do", () => {
    const check = { branch: "master", upstream: "origin/master", head: "h", upstreamHead: "u", base: "b", fetchedAt: null }
    recordUpdate("/a", { check: { ...check, state: { status: "upToDate" } }, applied: null, landed: [] })
    expect(entries()).toEqual([])
    recordUpdate("/a", { check: { ...check, state: conflictState(["src/app.txt"], 1) }, applied: null, landed: [] })
    expect(entries()[0]).toMatchObject({
      kind: "update",
      title: "Didn't update master",
      description: "Conflicts in 1 file: src/app.txt. Nothing was changed.",
      warning: true,
    })
  })

  it("records a robot with its page", () => {
    recordRobot("Safe Push started", "auth → master", ROBOT)
    expect(entries()[0]).toMatchObject({ kind: "patronus", title: "Safe Push started", description: "auth → master", details: { type: "robot", url: ROBOT.url } })
    recordRobot("auth merged into master", "Safe Push", ROBOT, "/b")
    expect(entries("/b")[0]).toMatchObject({ title: "auth merged into master", description: "Safe Push" })
  })
})

describe("recordPush", () => {
  it("keeps the remote commits a force push dropped", () => {
    const dropped = [{ id: "c0ffee", subject: "Fix login", author: "Ann", authorEmail: "ann@example.com", authorTime: 1 }]
    recordPush("/a", "auth", testRemoteStatus({ commitsAhead: 1, commitsBehind: 3, fastForward: false, droppedCommits: dropped, action: forcePush({ drops: true }) }), null)
    expect(entries()[0]).toMatchObject({
      kind: "push",
      title: "Force pushed auth",
      description: "1 commit · dropped 3 remote commits",
      warning: true,
      details: { type: "dropped", commits: dropped, more: 2 },
    })
    // a rebased branch replaces nothing of others
    recordPush("/a", "auth", testRemoteStatus({ commitsAhead: 2, fastForward: false, action: forcePush() }), null)
    expect(entries()[0]).toMatchObject({ title: "Force pushed auth", description: "2 commits · replaced the rebased copies" })
    expect(entries()[0]?.details).toBeUndefined()
  })
})

describe("groupActivityByDay", () => {
  it("groups by local day: today, yesterday, then the date", () => {
    const now = new Date(2026, 9, 14, 9, 0).getTime()
    const at = (day: number, hour: number) =>
      ({ id: `${day}-${hour}`, repositoryPath: "/a", time: new Date(2026, 9, day, hour).getTime(), kind: "push", title: "Pushed auth" }) satisfies ActivityEntry
    const days = groupActivityByDay([at(14, 8), at(14, 0), at(13, 23), at(10, 12)], now)
    expect(days.map(day => [day.label, day.entries.map(entry => entry.id)])).toEqual([
      ["Today", ["14-8", "14-0"]],
      ["Yesterday", ["13-23"]],
      [new Intl.DateTimeFormat(undefined, { dateStyle: "medium" }).format(new Date(2026, 9, 10)), ["10-12"]],
    ])
  })
})
