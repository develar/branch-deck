import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"
import type { GetPatronusRobotsParams, PatronusCommandReply, PatronusRobot, PatronusRobotsResult, PollPatronusRobotsParams, PostPatronusCommandParams } from "@/lib/bindings"
import {
  BACKGROUND_POLL_INTERVAL_MS,
  COMMAND_REPLY_INTERVAL_MS,
  COMMAND_REPLY_TIMEOUT_MS,
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
  startPatronusFromPullRequest,
} from "@/stores/patronus"

const { patronus, branches } = vi.hoisted(() => ({
  patronus: {
    lists: [] as GetPatronusRobotsParams[],
    polls: [] as PollPatronusRobotsParams[],
    list: null as PatronusRobotsResult | null,
    poll: null as PatronusRobotsResult | null,
    problemCalls: [] as string[],
    commands: [] as PostPatronusCommandParams[],
    replies: [] as PatronusCommandReply[],
    replyCalls: 0,
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
    postPatronusCommand: async (params: PostPatronusCommandParams) => {
      patronus.commands.push(params)
      return { status: "ok", data: { id: "4402154133", url: "https://github.com/acme/monorepo/pull/7#issuecomment-4402154133" } }
    },
    readPatronusCommandReply: async () => {
      patronus.replyCalls++
      return { status: "ok", data: patronus.replies.shift() ?? { status: "pending" } }
    },
    getPatronusRobotProblems: async (params: { robotId: string }) => {
      patronus.problemCalls.push(params.robotId)
      return { status: "ok", data: [{ title: "Changes require review approval", subsystem: "OWNERSHIP_REVIEW_APPROVAL_CHECK" }] }
    },
  },
}))

const { notices } = vi.hoisted(() => ({
  notices: {
    toasts: [] as { title: string, description?: string, actionProps?: { children: string, onClick: () => void }, data?: { kind: string, openRobot: () => void } }[],
    notifications: [] as { title: string, body: string }[],
    permissionRequests: 0,
    fetches: [] as string[],
    opened: [] as string[],
    pullRequestRefreshes: [] as string[],
    /** The pull requests GitHub lists, by branch */
    pullRequests: {} as Record<string, { number: number, state: string, headRefOid: string }>,
    /** "<branch> <comment>" of each `gh pr close` */
    closed: [] as string[],
  },
}))

vi.mock("@/stores/pull-requests", () => ({
  refreshPullRequests: async (reason: string) => {
    notices.pullRequestRefreshes.push(reason)
  },
  getPullRequest: (name: string) => notices.pullRequests[name],
  closePullRequest: async (name: string, comment: string) => {
    notices.closed.push(`${name} ${comment}`)
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

vi.mock("@tauri-apps/plugin-opener", () => ({
  openUrl: async (url: string) => {
    notices.opened.push(url)
  },
}))

vi.mock("@/stores/repository", () => import("@/test-support/selected-repository-mock"))
vi.mock("@/stores/activity", async () => (await import("@/stores/test-store-mocks")).activityMock)

vi.mock("@/stores/branch-sync", () => ({
  branchSyncStore: { getState: () => ({ branches: branches.current }) },
  getBranch: (name: string) => branches.current[name],
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
  patronus.commands = []
  patronus.replies = []
  patronus.replyCalls = 0
  notices.pullRequestRefreshes = []
  notices.toasts = []
  notices.notifications = []
  notices.permissionRequests = 0
  notices.fetches = []
  notices.opened = []
  notices.pullRequests = {}
  notices.closed = []
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
  it("tells once in a toast with Fetch and the robot page when a running robot of a branch merged", async () => {
    patronus.list = available(robot())
    await refreshPatronus("sync")
    patronus.poll = available(robot({ status: "success", finishTime: 200 }))
    await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS)
    expect(notices.toasts.map(it => [it.title, it.description, it.actionProps?.children, it.data?.kind])).toEqual([["auth merged into master", "Safe Push", "Fetch", "robot"]])
    expect(notices.notifications).toEqual([])
    notices.toasts[0]?.data?.openRobot()
    expect(notices.opened).toEqual(["u"])
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

  it("closes the pull requests of the ancestors a stacked branch landed, and says so", async () => {
    const stacked = (name: string, parent: string | null) => ({ name, commits: [{ hash: `${name}-tip` }], remoteStatus: null, stack: { effectiveBase: parent } })
    branches.current = { auth: stacked("auth", null), login: stacked("login", "auth"), oauth: stacked("oauth", "login") }
    notices.pullRequests = {
      auth: { number: 5, state: "open", headRefOid: "auth-tip" },
      // pushed before a later commit of login: the merge doesn't have that pull request's head
      login: { number: 6, state: "open", headRefOid: "older" },
    }
    const oauth = robot({ branchName: "oauth", sourceRevision: "oauth-tip", pullRequest: { number: 7, url: "u7" } })
    patronus.list = available(oauth)
    await refreshPatronus("sync")
    patronus.poll = available({ ...oauth, status: "success", finishTime: 200 })
    await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS)
    expect(notices.closed).toEqual(["auth Landed in master with #7, the Safe Push of user/virtual/oauth, which is built on this branch."])
    expect(notices.toasts.map(it => it.description)).toEqual(["Safe Push · closes the pull request of auth: it landed with oauth"])
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

describe("pull request start", () => {
  const ofPullRequest = robot({ id: "pr", startTime: START / 1000, pullRequest: { number: 7, url: "https://github.com/acme/monorepo/pull/7" } })

  it("posts the command, waits for the thumbs up and returns the robot of the pull request", async () => {
    patronus.replies = [{ status: "pending" }, { status: "accepted" }]
    patronus.list = available(ofPullRequest)
    const started = startPatronusFromPullRequest(7, "rebaseAutosquash")
    await vi.advanceTimersByTimeAsync(2 * COMMAND_REPLY_INTERVAL_MS)
    expect(await started).toEqual(ofPullRequest)
    expect(patronus.commands).toEqual([{ repositoryPath: "/repo", pullRequestNumber: 7, mode: "rebaseAutosquash" }])
    expect(patronus.replyCalls).toBe(2)
    // an incremental list from shortly before the comment
    expect(patronus.lists).toEqual([{ repositoryPath: "/repo", branchPrefix: "user", since: Math.floor((START - LIST_OVERLAP_MS) / 1000), background: false }])
    expect(getBranchRobot("auth")?.id).toBe("pr")
  })

  it("throws the reason the bot gave with its thumbs down", async () => {
    patronus.replies = [{ status: "rejected", reason: null }, { status: "rejected", reason: "Pull request #7 is a draft and cannot be merged." }]
    const started = startPatronusFromPullRequest(7, "rebaseAutosquash")
    const failed = expect(started).rejects.toThrow("Pull request #7 is a draft and cannot be merged.")
    await vi.advanceTimersByTimeAsync(2 * COMMAND_REPLY_INTERVAL_MS)
    await failed
    expect(patronus.lists).toEqual([])
  })

  it("gives up after a minute without an answer", async () => {
    const started = startPatronusFromPullRequest(7, "dryRun")
    const failed = expect(started).rejects.toThrow("Patronus did not start a robot within a minute: see pull request #7")
    await vi.advanceTimersByTimeAsync(COMMAND_REPLY_TIMEOUT_MS + COMMAND_REPLY_INTERVAL_MS)
    await failed
  })

  it("keeps the GitHub repository of a list through a poll, and refreshes the pull requests when a robot of one merged", async () => {
    patronus.list = { ...available(ofPullRequest), githubRepository: "acme/monorepo" }
    await refreshPatronus("sync")
    expect(patronusStore.getState().repositories["/repo\nuser"]?.githubRepository).toBe("acme/monorepo")
    patronus.poll = { ...available({ ...ofPullRequest, status: "success", finishTime: 200 }), githubRepository: null }
    await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS)
    expect(patronusStore.getState().repositories["/repo\nuser"]?.githubRepository).toBe("acme/monorepo")
    expect(notices.pullRequestRefreshes).toEqual(["push"])
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

  it("keeps the merge profile a poll, the details or the start named when a list, which never names it, answers the robot", () => {
    const profile = { id: "lsc", name: "Large Scale Change" }
    const polled = robot({ profile })
    const previous = { r1: polled }
    expect(mergeRobots(previous, [robot()], true)).toBe(previous)
    expect(mergeRobots(previous, [robot({ status: "failure" })], false).r1?.profile).toEqual(profile)
    // an answer that names another profile wins
    expect(mergeRobots(previous, [robot({ profile: { id: null, name: null } })], false).r1?.profile).toEqual({ id: null, name: null })
  })
})
