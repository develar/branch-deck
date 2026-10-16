import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"
import type { GetPatronusRobotsParams, PatronusCommandReply, PatronusRobot, PatronusRobotsResult, PollPatronusRobotsParams, PostPatronusCommandParams } from "@/lib/bindings"
import {
  BACKGROUND_POLL_INTERVAL_MS,
  COMMAND_REPLY_INTERVAL_MS,
  COMMAND_REPLY_TIMEOUT_MS,
  REVIEWED_ROBOT_WAIT_MS,
  FOCUS_REFRESH_AGE_MS,
  LIST_OVERLAP_MS,
  MIN_FOCUS_GAP_MS,
  POLL_INTERVAL_MS,
  SYNC_REFRESH_GAP_MS,
  getBranchRobot,
  markRobotFailing,
  mergeRobots,
  patronusStore,
  refreshPatronus,
  resetPatronus,
  setPatronusWindowFocus,
  startPatronusFromPullRequest,
  startPatronusFromReviewedPullRequest,
  watchRobot,
} from "@/stores/patronus"
import { FAILURE_PROBLEMS_WAIT_MS, resetFailingAnnouncements, watchFailingRobots } from "@/lib/patronus-failing"

const { patronus, branches } = vi.hoisted(() => ({
  patronus: {
    lists: [] as GetPatronusRobotsParams[],
    polls: [] as PollPatronusRobotsParams[],
    list: null as PatronusRobotsResult | null,
    poll: null as PatronusRobotsResult | null,
    problemCalls: [] as string[],
    /** What the problems of a robot are: the review approval check unless a test says otherwise; "offline" for an error */
    problems: null as { title: string, subsystem: string }[] | "offline" | null,
    /** The robot ids of each ask whether robots will fail */
    failingCalls: [] as string[][],
    failing: [] as string[],
    /** What failed (`RobotFailureKind`), which decides the step of the failed toast */
    failureKind: "checks" as string,
    detailCalls: [] as string[],
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
    getPatronusFailingRobots: async (params: { robotIds: string[] }) => {
      patronus.failingCalls.push(params.robotIds)
      return { status: "ok", data: params.robotIds.filter(id => patronus.failing.includes(id)) }
    },
    getPatronusRobotDetails: async (params: { robotId: string }) => {
      patronus.detailCalls.push(params.robotId)
      return { status: "ok", data: { checks: { failed: 1, failedNames: ["Fast Tests"], running: 2, passed: 3, canceled: 0 } } }
    },
    getRobotFailure: async () => ({ status: "ok", data: { failure: { kind: patronus.failureKind }, commits: null, headBranch: "master" } }),
    getPatronusRobotProblems: async (params: { robotId: string }) => {
      patronus.problemCalls.push(params.robotId)
      if (patronus.problems === "offline") {
        return { status: "error", error: "offline" }
      }
      return { status: "ok", data: patronus.problems ?? [{ title: "Changes require review approval", subsystem: "OWNERSHIP_REVIEW_APPROVAL_CHECK" }] }
    },
  },
}))

const { notices } = vi.hoisted(() => ({
  notices: {
    toasts: [] as { title: string, description?: string, actionProps?: { children: string, onClick: () => void }, data?: { secondaryAction: { label: string, onClick: () => void } } }[],
    notifications: [] as { title: string, body: string }[],
    permissionRequests: 0,
    fetches: [] as string[],
    opened: [] as string[],
    pullRequestRefreshes: [] as string[],
    /** The pull requests GitHub lists, by branch */
    pullRequests: {} as Record<string, { number: number, state: string, headRefOid: string }>,
    /** "<branch> <comment>" of each close of a pull request */
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
  branchSyncStore: { getState: () => ({ branches: branches.current, branchOrder: Object.keys(branches.current) }) },
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
  patronus.problems = null
  patronus.failingCalls = []
  patronus.failing = []
  patronus.failureKind = "checks"
  patronus.detailCalls = []
  resetFailingAnnouncements()
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
    expect(notices.toasts.map(it => [it.title, it.description, it.actionProps?.children, it.data?.secondaryAction.label])).toEqual([["auth merged into master", "Safe Push", "Fetch", "Open in Patronus"]])
    expect(notices.notifications).toEqual([])
    notices.toasts[0]?.data?.secondaryAction.onClick()
    expect(notices.opened).toEqual(["u"])
    notices.toasts[0]?.actionProps?.onClick()
    expect(notices.fetches).toEqual(["/repo"])

    // the same robot in a later answer is no news
    await refreshPatronus("manual")
    expect(notices.toasts).toHaveLength(1)
  })

  it("polls a watched robot of no branch and names its merge by what it tests", async () => {
    patronus.list = available()
    await refreshPatronus("sync")
    // the head of the pull request of someone else: no branch has it
    const ofReview = { id: "pr", branchName: null, sourceRevision: "their-head" }
    watchRobot(robot(ofReview), "#101 Rework the session store")
    expect(getBranchRobot("auth")).toBeUndefined()
    patronus.poll = available(robot({ ...ofReview, status: "success", finishTime: 200 }))
    await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS)
    expect(patronus.polls.at(-1)?.robots).toEqual([{ id: "pr", status: "running" }])
    expect(notices.toasts.map(it => it.title)).toEqual(["#101 Rework the session store merged into master"])
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
    // the merge of an own branch keeps it, in the GitHub repository of the Space mirror
    expect(patronus.commands).toEqual([{ repositoryPath: "/repo", pullRequestNumber: 7, mode: "rebaseAutosquash", subject: "ownBranch", githubRepository: null }])
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

  it("starts a reviewed pull request of someone else with its thumbs up, the robot only if the user's robots list it", async () => {
    patronus.replies = [{ status: "accepted" }]
    patronus.list = available(ofPullRequest)
    const found = startPatronusFromReviewedPullRequest(7, "acme/monorepo")
    await vi.advanceTimersByTimeAsync(COMMAND_REPLY_INTERVAL_MS)
    expect(await found).toEqual(ofPullRequest)
    expect(patronus.commands).toEqual([{ repositoryPath: "/repo", pullRequestNumber: 7, mode: "rebaseAutosquash", subject: "reviewedPullRequest", githubRepository: "acme/monorepo" }])

    // Patronus may count the robot as the author's: the thumbs up is the start all the same
    patronus.replies = [{ status: "accepted" }]
    patronus.list = available()
    const notFound = startPatronusFromReviewedPullRequest(8, "acme/monorepo")
    await vi.advanceTimersByTimeAsync(REVIEWED_ROBOT_WAIT_MS + 2 * COMMAND_REPLY_INTERVAL_MS)
    expect(await notFound).toBeNull()
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

describe("failing robots", () => {
  it("asks whether the running robots will fail after a list and each poll, until known, never for a canceling one", async () => {
    patronus.list = available(robot(), robot({ id: "r2", branchName: "login", sourceRevision: "login-tip", status: "canceling" }))
    setBranches("auth", "login")
    patronus.failing = ["r1"]
    await refreshPatronus("sync")
    await vi.advanceTimersByTimeAsync(0)
    expect(patronus.failingCalls).toEqual([["r1"]])
    expect(patronusStore.getState().repositories["/repo\nuser"]?.failing).toEqual({ r1: true })
    // known: not asked again
    await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS)
    expect(patronus.failingCalls).toEqual([["r1"]])
  })

  it("tells once that a running robot will fail, with Cancel Safe Push and the failed checks", async () => {
    const stop = watchFailingRobots()
    patronus.list = available(robot())
    patronus.failing = ["r1"]
    await refreshPatronus("sync")
    await vi.advanceTimersByTimeAsync(0)
    expect(patronus.detailCalls).toEqual(["r1"])
    expect(notices.toasts.map(it => [it.title, it.description, it.actionProps?.children, it.data?.secondaryAction.label])).toEqual([
      ["A required check of auth failed", "Fast Tests · the Safe Push will fail", "Cancel Safe Push", "Open in Patronus"],
    ])
    // marked again by the details of an open popover: told once
    markRobotFailing("r1")
    patronusStore.setState(state => ({ repositories: { ...state.repositories } }))
    await vi.advanceTimersByTimeAsync(0)
    expect(notices.toasts).toHaveLength(1)
    stop()
  })
})

describe("failed robots", () => {
  /** The robot r1 of auth running after the first list, failed in the next poll */
  async function failRunningRobot(): Promise<void> {
    patronus.list = available(robot())
    await refreshPatronus("sync")
    await vi.advanceTimersByTimeAsync(0)
    patronus.poll = available(robot({ status: "failure", finishTime: 200 }))
    await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS)
  }

  function toasts() {
    return notices.toasts.map(it => [it.title, it.description, it.actionProps?.children, it.data?.secondaryAction.label])
  }

  it("tells once that a robot failed, with Copy Prompt for Agent and the failed checks, once its problems are read", async () => {
    patronus.problems = []
    const stop = watchFailingRobots()
    await failRunningRobot()
    expect(patronus.problemCalls).toEqual(["r1"])
    expect(toasts()).toEqual([["Safe Push of auth failed", "Fast Tests · auth → master", "Copy Prompt for Agent", "Open in Patronus"]])
    // a later answer with the same robot tells nothing
    patronusStore.setState(state => ({ repositories: { ...state.repositories } }))
    await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS)
    expect(notices.toasts).toHaveLength(1)
    stop()
  })

  it("tells a robot whose builds never started with Restart Safe Push: the CI failed, not the branch", async () => {
    patronus.problems = []
    patronus.failureKind = "infrastructure"
    const stop = watchFailingRobots()
    await failRunningRobot()
    expect(toasts()).toEqual([["Safe Push of auth failed", "Fast Tests · auth → master", "Restart Safe Push", "Open in Patronus"]])
    stop()
  })

  it("tells a massive failure to check the target first", async () => {
    patronus.problems = []
    patronus.failureKind = "massive"
    const stop = watchFailingRobots()
    await failRunningRobot()
    expect(toasts()).toEqual([["Safe Push of auth failed", "Fast Tests · many tests failed, check master · auth → master", "Copy Prompt for Agent", "Open in Patronus"]])
    stop()
  })

  it("tells a robot that failed for missing approvals \"needs review\", with its pull request", async () => {
    notices.pullRequests = { auth: { number: 12, state: "open", headRefOid: "tip" } }
    const stop = watchFailingRobots()
    await failRunningRobot()
    expect(toasts()).toEqual([["Safe Push of auth needs review", "The code owners did not approve the changes", "Open Pull Request", "Open in Patronus"]])
    stop()
  })

  it("tells no second time a robot told failing while it ran", async () => {
    patronus.problems = []
    patronus.failing = ["r1"]
    const stop = watchFailingRobots()
    await failRunningRobot()
    expect(toasts()).toEqual([["A required check of auth failed", "Fast Tests · the Safe Push will fail", "Cancel Safe Push", "Open in Patronus"]])
    stop()
  })

  it("tells a robot whose problems never come without them", async () => {
    patronus.problems = "offline"
    const stop = watchFailingRobots()
    await failRunningRobot()
    // Patronus doesn't say what the problems are: the toast waits for them a while
    expect(notices.toasts).toHaveLength(0)
    await vi.advanceTimersByTimeAsync(FAILURE_PROBLEMS_WAIT_MS)
    expect(toasts()).toEqual([["Safe Push of auth failed", "Fast Tests · auth → master", "Copy Prompt for Agent", "Open in Patronus"]])
    stop()
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
