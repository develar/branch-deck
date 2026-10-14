import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"
import type { GetPatronusRobotsParams, PatronusRobot, PatronusRobotsResult, PollPatronusRobotsParams } from "@/lib/bindings"
import {
  BACKGROUND_POLL_INTERVAL_MS,
  FOCUS_REFRESH_AGE_MS,
  LIST_OVERLAP_MS,
  MIN_FOCUS_GAP_MS,
  POLL_INTERVAL_MS,
  SYNC_REFRESH_GAP_MS,
  getBranchRobot,
  mergeRobots,
  patronusStore,
  refreshPatronus,
  resetPatronus,
  setPatronusWindowFocus,
} from "@/stores/patronus"

const { patronus, branches } = vi.hoisted(() => ({
  patronus: {
    lists: [] as GetPatronusRobotsParams[],
    polls: [] as PollPatronusRobotsParams[],
    list: null as PatronusRobotsResult | null,
    poll: null as PatronusRobotsResult | null,
    problemCalls: [] as string[],
  },
  branches: { current: {} as Record<string, unknown> },
}))

vi.mock("@/lib/bindings", () => ({
  commands: {
    getPatronusRobots: async (params: GetPatronusRobotsParams) => {
      patronus.lists.push(params)
      return { status: "ok", data: patronus.list }
    },
    pollPatronusRobots: async (params: PollPatronusRobotsParams) => {
      patronus.polls.push(params)
      return { status: "ok", data: patronus.poll }
    },
    getPatronusRobotProblems: async (params: { robotId: string }) => {
      patronus.problemCalls.push(params.robotId)
      return { status: "ok", data: [{ title: "Changes require review approval", subsystem: "OWNERSHIP_REVIEW_APPROVAL_CHECK" }] }
    },
  },
}))

const { notices } = vi.hoisted(() => ({
  notices: {
    toasts: [] as { title: string, description?: string, actionProps?: { children: string, onClick: () => void } }[],
    notifications: [] as { title: string, body: string }[],
    permissionRequests: 0,
    fetches: [] as string[],
  },
}))

vi.mock("@/components/ui/toast", () => ({
  toast: {
    add: (options: (typeof notices.toasts)[number]) => {
      notices.toasts.push(options)
      return "toast"
    },
    close: () => {},
  },
}))

vi.mock("@tauri-apps/plugin-notification", () => ({
  isPermissionGranted: async () => false,
  requestPermission: async () => {
    notices.permissionRequests++
    return "granted"
  },
  sendNotification: (options: { title: string, body: string }) => notices.notifications.push(options),
}))

vi.mock("@/stores/update-project", () => ({
  runFetchBaseline: async (repositoryPath: string) => {
    notices.fetches.push(repositoryPath)
  },
}))

vi.mock("@/stores/repository", () => import("@/test-support/selected-repository-mock"))

vi.mock("@/stores/branch-sync", () => ({
  branchSyncStore: { getState: () => ({ branches: branches.current }) },
  useBranchSync: () => undefined,
  useBranchTip: () => undefined,
  useBranchRemoteTip: () => null,
}))

function robot(patch: Partial<PatronusRobot> = {}): PatronusRobot {
  return {
    id: "r1",
    name: "n",
    status: "running",
    pushMode: "rebaseAutosquash",
    branchName: "auth",
    sourceRevision: "tip",
    targetBranch: "master",
    startTime: 100,
    finishTime: null,
    cancellationReason: null,
    url: "u",
    ...patch,
  }
}

function available(...robots: PatronusRobot[]): PatronusRobotsResult {
  return { availability: { status: "available" }, robots }
}

function setBranches(...names: string[]): void {
  branches.current = Object.fromEntries(names.map(name => [name, { name, commits: [{ hash: "tip" }], remoteStatus: null }]))
}

const START = 1_000_000_000

beforeEach(() => {
  vi.useFakeTimers()
  vi.setSystemTime(START)
  resetPatronus()
  patronus.lists = []
  patronus.polls = []
  patronus.problemCalls = []
  notices.toasts = []
  notices.notifications = []
  notices.permissionRequests = 0
  notices.fetches = []
  patronus.list = available(robot({ status: "failure" }))
  patronus.poll = available()
  setBranches("auth")
})

afterEach(() => {
  vi.useRealTimers()
})

describe("refreshPatronus", () => {
  it("lists the last days first, then only the robots started since shortly before the last list", async () => {
    await refreshPatronus("sync")
    expect(patronus.lists).toEqual([{ repositoryPath: "/repo", branchPrefix: "user", since: null, background: true }])
    expect(getBranchRobot("auth")?.status).toBe("failure")

    vi.advanceTimersByTime(SYNC_REFRESH_GAP_MS)
    await refreshPatronus("sync")
    expect(patronus.lists[1]?.since).toBe(Math.floor((START - LIST_OVERLAP_MS) / 1000))
  })

  it("asks after a sync every five minutes at most, on focus when older than two minutes, never after a push", async () => {
    await refreshPatronus("sync")
    vi.advanceTimersByTime(SYNC_REFRESH_GAP_MS - 1)
    await refreshPatronus("sync")
    await refreshPatronus("push")
    expect(patronus.lists).toHaveLength(1)

    vi.setSystemTime(START + MIN_FOCUS_GAP_MS)
    await refreshPatronus("focus")
    expect(patronus.lists).toHaveLength(1)
    vi.setSystemTime(START + FOCUS_REFRESH_AGE_MS)
    await refreshPatronus("focus")
    expect(patronus.lists).toHaveLength(2)
  })

  it("lists the full window on the refresh button, asked by the user", async () => {
    await refreshPatronus("sync")
    await refreshPatronus("manual")
    expect(patronus.lists[1]).toMatchObject({ since: null, background: false })
  })

  it("does not ask again for a repository that is not on Space, until the refresh button", async () => {
    patronus.list = { availability: { status: "notPatronus" }, robots: [] }
    await refreshPatronus("sync")
    vi.advanceTimersByTime(SYNC_REFRESH_GAP_MS + FOCUS_REFRESH_AGE_MS)
    await refreshPatronus("sync")
    await refreshPatronus("focus")
    expect(patronus.lists).toHaveLength(1)
    await refreshPatronus("manual")
    expect(patronus.lists).toHaveLength(2)
  })

  it("keeps the shown robots when Patronus can't answer", async () => {
    await refreshPatronus("sync")
    patronus.list = { availability: { status: "notLoggedIn" }, robots: [] }
    await refreshPatronus("manual")
    const entry = patronusStore.getState().repositories["/repo\nuser"]!
    expect(entry.availability).toEqual({ status: "notLoggedIn" })
    expect(entry.robots.r1?.status).toBe("failure")
  })
})

describe("polling", () => {
  it("polls the running robots of the branches every minute while the window is focused", async () => {
    patronus.list = available(robot(), robot({ id: "other-branch", branchName: "gone" }))
    await refreshPatronus("sync")
    expect(patronus.polls).toHaveLength(0)

    await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS)
    expect(patronus.polls).toEqual([{ repositoryPath: "/repo", branchPrefix: "user", robots: [{ id: "r1", status: "running" }] }])

    patronus.poll = available(robot({ status: "success", finishTime: 200 }))
    await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS)
    expect(patronus.polls).toHaveLength(2)
    expect(getBranchRobot("auth")?.status).toBe("success")

    // nothing runs any more: no timer
    await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS * 3)
    expect(patronus.polls).toHaveLength(2)
  })

  it("stops on blur and asks at once on focus", async () => {
    patronus.list = available(robot())
    await refreshPatronus("sync")
    setPatronusWindowFocus(false)
    await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS * 3)
    expect(patronus.polls).toHaveLength(0)

    setPatronusWindowFocus(true)
    await vi.advanceTimersByTimeAsync(0)
    expect(patronus.polls).toHaveLength(1)
    // a quick blur and focus asks nothing more
    setPatronusWindowFocus(false)
    setPatronusWindowFocus(true)
    await vi.advanceTimersByTimeAsync(0)
    expect(patronus.polls).toHaveLength(1)
  })

  it("polls a robot that pushes every five minutes while the window is not focused, a dry run not at all", async () => {
    patronus.list = available(robot())
    await refreshPatronus("sync")
    setPatronusWindowFocus(false)
    await vi.advanceTimersByTimeAsync(BACKGROUND_POLL_INTERVAL_MS - 1)
    expect(patronus.polls).toHaveLength(0)
    await vi.advanceTimersByTimeAsync(1)
    expect(patronus.polls).toHaveLength(1)
    await vi.advanceTimersByTimeAsync(BACKGROUND_POLL_INTERVAL_MS)
    expect(patronus.polls).toHaveLength(2)

    resetPatronus()
    patronus.polls = []
    patronus.list = available(robot({ pushMode: "dryRun" }))
    await refreshPatronus("sync")
    setPatronusWindowFocus(false)
    await vi.advanceTimersByTimeAsync(BACKGROUND_POLL_INTERVAL_MS * 3)
    expect(patronus.polls).toHaveLength(0)
  })

  it("polls nothing without running robots", async () => {
    await refreshPatronus("sync")
    setPatronusWindowFocus(true)
    await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS * 3)
    expect(patronus.polls).toHaveLength(0)
  })
})

describe("merges", () => {
  it("tells once in a toast with Fetch when a running robot of a branch merged", async () => {
    patronus.list = available(robot())
    await refreshPatronus("sync")
    patronus.poll = available(robot({ status: "success", finishTime: 200 }))
    await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS)
    expect(notices.toasts.map(it => [it.title, it.description, it.actionProps?.children])).toEqual([["auth merged into master", "Safe Push", "Fetch"]])
    expect(notices.notifications).toEqual([])
    notices.toasts[0]?.actionProps?.onClick()
    expect(notices.fetches).toEqual(["/repo"])

    // the same robot in a later answer is no news
    await refreshPatronus("manual")
    expect(notices.toasts).toHaveLength(1)
  })

  it("tells a merge in the background by an OS notification, and in a toast on focus", async () => {
    patronus.list = available(robot())
    await refreshPatronus("sync")
    setPatronusWindowFocus(false)
    patronus.poll = available(robot({ status: "success", finishTime: 200 }))
    await vi.advanceTimersByTimeAsync(BACKGROUND_POLL_INTERVAL_MS)
    expect(notices.notifications).toEqual([{ title: "auth merged into master", body: "Safe Push · Fetch and update to pick it up" }])
    expect(notices.permissionRequests).toBe(1)
    expect(notices.toasts).toEqual([])

    setPatronusWindowFocus(true)
    expect(notices.toasts.map(it => it.title)).toEqual(["auth merged into master"])
  })

  it("does not tell a successful dry run or a robot first seen merged", async () => {
    patronus.list = available(robot({ pushMode: "dryRun" }), robot({ id: "r2", branchName: "other", status: "success" }))
    setBranches("auth", "other")
    await refreshPatronus("sync")
    patronus.poll = available(robot({ pushMode: "dryRun", status: "success" }))
    await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS)
    expect(getBranchRobot("auth")?.status).toBe("success")
    expect(notices.toasts).toEqual([])
  })
})

describe("review problems", () => {
  it("reads the problems of a failed robot of a branch once", async () => {
    await refreshPatronus("sync")
    await vi.advanceTimersByTimeAsync(0)
    expect(patronus.problemCalls).toEqual(["r1"])
    expect(patronusStore.getState().repositories["/repo\nuser"]?.needsReview).toEqual({ r1: true })
    await refreshPatronus("manual")
    await vi.advanceTimersByTimeAsync(0)
    expect(patronus.problemCalls).toEqual(["r1"])
  })
})

describe("mergeRobots", () => {
  it("keeps the objects of unchanged robots and the record when nothing changed", () => {
    const first = robot()
    const previous = { r1: first }
    expect(mergeRobots(previous, [robot()], true)).toBe(previous)
    const changed = mergeRobots(previous, [robot(), robot({ id: "r2" })], false)
    expect(changed).not.toBe(previous)
    expect(changed.r1).toBe(first)
    // a full list drops the robots it does not have
    expect(Object.keys(mergeRobots(changed, [robot({ id: "r2" })], true))).toEqual(["r2"])
  })
})
