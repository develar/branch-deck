// @spec spec/repository/activity.spec.md
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type { JournalEntry, LandedBranch, PatronusRobot, UpdateProjectResult } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import type { ActivityEntry } from "@/stores/activity"
import {
  activityStore,
  canUndoActivity,
  clearActivity,
  groupActivityByDay,
  recordActivity,
  recordLanded,
  recordRewrite,
  recordRobotNews,
  recordUpdate,
  refreshActivity,
  selectActivity,
  selectUnavailableJournals,
  undoActivity,
  UNDO_WINDOW_MS,
  withUndo,
} from "@/stores/activity"
import { fakeActivity } from "@/test-support/fake-activity"
import { conflictState } from "@/test-support/update-conflict"

const { selection } = vi.hoisted(() => ({ selection: { path: "/a" } }))

vi.mock("@tauri-apps/plugin-log", () => ({ error: async () => {} }))
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: async () => {} }))
vi.mock("@/components/ui/toast", () => ({ toast: { add: () => "toast", close: () => {} } }))
vi.mock("@/stores/repository", () => ({
  getSelectedProject: () => ({ path: selection.path }),
  useSelectedProject: () => ({ path: selection.path }),
}))
vi.mock("@/lib/bindings", async importOriginal => {
  const original = await importOriginal<typeof import("@/lib/bindings")>()
  return { ...original, commands: { ...original.commands, ...(await import("@/test-support/fake-activity")).fakeActivity.commands } }
})

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

const ROBOT = {
  id: "r1",
  status: "failed",
  pushMode: "rebaseAutosquash",
  branchName: "auth",
  targetBranch: "master",
  url: "https://patronus.example.com/robots/r1",
} as unknown as PatronusRobot

/** The journal entry of a push of `branch` that ended at `endedAt`, in the worktree `/a` */
function pushOf(id: string, branch: string, endedAt: number): JournalEntry {
  return {
    id,
    tool: "bd",
    pid: 1,
    worktree: "/a",
    prefix: "user",
    leaseSlot: false,
    intent: { kind: "push", subject: { kind: "branch", name: branch }, scope: { kind: "alone" }, expectRemote: null, force: false, restartRunning: false },
    subject: branch,
    startedAt: endedAt,
    endedAt,
    effects: [{ effect: { kind: "push", branches: [branch], force: false }, status: { kind: "done" }, remote: [null] }],
    result: { kind: "none" },
    error: null,
    stats: { gitProcesses: {}, httpRequests: 0, millis: 1 },
    transition: null,
  }
}

describe("activity", () => {
  beforeEach(async () => {
    fakeActivity.reset()
    selection.path = "/a"
    await refreshActivity()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it("reads the journals of the repositories and the page's, each repository's log apart, newest first", async () => {
    const now = Math.floor(Date.now() / 1000)
    fakeActivity.setJournals([{ path: "/a/.git/branch-deck/journal.jsonl", repositories: ["/a"], available: true, entries: [pushOf("op1", "auth", now - 10)] }])
    await recordActivity("/a", { kind: "update", title: "Updated master: fast-forwarded to origin/master" })
    await recordActivity("/b", { kind: "update", title: "Updated master: rebased 1 commit" })
    // the page's entries show at once, the journal's with the next read
    expect(entries("/a").map(entry => entry.title)).toEqual(["Updated master: fast-forwarded to origin/master"])
    await refreshActivity()
    expect(entries("/a").map(entry => entry.title)).toEqual(["Updated master: fast-forwarded to origin/master", "Pushed branch auth"])
    expect(entries("/b").map(entry => entry.title)).toEqual(["Updated master: rebased 1 commit"])
    expect(fakeActivity.page.map(entry => entry.activity.repositoryPath)).toEqual(["/a", "/b"])

    expect(await clearActivity("/a")).toBeNull()
    expect(entries("/a")).toEqual([])
    expect(entries("/b")).toHaveLength(1)
  })

  it("records nothing without a repository", async () => {
    expect(await recordActivity("", { kind: "update", title: "Updated master" })).toBeNull()
    expect(fakeActivity.page).toEqual([])
  })

  it("lists the journals of a repository its read could not read, and none of another repository", async () => {
    fakeActivity.setJournals([
      { path: "/a/.git/branch-deck/journal.jsonl", repositories: ["/a", "/a-feature"], available: false, entries: [] },
      { path: "/b/.git/branch-deck/journal.jsonl", repositories: ["/b"], available: true, entries: [] },
    ])
    await refreshActivity()
    const state = activityStore.getState()
    expect(selectUnavailableJournals(state, "/a-feature")).toEqual(["/a/.git/branch-deck/journal.jsonl"])
    expect(selectUnavailableJournals(state, "/b")).toEqual([])
    expect(selectUnavailableJournals(state, undefined)).toEqual([])
  })

  it("gives the Activity window the Undo of the main window's rewrites by entry id", () => {
    const [kept, rewritten] = [
      { id: "e1", repositoryPath: "/a", time: 0, kind: "push", title: "Pushed branch auth" },
      { id: "e2", repositoryPath: "/a", time: 0, kind: "rewrite", title: "Squashed 2 commits" },
    ] as const
    expect(withUndo([kept, rewritten], { e2: { until: 10, undone: true } })).toEqual([kept, { ...rewritten, undo: { until: 10, undone: true } }])
  })

  it("offers the Undo of a rewrite for the window of its toast, once", async () => {
    vi.useFakeTimers({ now: 1_000_000 })
    const undo = vi.fn(async () => true)
    const runFromToast = recordRewrite("/a", "Squashed 2 commits", undo)
    await vi.waitFor(() => expect(entries()[0]?.undo).toBeDefined())
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

    recordRewrite("/a", "Moved 1 commit to search", undo)
    await vi.waitFor(() => expect(entries()[0]?.title).toBe("Moved 1 commit to search"))
    await vi.waitFor(() => expect(entries()[0]?.undo).toBeDefined())
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
    recordRewrite("/a", "Squashed 2 fixups", undo)()
    expect(undo).toHaveBeenCalledTimes(1)
    await vi.waitFor(() => expect(entries()[0]?.undo).toBeDefined())
    // the undo may wait for another rewrite in the queue: the entry is not undone before it ran
    expect(entries()[0]!.undo?.undone).toBeUndefined()
    finish(true)
    await vi.waitFor(() => expect(entries()[0]!.undo?.undone).toBe(true))
  })

  it("keeps the Undo of a rewrite whose undo failed, in its toast and in Activity: a second press runs it again", async () => {
    const undo = vi.fn(async () => false)
    const runFromToast = recordRewrite("/a", "Moved 1 commit to search", undo)
    await vi.waitFor(() => expect(entries()[0]?.undo).toBeDefined())
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

  it("has no Undo for a rewrite of an earlier session", async () => {
    // the journal of the page remembers the rewrite, not its undo
    await recordActivity("/a", { kind: "rewrite", title: "Squashed 2 commits" })
    expect(entries()[0]!.undo).toBeUndefined()
    expect(canUndoActivity(entries()[0]!)).toBe(false)
  })

  it("records the errors the user was told about, in the repository the main window shows", async () => {
    selection.path = "/b"
    notifyError("Couldn't push auth", { message: "rejected" })
    await vi.waitFor(() => expect(entries("/b")[0]).toMatchObject({ kind: "error", title: "Couldn't push auth", description: "rejected" }))
  })

  it("keeps the operation a failed command names, for its lines in the log", async () => {
    notifyError("Cannot push branch auth", { message: "rejected", kind: null, operation_id: "01K8ZQ3V6Y4M2N7P9R1S5T0W2X" })
    await vi.waitFor(() => expect(entries()[0]).toMatchObject({ kind: "error", title: "Cannot push branch auth", operationId: "01K8ZQ3V6Y4M2N7P9R1S5T0W2X" }))

    // an error raised outside a command names none
    notifyError("Cannot open the review", new Error("closed"))
    await vi.waitFor(() => expect(entries()[0]!.title).toBe("Cannot open the review"))
    expect(entries()[0]).not.toHaveProperty("operationId")
  })

  it("records a landed report once, whether the update or the sync after one brings it", async () => {
    const update: UpdateProjectResult = {
      check: { branch: "master", upstream: "origin/master", head: "h", upstreamHead: "u", base: "b", state: { status: "clean", kind: "rebase" }, fetchedAt: null },
      applied: { kind: "rebase", newHead: "n", replayed: 2, droppedAlreadyUpstream: 1 },
      landed: [landedBranch("auth", ["c1"])],
    }
    recordUpdate("/a", update)
    // the sync after the update sends the same report, also before the record of the update answered
    recordLanded("/a", [landedBranch("auth", ["c1"])], "origin/master")
    await vi.waitFor(() => expect(entries()[0]).toMatchObject({ kind: "update", title: "auth landed in origin/master", description: "Updated master: rebased 2 commits" }))

    // then one with a commit an update elsewhere landed
    recordLanded("/a", [landedBranch("auth", ["c1"])], "origin/master")
    recordLanded("/a", [landedBranch("auth", ["c1"]), landedBranch("search", ["s1"], 1)], "origin/master")
    await vi.waitFor(() => expect(entries()).toHaveLength(2))
    expect(entries()[0]).toMatchObject({ kind: "landed", title: "1 commit of search landed · 1 stays local", description: "Found by a sync" })
    expect(entries()[0]!.details).toEqual({ type: "landed", upstream: "origin/master", report: [landedBranch("search", ["s1"], 1)] })
  })

  it("records a refused update, not one with nothing to do", async () => {
    const check = { branch: "master", upstream: "origin/master", head: "h", upstreamHead: "u", base: "b", fetchedAt: null }
    recordUpdate("/a", { check: { ...check, state: { status: "upToDate" } }, applied: null, landed: [] })
    recordUpdate("/a", { check: { ...check, state: conflictState(["src/app.txt"], 1) }, applied: null, landed: [] })
    await vi.waitFor(() => expect(entries()).toHaveLength(1))
    expect(entries()[0]).toMatchObject({
      kind: "update",
      title: "Didn't update master",
      description: "Conflicts in 1 file: src/app.txt. Nothing was changed.",
      warning: true,
    })
  })

  it("records the news of a robot with its page", async () => {
    recordRobotNews("/b", "Safe Push of auth failed", "Fast Tests · auth → master", ROBOT)
    await vi.waitFor(() =>
      expect(entries("/b")[0]).toMatchObject({
        kind: "patronus",
        title: "Safe Push of auth failed",
        description: "Fast Tests · auth → master",
        details: { type: "robot", url: ROBOT.url },
      }),
    )
  })
})

describe("groupActivityByDay", () => {
  it("groups by local day: today, yesterday, then the date", () => {
    const now = new Date(2026, 9, 14, 9, 0).getTime()
    const at = (day: number, hour: number) =>
      ({ id: `${day}-${hour}`, repositoryPath: "/a", time: new Date(2026, 9, day, hour).getTime(), kind: "push", title: "Pushed branch auth" }) satisfies ActivityEntry
    const days = groupActivityByDay([at(14, 8), at(14, 0), at(13, 23), at(10, 12)], now)
    expect(days.map(day => [day.label, day.entries.map(entry => entry.id)])).toEqual([
      ["Today", ["14-8", "14-0"]],
      ["Yesterday", ["13-23"]],
      [new Intl.DateTimeFormat(undefined, { dateStyle: "medium" }).format(new Date(2026, 9, 10)), ["10-12"]],
    ])
  })
})
