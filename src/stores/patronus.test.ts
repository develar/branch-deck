import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type {
  GetPatronusRobotsParams,
  GetRobotCommitMapParams,
  PatronusCommandReply,
  PatronusRobot,
  PatronusRobotsResult,
  PollPatronusRobotsParams,
  PostPatronusCommandParams,
  GetRepositoryViewParams,
  LandingRefusal,
  PatronusView,
  WaitLandingNeedsParams,
} from "@/lib/bindings"
import { formatPushHoldReason } from "@/lib/patronus"
import { FAILURE_PROBLEMS_WAIT_MS, formatFailedRobotToast, resetFailingAnnouncements, watchFailingRobots } from "@/lib/patronus-failing"
import { getStackLandedBy, resetLandedStacks } from "@/stores/landed-stacks"
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
  getLandedBy,
  getLandedWithChildrenReason,
  getLandingBranchCount,
  getLandingNeeds,
  getLandingSpan,
  getPushHold,
  getStackBranchHoldReason,
  getStackLandedReason,
  markRobotFailing,
  mergeRobots,
  patronusStore,
  refreshPatronus,
  resetPatronus,
  setPatronusWindowFocus,
  startPatronusFromPullRequest,
  startPatronusFromReviewedPullRequest,
  watchRobot,
  whenRepositoryViewCurrent,
} from "@/stores/patronus"
import { robotCoverageStore, watchRobotCoverage } from "@/stores/robot-coverage"
import { appSettingsStore } from "@/stores/settings/appSettings"
import { testRemoteStatus } from "@/stores/test-branch"
import { branchFacts, landedWhole, robotOf } from "@/test-support/patronus-repository"

const { patronus, branches } = vi.hoisted(() => ({
  patronus: {
    lists: [] as GetPatronusRobotsParams[],
    polls: [] as PollPatronusRobotsParams[],
    list: null as PatronusRobotsResult | null,
    poll: null as PatronusRobotsResult | null,
    problemCalls: [] as string[],
    /** What the problems of a robot are: the review approval check unless a test says otherwise; "offline" for an error */
    problems: null as { title: string; subsystem: string }[] | "offline" | null,
    /** The robot ids of each ask whether robots will fail */
    failingCalls: [] as string[][],
    failing: [] as string[],
    /** What failed (`RobotFailureKind`), which decides the step of the failed toast */
    failureKind: "checks" as string,
    detailCalls: [] as string[],
    commands: [] as PostPatronusCommandParams[],
    replies: [] as PatronusCommandReply[],
    replyCalls: 0,
    viewCalls: [] as GetRepositoryViewParams[],
    view: { branches: {}, stacks: {}, landedByRobot: {} } as PatronusView,
    /** The waits for the landing needs; the view the backend answers once the computation finished, then counted */
    landingWaits: [] as WaitLandingNeedsParams[],
    afterLanding: null as PatronusView | null,
    landingGeneration: 0,
    /** Holds the wait until a test resolves it: the computation runs meanwhile */
    landingComputed: null as Promise<void> | null,
    commitMapCalls: [] as GetRobotCommitMapParams[],
  },
  branches: {
    current: {} as Record<string, unknown>,
    stacks: {} as Record<string, unknown>,
    /** The count of finished syncs and the fingerprint of what the last one read */
    syncCount: 0,
    fingerprint: null as string | null,
    /** The sync a merge waits for (`whenSynced`) ends with it */
    synced: Promise.resolve(),
    /** The repository the main window shows */
    selected: "/repo",
  },
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
    getRobotCommitMap: async (params: GetRobotCommitMapParams) => {
      patronus.commitMapCalls.push(params)
      return { status: "ok", data: { commits: [], sourceMissing: false, virtualBehindHead: false } }
    },
    getRepositoryView: async (params: GetRepositoryViewParams) => {
      patronus.viewCalls.push(params)
      return { status: "ok", data: { revision: params.revision, tableGeneration: 1, landingGeneration: patronus.landingGeneration, patronus: patronus.view } }
    },
    waitLandingNeeds: async (params: WaitLandingNeedsParams) => {
      patronus.landingWaits.push(params)
      await patronus.landingComputed
      if (patronus.afterLanding !== null) {
        patronus.view = patronus.afterLanding
        patronus.afterLanding = null
        patronus.landingGeneration += 1
      }
      return { status: "ok", data: patronus.landingGeneration }
    },
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
    toasts: [] as {
      type?: string
      title: string
      description?: string
      actionProps?: { children: string; onClick: () => void }
      data?: { secondaryAction: { label: string; onClick: () => void } }
    }[],
    notifications: [] as { title: string; body: string }[],
    permissionRequests: 0,
    fetches: [] as string[],
    /** The repositories of the Update Project runs of a toast (`runUpdateProjectOf`: an update while shown, else a fetch) */
    updates: [] as string[],
    opened: [] as string[],
    pullRequestRefreshes: [] as string[],
    /** The pull requests GitHub lists, by branch */
    pullRequests: {} as Record<string, { number: number; state: string; headRefOid: string }>,
    /** "<branch> <comment>" of each close of a pull request */
    closed: [] as string[],
    /** The repository key of each close */
    closedIn: [] as string[],
    /** The pull requests of the whole stacks GitHub lists, by root */
    stackPullRequests: {} as Record<string, { number: number; state: string; headRefOid: string }>,
  },
}))

vi.mock("@/stores/pull-requests", () => ({
  refreshPullRequests: async (reason: string) => {
    notices.pullRequestRefreshes.push(reason)
  },
  getPullRequest: (name: string) => notices.pullRequests[name],
  getStackPullRequest: (root: string) => notices.stackPullRequests[root],
  closePullRequest: async (name: string, comment: string, key: string) => {
    notices.closed.push(`${name} ${comment}`)
    notices.closedIn.push(key)
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
  sendNotification: (options: { title: string; body: string }) => notices.notifications.push(options),
}))

vi.mock("@/stores/update-project", () => ({
  runFetchBaseline: async (repositoryPath: string) => {
    notices.fetches.push(repositoryPath)
  },
  runUpdateProjectOf: async (repositoryPath: string) => {
    notices.updates.push(repositoryPath)
  },
}))

vi.mock("@tauri-apps/plugin-opener", () => ({
  openUrl: async (url: string) => {
    notices.opened.push(url)
  },
}))

vi.mock("@/stores/repository", async () => ({
  ...(await import("@/test-support/selected-repository-mock")),
  getSelectedProject: () => ({ path: branches.selected }),
}))
vi.mock("@/stores/activity", async () => (await import("@/stores/test-store-mocks")).activityMock)

const { fetches } = vi.hoisted(() => ({
  /** When Update Project last fetched the baseline of /repo (ms), as the recent projects keep it */
  fetches: { lastFetchTime: undefined as number | undefined },
}))

vi.mock("@/stores/settings/repositorySettings", async importOriginal => ({
  ...(await importOriginal<typeof import("@/stores/settings/repositorySettings")>()),
  getRecentProjects: () => [{ path: "/repo", lastFetchTime: fetches.lastFetchTime }],
}))

vi.mock("@/stores/branch-sync", () => ({
  branchSyncStore: {
    getState: () => ({
      branches: branches.current,
      branchOrder: Object.keys(branches.current),
      stackBranches: branches.stacks,
      parked: {},
      isSyncing: false,
      syncCount: branches.syncCount,
      syncedInputs: branches.fingerprint === null ? null : { repositoryPath: "/repo", branchPrefix: "user", fingerprint: branches.fingerprint },
      baselineBranch: "origin/master",
    }),
    subscribe: () => () => {},
  },
  getBranch: (name: string) => branches.current[name],
  getStackBranch: (root: string) => branches.stacks[root],
  whenSynced: () => branches.synced,
  // the branches built on `name`, parents first, by the parent each was placed on
  stackDescendantsIn: (state: { branches: Record<string, { name: string; stack?: { effectiveBase: string | null } }> }, name: string) => {
    const children = (parent: string): string[] =>
      Object.values(state.branches)
        .filter(branch => branch.stack?.effectiveBase === parent)
        .flatMap(branch => [branch.name, ...children(branch.name)])
    return children(name)
  },
  useStackBranch: () => undefined,
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

/** The view of the merged robot r1 of oauth that landed the stack auth ← login ← oauth, `authAfter` left of auth */
function landedOauthStack(authAfter: string[] = []): PatronusView {
  const landed = [
    { branch: "auth", own: false, after: authAfter },
    { branch: "login", own: false, after: [] },
    { branch: "oauth", own: true, after: [] },
  ]
  return { branches: {}, stacks: {}, landedByRobot: { r1: landed } }
}

/** The view the backend answers from now on, and the store once it took it */
async function answerView(view: Partial<PatronusView>): Promise<void> {
  patronus.view = { branches: {}, stacks: {}, landedByRobot: {}, ...view }
  await whenRepositoryViewCurrent()
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
  patronus.viewCalls = []
  patronus.view = { branches: {}, stacks: {}, landedByRobot: {} }
  patronus.landingWaits = []
  patronus.afterLanding = null
  patronus.landingGeneration = 0
  patronus.landingComputed = null
  patronus.commitMapCalls = []
  robotCoverageStore.setState({ maps: {} })
  notices.pullRequestRefreshes = []
  notices.toasts = []
  notices.notifications = []
  notices.permissionRequests = 0
  notices.fetches = []
  notices.updates = []
  notices.opened = []
  notices.pullRequests = {}
  notices.stackPullRequests = {}
  notices.closed = []
  notices.closedIn = []
  branches.stacks = {}
  branches.syncCount = 0
  branches.fingerprint = null
  branches.synced = Promise.resolve()
  branches.selected = "/repo"
  resetLandedStacks()
  fetches.lastFetchTime = undefined
  patronus.list = available(robot({ status: "failure" }))
  patronus.poll = available()
  setBranches("auth")
})

afterEach(() => {
  vi.useRealTimers()
})

describe("holds", () => {
  it("holds every branch the whole-stack Safe Push lands", async () => {
    setBranches("air", "bazel", "side", "other")
    branches.stacks = { air: { root: "air", members: ["air", "bazel", "side"], tip: "stack-tip", remoteStatus: null } }
    patronus.list = available(robot({ branchName: null, stackRoot: "air", sourceRevision: "stack-tip" }))
    await refreshPatronus("sync")
    const span = { by: robotOf("air", "r1", true), carried: { commits: ["c1"], amended: [] } }
    const held = branchFacts({ span, hold: { kind: "span", ...span } })
    await answerView({
      branches: { air: held, bazel: held, side: held },
      stacks: { air: { landingMember: null, holdRobotId: "r1", startBlocked: true, start: null, dryRun: null } },
    })
    const holds = ["air", "bazel", "side", "other"].map(name => getPushHold(name))
    expect(holds.map(hold => (hold === null ? null : [hold.kind, hold.branch, hold.wholeStack]))).toEqual([
      ["span", "air", true],
      ["span", "air", true],
      ["span", "air", true],
      null,
    ])
    expect(formatPushHoldReason("bazel", holds[1]!)).toBe("A Safe Push of the whole air stack is running: it lands 1 commit of bazel with it")
    // and its stack branch: a push into it would fail the robot at the end
    expect(getStackBranchHoldReason("air")).toBe("A Safe Push of the whole air stack is running: wait until it merges, or restart it on the current stack")
    // the robot of the whole stack is no branch's, not even the root's
    expect(getBranchRobot("air")).toBeUndefined()
  })

  it("holds the branches a merged Safe Push landed, also one first seen merged, and lands the rest without them", async () => {
    const stacked = (name: string, parent: string | null) => ({ name, commits: [{ hash: `${name}-tip` }], remoteStatus: null, stack: { effectiveBase: parent } })
    branches.current = { auth: stacked("auth", null), login: stacked("login", "auth"), oauth: stacked("oauth", "login"), sso: stacked("sso", "login") }
    expect(getLandingBranchCount("sso")).toBe(3)
    // the robot of oauth merged elsewhere (`bd`, before a restart): the view says what it landed
    patronus.list = available(robot({ branchName: "oauth", sourceRevision: "oauth-tip", status: "success", finishTime: 200 }))
    await refreshPatronus("sync")
    const by = robotOf("oauth", "r1")
    await answerView({ branches: { auth: landedWhole(by), login: landedWhole(by), oauth: landedWhole(by, true) } })
    const holds = ["auth", "login", "oauth", "sso"].map(name => getPushHold(name))
    // oauth too: all of it landed with its own robot
    expect(holds.map(hold => (hold === null ? null : [hold.kind, hold.branch]))).toEqual([["landed", "oauth"], ["landed", "oauth"], ["landed", "oauth"], null])
    expect(formatPushHoldReason("login", holds[1]!)).toBe("Landed in master with oauth: nothing of it is left to push")
    expect(formatPushHoldReason("oauth", holds[2]!)).toBe("Merged by Patronus: nothing of it is left to push")
    expect(getLandedBy("auth")?.branch).toBe("oauth")
    // sso, built on login too, lands only itself now
    expect(getLandingBranchCount("sso")).toBe(1)
    // no toast: no robot was seen running
    expect(notices.toasts).toEqual([])
  })

  it("holds the stack branch once merged Safe Pushes landed every branch of the whole stack, and says who landed them", async () => {
    const stacked = (name: string, parent: string | null) => ({ name, commits: [{ hash: `${name}-tip` }], remoteStatus: null, stack: { effectiveBase: parent } })
    branches.current = { auth: stacked("auth", null), login: stacked("login", "auth"), session: stacked("session", "auth") }
    branches.stacks = { auth: { root: "auth", members: ["auth", "login", "session"], tip: "stack-tip", remoteStatus: null } }
    const login = robot({ id: "login-robot", branchName: "login", sourceRevision: "login-tip", status: "success", finishTime: 200 })
    patronus.list = available(login)
    await refreshPatronus("sync")
    const byLogin = robotOf("login", "login-robot")
    await answerView({ branches: { auth: landedWhole(byLogin), login: landedWhole(byLogin, true) } })
    // the Safe Push of login landed auth and login: session is left, so is the stack branch, which carries it
    expect(getPushHold("auth")).toMatchObject({ kind: "landed", branch: "login" })
    expect([getStackLandedReason("auth"), getStackBranchHoldReason("auth"), getLandedWithChildrenReason("auth")]).toEqual([null, null, null])

    // session landed with its own: nothing of the stack is left
    patronus.list = available(login, robot({ id: "session-robot", branchName: "session", sourceRevision: "session-tip", status: "success", finishTime: 300 }))
    vi.advanceTimersByTime(SYNC_REFRESH_GAP_MS)
    await refreshPatronus("sync")
    await answerView({ branches: { auth: landedWhole(byLogin), login: landedWhole(byLogin, true), session: landedWhole(robotOf("session", "session-robot"), true) } })
    expect(getStackLandedReason("auth")).toBe("Landed in master with login and session")
    expect(getStackBranchHoldReason("auth")).toBe("Landed in master with login and session: nothing of it is left to push")
    expect(getLandedWithChildrenReason("auth")).toBe("Landed in master with login and session")
    // a branch without children has no Push with Children
    expect(getLandedWithChildrenReason("login")).toBeNull()
  })

  it("asks the repository view with the robots that land, their matches and the remote facts, once per change of them", async () => {
    const stacked = (name: string, parent: string | null) => ({ name, commits: [{ hash: `${name}-tip` }], remoteStatus: null, stack: { effectiveBase: parent } })
    branches.current = { auth: stacked("auth", null), login: stacked("login", "auth") }
    const merged = robot({ branchName: "login", sourceRevision: "login-old", status: "success", finishTime: 200 })
    patronus.list = available(merged, robot({ id: "dry", branchName: "auth", pushMode: "dryRun", status: "success", finishTime: 100 }))
    await refreshPatronus("sync")
    await answerView({ branches: { login: landedWhole(robotOf("login", "r1"), true) } })
    // a merged dry run lands nothing: not sent; the robot of each branch as the rows match it
    expect(patronus.viewCalls.map(call => [call.repositoryPath, call.branchPrefix, call.robots.map(it => it.id), call.matched])).toEqual([
      ["/repo", "user", ["r1"], { branches: { login: "r1", auth: "dry" }, stacks: {} }],
    ])
    expect(patronus.viewCalls[0]?.remote.branches.login).toEqual({ remoteTip: null, upToDate: false })
    expect(getPushHold("login")).toMatchObject({ kind: "landed", branch: "login" })
    // the same robots and branches are not asked again
    vi.advanceTimersByTime(SYNC_REFRESH_GAP_MS)
    await refreshPatronus("sync")
    await whenRepositoryViewCurrent()
    expect(patronus.viewCalls).toHaveLength(1)
    // a robot that changes asks again
    patronus.list = available(merged, robot({ id: "r2", branchName: "auth" }))
    vi.advanceTimersByTime(SYNC_REFRESH_GAP_MS)
    await refreshPatronus("sync")
    await whenRepositoryViewCurrent()
    expect(patronus.viewCalls.map(call => call.robots.map(it => it.id))).toEqual([["r1"], ["r1", "r2"]])
  })

  it("waits for the landing needs Rust computes in the background, unknown until then, and asks the view again once", async () => {
    branches.current = { auth: { name: "auth", commits: [{ hash: "auth-tip" }], remoteStatus: null } }
    patronus.list = available(robot({ branchName: "auth", sourceRevision: "auth-old" }))
    await refreshPatronus("sync")
    const refusal: LandingRefusal = {
      kind: "missingDeclaration",
      branch: "auth",
      identifier: "controlUrl",
      file: "src/b/Use.kt",
      line: 3,
      usedBy: { id: "a2543083", subject: "Start with controlUrl", branch: "auth" },
      declaredBy: { id: "ecebf000", subject: "Add controlUrl", branch: "util" },
      fix: { kind: "restack", branch: "auth", onto: "util" },
    }
    patronus.afterLanding = { branches: { auth: branchFacts({ landingNeeds: { state: "known", refusals: [refusal] } }) }, stacks: {}, landedByRobot: {} }
    let computed = () => {}
    patronus.landingComputed = new Promise(resolve => {
      computed = resolve
    })
    await answerView({ branches: {} })
    // not computed yet: unknown, never "nothing"
    expect(getLandingNeeds("auth")).toBeNull()
    computed()
    await vi.waitFor(() => expect(patronus.viewCalls).toHaveLength(2))
    await whenRepositoryViewCurrent()
    expect(patronus.landingWaits).toEqual([{ repositoryPath: "/repo", branchPrefix: "user", after: 0 }])
    expect(getLandingNeeds("auth")).toEqual([refusal])
    // known: no wait, no view again
    await whenRepositoryViewCurrent()
    expect([patronus.landingWaits.length, patronus.viewCalls.length]).toEqual([1, 2])
  })

  it("keeps the store as it is when a view asked again says the same, so no row reads it again", async () => {
    branches.current = { auth: { name: "auth", commits: [{ hash: "auth-tip" }], remoteStatus: null } }
    patronus.list = available(robot({ branchName: "auth", sourceRevision: "auth-tip" }))
    await refreshPatronus("sync")
    const held = () => ({ branches: { auth: branchFacts({ hold: { kind: "own", by: robotOf("auth", "r1"), carried: null } }) } })
    await answerView(held())
    const state = patronusStore.getState()
    const hold = getPushHold("auth")
    expect(hold).toMatchObject({ kind: "own", branch: "auth" })
    // the remote moved: asked again, the same answer in a new object
    branches.current = { auth: { name: "auth", commits: [{ hash: "auth-tip" }], remoteStatus: testRemoteStatus({ remoteTip: "auth-tip" }) } }
    await answerView(held())
    expect(patronus.viewCalls).toHaveLength(2)
    expect(patronusStore.getState()).toBe(state)
    expect(getPushHold("auth")).toBe(hold)
  })

  it("asks the repository view after a sync only when the sync read other inputs: the backend's table is the same", async () => {
    branches.fingerprint = "inputs-1"
    patronus.list = available(robot({ branchName: "auth", sourceRevision: "auth-tip" }))
    await refreshPatronus("sync")
    await answerView({})
    // another sync of the same inputs: nothing to ask
    branches.syncCount = 2
    await whenRepositoryViewCurrent()
    expect(patronus.viewCalls).toHaveLength(1)
    // a sync that read a commit: its table is new
    branches.syncCount = 3
    branches.fingerprint = "inputs-2"
    await whenRepositoryViewCurrent()
    expect(patronus.viewCalls).toHaveLength(2)
  })

  it("holds nothing for a dry run of the whole stack", async () => {
    setBranches("air", "bazel")
    branches.stacks = { air: { root: "air", members: ["air", "bazel"], tip: "stack-tip", remoteStatus: null } }
    patronus.list = available(robot({ branchName: null, stackRoot: "air", sourceRevision: "stack-tip", pushMode: "dryRun" }))
    await refreshPatronus("sync")
    expect([getPushHold("air"), getPushHold("bazel")]).toEqual([null, null])
  })
})

describe("watchRobotCoverage", () => {
  it("asks the commit map of a running robot once its branch changed since it started, once per tip, and drops it once the robot covers the branch", async () => {
    // a commit after the one the robot took, not pushed
    const ahead = testRemoteStatus({ remoteTip: "tip", unpushedCommits: ["newer"], commitsAhead: 1, myCommitsAhead: 1 })
    branches.current = { auth: { name: "auth", commits: [{ hash: "newer" }], remoteStatus: ahead } }
    patronus.list = available(robot())
    await refreshPatronus("sync")
    const stop = watchRobotCoverage()
    await vi.waitFor(() => expect(Object.keys(robotCoverageStore.getState().maps)).toHaveLength(1))
    expect(patronus.commitMapCalls).toEqual([
      { repositoryPath: "/repo", branchPrefix: "user", branchName: "auth", stackRoot: null, sourceRevision: "tip", ancestors: [], baselineBranch: "origin/master" },
    ])
    // the same tip is not asked again
    vi.advanceTimersByTime(SYNC_REFRESH_GAP_MS)
    patronus.list = available(robot({ name: "renamed" }))
    await refreshPatronus("sync")
    await vi.waitFor(() => expect(patronus.lists).toHaveLength(2))
    await Promise.resolve()
    expect(patronus.commitMapCalls).toHaveLength(1)

    // restarted on the branch as it is: it covers every commit, the map goes
    vi.advanceTimersByTime(SYNC_REFRESH_GAP_MS)
    patronus.list = available(robot({ sourceRevision: "newer" }))
    await refreshPatronus("sync")
    await vi.waitFor(() => expect(robotCoverageStore.getState().maps).toEqual({}))
    expect(patronus.commitMapCalls).toHaveLength(1)
    stop()
  })

  it("asks nothing for a finished robot", async () => {
    branches.current = { auth: { name: "auth", commits: [{ hash: "newer" }], remoteStatus: testRemoteStatus({ remoteTip: "tip", unpushedCommits: ["newer"], commitsAhead: 1 }) } }
    patronus.list = available(robot({ status: "failure", finishTime: 200 }))
    await refreshPatronus("sync")
    const stop = watchRobotCoverage()
    await Promise.resolve()
    await Promise.resolve()
    expect(patronus.commitMapCalls).toEqual([])
    stop()
  })
})

describe("refreshPatronus", () => {
  it("lists the last days first, then only the robots started since shortly before the last list", async () => {
    await refreshPatronus("sync")
    expect(patronus.lists).toEqual([{ repositoryPath: "/repo", branchPrefix: "user", since: null, background: true, newApi: false }])
    expect(getBranchRobot("auth")?.status).toBe("failure")

    vi.advanceTimersByTime(SYNC_REFRESH_GAP_MS)
    await refreshPatronus("sync")
    expect(patronus.lists[1]?.since).toBe(Math.floor((START - LIST_OVERLAP_MS) / 1000))
  })

  it("asks the list with the New Patronus API when the setting is on", async () => {
    appSettingsStore.setState({ patronusNewApi: true })
    try {
      await refreshPatronus("sync")
      expect(patronus.lists.map(params => params.newApi)).toEqual([true])
    } finally {
      appSettingsStore.setState({ patronusNewApi: false })
    }
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
  it("tells once in a toast with Update Project and the robot page when a running robot of a branch merged", async () => {
    patronus.list = available(robot())
    await refreshPatronus("sync")
    patronus.poll = available(robot({ status: "success", finishTime: 200 }))
    await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS)
    expect(notices.toasts.map(it => [it.title, it.description, it.actionProps?.children, it.data?.secondaryAction.label])).toEqual([
      ["auth merged into master", "Safe Push", "Update Project", "Open in Patronus"],
    ])
    expect(notices.notifications).toEqual([])
    notices.toasts[0]?.data?.secondaryAction.onClick()
    expect(notices.opened).toEqual(["u"])
    // one click picks the merge up: Update Project fetches first
    notices.toasts[0]?.actionProps?.onClick()
    expect(notices.updates).toEqual(["/repo"])
    expect(notices.fetches).toEqual([])

    // the same robot in a later answer is no news
    await refreshPatronus("manual")
    expect(notices.toasts).toHaveLength(1)
  })

  it("offers Update Project also after a fetch since the merge: the update is what picks it up", async () => {
    patronus.list = available(robot())
    await refreshPatronus("sync")
    setPatronusWindowFocus(false)
    // fetched a second after the robot finished
    fetches.lastFetchTime = 201_000
    patronus.poll = available(robot({ status: "success", finishTime: 200 }))
    await vi.advanceTimersByTimeAsync(BACKGROUND_POLL_INTERVAL_MS)
    expect(notices.notifications).toEqual([{ title: "auth merged into master", body: "Safe Push" }])
    setPatronusWindowFocus(true)
    expect(notices.toasts.map(it => [it.title, it.actionProps?.children, it.data?.secondaryAction.label])).toEqual([
      ["auth merged into master", "Update Project", "Open in Patronus"],
    ])
    notices.toasts[0]?.actionProps?.onClick()
    expect(notices.updates).toEqual(["/repo"])
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
    // the notification says what merged, as the toast does
    expect(notices.notifications).toEqual([{ title: "auth merged into master", body: "Safe Push" }])
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
    patronus.view = landedOauthStack()
    patronus.poll = available({ ...oauth, status: "success", finishTime: 200 })
    await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS)
    await vi.waitFor(() => expect(notices.closed).toEqual(["auth Landed in master with #7, the Safe Push of user/virtual/oauth, which is built on this branch."]))
    // the pull request of the robot's repository
    expect(notices.closedIn).toEqual(["/repo\nuser"])
    expect(notices.toasts.map(it => it.description)).toEqual(["Safe Push · closes the pull request of auth: it landed with oauth"])
  })

  it("closes no pull request of the repository shown when the merged robot's repository was left before its view came", async () => {
    const stacked = (name: string, parent: string | null) => ({ name, commits: [{ hash: `${name}-tip` }], remoteStatus: null, stack: { effectiveBase: parent } })
    branches.current = { auth: stacked("auth", null), login: stacked("login", "auth"), oauth: stacked("oauth", "login") }
    notices.pullRequests = { auth: { number: 5, state: "open", headRefOid: "auth-tip" } }
    const oauth = robot({ branchName: "oauth", sourceRevision: "oauth-tip", pullRequest: { number: 7, url: "u7" } })
    patronus.list = available(oauth)
    await refreshPatronus("sync")
    await answerView(landedOauthStack())
    // the merge waits for the sync; meanwhile the user opens another repository, whose auth has an open pull request too
    let synced!: () => void
    branches.synced = new Promise(resolve => {
      synced = resolve
    })
    patronus.poll = available({ ...oauth, status: "success", finishTime: 200 })
    await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS)
    branches.selected = "/other"
    synced()

    await vi.waitFor(() => expect(notices.toasts).toHaveLength(1))
    expect(notices.closed).toEqual([])
    expect(notices.toasts.map(it => it.description)).toEqual(["Safe Push · the pull requests of the branches it landed stay open: repo is no longer the open repository"])
  })

  it("closes the pull requests of what a merged robot landed as the view says it, the toast waiting for the view of the merge", async () => {
    const stacked = (name: string, parent: string | null, remoteTip: string) => ({
      name,
      commits: [{ hash: `${name}-copy`, originalHash: `${name}-original` }],
      remoteStatus: { exists: true, remoteTip, commitsBehind: 0 },
      stack: { effectiveBase: parent },
    })
    branches.current = { auth: stacked("auth", null, "auth-old"), login: stacked("login", "auth", "login-old"), oauth: stacked("oauth", "login", "oauth-old") }
    notices.pullRequests = {
      auth: { number: 5, state: "open", headRefOid: "auth-old" },
      login: { number: 6, state: "open", headRefOid: "login-old" },
    }
    const oauth = robot({ branchName: "oauth", sourceRevision: "oauth-old" })
    patronus.list = available(oauth)
    await refreshPatronus("sync")
    // the view of the running robot lands nothing yet; the one of the merge says auth has a commit left
    patronus.view = landedOauthStack(["auth-new"])
    patronus.poll = available({ ...oauth, status: "success", finishTime: 200 })
    await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS)
    await vi.waitFor(() => expect(notices.closed).toEqual(["login Landed in master with the Safe Push of user/virtual/oauth, which is built on this branch."]))
    expect(patronus.viewCalls.at(-1)?.robots.map(it => [it.id, it.status])).toEqual([["r1", "success"]])
    expect(notices.toasts.map(it => it.description)).toEqual(["Safe Push · closes the pull request of login: it landed with oauth"])
  })

  it("closes the pull requests of the branches the whole stack landed", async () => {
    const member = (name: string, parent: string | null, remoteTip: string | null = null, commitsBehind = 0) => ({
      name,
      commits: [{ hash: `${name}-tip` }],
      remoteStatus: remoteTip === null ? null : { remoteTip, commitsBehind },
      stack: { effectiveBase: parent },
    })
    branches.current = {
      air: member("air", null),
      // pushed before a sync rebuilt it: the remote branch has nothing the branch lacks
      bazel: member("bazel", "air", "bazel-old"),
      // someone else's commit on the remote: the merge doesn't have it
      side: member("side", "air", "side-old", 1),
    }
    branches.stacks = { air: { root: "air", members: ["air", "bazel", "side"], tip: "stack-tip", remoteStatus: null } }
    notices.pullRequests = {
      air: { number: 5, state: "open", headRefOid: "air-tip" },
      bazel: { number: 6, state: "open", headRefOid: "bazel-old" },
      side: { number: 7, state: "open", headRefOid: "side-old" },
    }
    const whole = robot({ branchName: null, stackRoot: "air", sourceRevision: "stack-tip", pullRequest: { number: 12, url: "u12" } })
    patronus.list = available(whole)
    await refreshPatronus("sync")
    patronus.view = {
      branches: {},
      stacks: {},
      landedByRobot: { r1: ["air", "bazel", "side"].map(branch => ({ branch, own: false, after: [] })) },
    }
    // the robot of the whole stack is polled: no branch has it
    patronus.poll = available({ ...whole, status: "success", finishTime: 200 })
    await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS)
    expect(patronus.polls.at(-1)?.robots).toEqual([{ id: "r1", status: "running" }])
    const comment = "Landed in master with #12, the Safe Push of the whole air stack (user/stack/air), which holds this branch."
    await vi.waitFor(() => expect(notices.closed).toEqual([`air ${comment}`, `bazel ${comment}`]))
    expect(notices.toasts.map(it => [it.title, it.description])).toEqual([
      ["Whole air stack merged into master", "Safe Push · closes the pull requests of air and bazel: they landed with the whole stack"],
    ])
  })

  it("says that a piece that landed the root leaves the pull request of the whole stack, never closed by itself", async () => {
    const stacked = (name: string, parent: string | null) => ({ name, commits: [{ hash: `${name}-tip` }], remoteStatus: null, stack: { effectiveBase: parent } })
    branches.current = { air: stacked("air", null), auth: stacked("auth", "air"), side: stacked("side", "air") }
    branches.stacks = { air: { root: "air", members: ["air", "auth", "side"], tip: "stack-tip", remoteStatus: null } }
    notices.stackPullRequests = { air: { number: 12, state: "open", headRefOid: "stack-tip" } }
    const auth = robot({ branchName: "auth", sourceRevision: "auth-tip" })
    patronus.list = available(auth)
    await refreshPatronus("sync")
    patronus.poll = available({ ...auth, status: "success", finishTime: 200 })
    await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS)
    expect(notices.toasts.map(it => it.description)).toEqual(["Safe Push · #12 no longer matches the air stack: auth landed"])
    expect(getStackLandedBy("air")).toBe("auth")
    expect(notices.closed).toEqual([])
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
  const auth = { branchName: "auth" }

  it("posts the command, waits for the thumbs up and returns the robot of the pull request", async () => {
    patronus.replies = [{ status: "pending" }, { status: "accepted" }]
    patronus.list = available(ofPullRequest)
    const started = startPatronusFromPullRequest(7, "rebaseAutosquash", auth)
    await vi.advanceTimersByTimeAsync(2 * COMMAND_REPLY_INTERVAL_MS)
    expect(await started).toEqual(ofPullRequest)
    // the merge of an own branch keeps it, in the GitHub repository of the Space mirror; Rust checks the start of the
    // branch before the comment
    expect(patronus.commands).toEqual([
      {
        repositoryPath: "/repo",
        pullRequestNumber: 7,
        mode: "rebaseAutosquash",
        subject: "ownBranch",
        githubRepository: null,
        start: { branchName: "auth", branchPrefix: "user" },
      },
    ])
    expect(patronus.replyCalls).toBe(2)
    // an incremental list from shortly before the comment
    expect(patronus.lists).toEqual([{ repositoryPath: "/repo", branchPrefix: "user", since: Math.floor((START - LIST_OVERLAP_MS) / 1000), background: false }])
    expect(getBranchRobot("auth")?.id).toBe("pr")
  })

  it("throws the reason the bot gave with its thumbs down", async () => {
    patronus.replies = [
      { status: "rejected", reason: null },
      { status: "rejected", reason: "Pull request #7 is a draft and cannot be merged." },
    ]
    const started = startPatronusFromPullRequest(7, "rebaseAutosquash", auth)
    const failed = expect(started).rejects.toThrow("Pull request #7 is a draft and cannot be merged.")
    await vi.advanceTimersByTimeAsync(2 * COMMAND_REPLY_INTERVAL_MS)
    await failed
    expect(patronus.lists).toEqual([])
  })

  it("gives up after a minute without an answer", async () => {
    const started = startPatronusFromPullRequest(7, "dryRun", auth)
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
    expect(patronus.commands).toEqual([
      { repositoryPath: "/repo", pullRequestNumber: 7, mode: "rebaseAutosquash", subject: "reviewedPullRequest", githubRepository: "acme/monorepo", start: null },
    ])

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

  it("asks after an answer only: sync ends and focus events the list gate refuses ask nothing, each poll still asks", async () => {
    patronus.list = available(robot())
    setBranches("auth")
    await refreshPatronus("sync")
    await vi.advanceTimersByTimeAsync(0)
    expect(patronus.failingCalls).toEqual([["r1"]])
    for (let i = 0; i < 10; i++) {
      await refreshPatronus(i % 2 === 0 ? "sync" : "focus")
      await vi.advanceTimersByTimeAsync(1000)
    }
    expect(patronus.lists).toHaveLength(1)
    expect(patronus.failingCalls).toEqual([["r1"]])

    await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS)
    expect(patronus.polls).toHaveLength(1)
    expect(patronus.failingCalls).toEqual([["r1"], ["r1"]])
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

describe("toast steps decided again at the click", () => {
  function infoToasts() {
    return notices.toasts.filter(it => it.type === "info").map(it => [it.title, it.description, it.actionProps?.children])
  }

  it("cancels nothing once the robot finished: the toast says it merged", async () => {
    const stop = watchFailingRobots()
    patronus.list = available(robot())
    patronus.failing = ["r1"]
    await refreshPatronus("sync")
    await vi.advanceTimersByTimeAsync(0)
    const failing = notices.toasts[0]!
    expect(failing.actionProps?.children).toBe("Cancel Safe Push")
    patronus.poll = available(robot({ status: "success", finishTime: 200 }))
    await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS)

    failing.actionProps!.onClick()
    expect(infoToasts()).toEqual([["Safe Push of auth merged", "Cancel Safe Push no longer applies", "Open in Patronus"]])
    stop()
  })

  it("restarts nothing once the branch has a newer robot: the toast says it was restarted", async () => {
    patronus.problems = []
    patronus.failureKind = "infrastructure"
    const stop = watchFailingRobots()
    patronus.list = available(robot())
    await refreshPatronus("sync")
    await vi.advanceTimersByTimeAsync(0)
    patronus.poll = available(robot({ status: "failure", finishTime: 200 }))
    await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS)
    const failed = notices.toasts.find(it => it.title === "Safe Push of auth failed")!
    expect(failed.actionProps?.children).toBe("Restart Safe Push")
    // restarted elsewhere (the IDE, bd): a new robot of the branch
    patronus.list = available(robot({ status: "failure", finishTime: 200 }), robot({ id: "r2", startTime: 300 }))
    await refreshPatronus("manual")

    failed.actionProps!.onClick()
    expect(infoToasts()).toEqual([["Safe Push of auth was restarted", "auth → master", "Open in Patronus"]])
    stop()
  })
})

describe("push holds", () => {
  it("holds a branch whose Safe Push runs and the ancestors it lands; not once the robot no longer runs", async () => {
    const stacked = (name: string, parent: string | null) => ({ name, commits: [{ hash: `${name}-tip` }], remoteStatus: null, stack: { effectiveBase: parent } })
    branches.current = { auth: stacked("auth", null), login: stacked("login", "auth"), oauth: stacked("oauth", "login") } as unknown as typeof branches.current
    patronus.list = available(robot({ id: "login-run", branchName: "login", sourceRevision: "login-tip" }))
    await refreshPatronus("sync")
    const span = { by: robotOf("login", "login-run"), carried: null }
    await answerView({
      branches: { auth: branchFacts({ span, hold: { kind: "span", ...span } }), login: branchFacts({ hold: { kind: "own", by: robotOf("login", "login-run"), carried: null } }) },
    })
    expect(getPushHold("login")).toMatchObject({ kind: "own", branch: "login", robot: { id: "login-run" } })
    expect(getPushHold("auth")).toMatchObject({ kind: "span", branch: "login", robot: { id: "login-run" } })
    expect(getPushHold("oauth")).toBeNull()
    // canceled meanwhile: the view of the running robot holds nothing until the next one comes
    patronus.list = available(robot({ id: "login-run", branchName: "login", sourceRevision: "login-tip", status: "canceling" }))
    await vi.advanceTimersByTimeAsync(SYNC_REFRESH_GAP_MS)
    await refreshPatronus("sync")
    expect(getPushHold("login")).toBeNull()
    expect(getPushHold("auth")).toBeNull()
  })

  it("holds only the ancestors the view says the commit of a running Safe Push has, and counts their commits", async () => {
    const stacked = (name: string, parent: string | null) => ({ name, commits: [{ hash: `${name}-tip` }], remoteStatus: null, stack: { effectiveBase: parent } })
    // a restack placed env below auth after the Safe Push of login started
    branches.current = { env: stacked("env", null), auth: stacked("auth", "env"), login: stacked("login", "auth") } as unknown as typeof branches.current
    patronus.list = available(robot({ id: "login-run", branchName: "login", sourceRevision: "login-old" }))
    await refreshPatronus("sync")
    const span = { by: robotOf("login", "login-run"), carried: { commits: ["auth-original"], amended: [] } }
    await answerView({ branches: { auth: branchFacts({ span, hold: { kind: "span", ...span } }) } })
    expect(getPushHold("env")).toBeNull()
    expect(formatPushHoldReason("auth", getPushHold("auth")!)).toBe("A Safe Push of login is running: it lands 1 commit of auth with it")
    // the span starts below the root it left out
    expect([getLandingSpan("env"), getLandingSpan("auth")?.branch]).toEqual([null, "login"])
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

  it('tells a robot that failed for missing approvals "needs review", with its pull request', async () => {
    notices.pullRequests = { auth: { number: 12, state: "open", headRefOid: "tip" } }
    const stop = watchFailingRobots()
    await failRunningRobot()
    expect(toasts()).toEqual([["Safe Push of auth needs review", "The code owners did not approve the changes", "Open Pull Request", "Open in Patronus"]])
    stop()
  })

  it("names the profile of a robot started without the approvals whose profile checks them all the same", () => {
    const failed = robot({ status: "failure", profile: { id: "external", name: "External Contribution", skipsReview: false } })
    expect(formatFailedRobotToast(failed, true, null)).toEqual({
      title: "Safe Push of auth needs review",
      description: "External Contribution doesn't skip the code-owner review: the code owners did not approve the changes",
    })
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
    const profile = { id: "lsc", name: "Large Scale Change", skipsReview: null }
    const polled = robot({ profile })
    const previous = { r1: polled }
    expect(mergeRobots(previous, [robot()], true)).toBe(previous)
    expect(mergeRobots(previous, [robot({ status: "failure" })], false).r1?.profile).toEqual(profile)
    // an answer that names another profile wins
    expect(mergeRobots(previous, [robot({ profile: { id: null, name: null, skipsReview: null } })], false).r1?.profile).toEqual({ id: null, name: null, skipsReview: null })
  })
})
