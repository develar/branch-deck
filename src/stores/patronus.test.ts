import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test"
import { internalError } from "@/lib/api-error"
import type {
  FactState,
  GetRobotCommitMapParams,
  LandedPullRequest,
  News,
  PatronusAvailability,
  PatronusPart,
  PatronusRobot,
  PatronusView,
  Reason,
  RobotsView,
} from "@/lib/bindings"
import { resetEventToasts } from "@/lib/event-toast"
import { setWindowFocus } from "@/lib/os-notification"
import { formatFailedRobotToast, resetFailingAnnouncements, tellRobotNews, watchFailingRobots } from "@/lib/patronus-failing"
import { showRobotToast } from "@/lib/robot-toast"
import { showRobotEventToast } from "@/lib/robot-toast"
import { currentDemand, resetDemand } from "@/model/demand"
import { formatPushHoldReason, formatStackHold } from "@/presenters/reasons"
import {
  applyRobotsFacts,
  applySessionFacts,
  getBranchRobot,
  getRobot,
  getLandedBy,
  getLandingBranchCount,
  getLandingSpan,
  getPushHold,
  getStackLanding,
  isPatronusPending,
  isRobotProblemsPending,
  patronusStore,
  resetPatronus,
  watchRobot,
  whenRepositoryViewCurrent,
} from "@/stores/patronus"
import { robotCoverageStore, watchRobotCoverage } from "@/stores/robot-coverage"
import { applyRobotDetailsFacts, resetRobotDetails } from "@/stores/robot-details"
import { PUSH, testRemoteStatus } from "@/stores/test-branch"
import { enabled } from "@/test-support/actions"
import { emptyView } from "@/test-support/fake-sessions"
import { branchFacts, landedWhole, robotOf, stackFacts, stackHeld, withRowRobots } from "@/test-support/patronus-repository"
import { describeToast, openToasts, resetToasts, toastModule, toasts as shownToasts } from "@/test-support/toast-mock"

const { patronus, branches } = vi.hoisted(() => ({
  patronus: {
    /** The reasons of each `refresh_facts` of the robots the page asked */
    refreshes: [] as string[],
    /** What the session's next list answers (`list`, a list a refresh asks) and its next poll (`poll`) */
    list: null as RobotsAnswer | null,
    poll: null as RobotsAnswer | null,
    problemCalls: [] as string[],
    /** What the problems of a robot are: the review approval check unless a test says otherwise; "offline" for an error */
    problems: null as { title: string; subsystem: string }[] | "offline" | null,
    /** Holds the answer of the problems until a test resolves it */
    problemsHeld: null as Promise<void> | null,
    /** The robot ids of each ask whether robots will fail */
    failingCalls: [] as string[][],
    failing: [] as string[],
    /** What failed (`RobotFailureKind`), which decides the step of the failed toast */
    failureKind: "checks" as string,
    detailCalls: [] as string[],
    /** What the session derives of the robots and the table after each answer of Patronus */
    view: { branches: {}, stacks: {}, landedByRobot: {}, restarts: {} } as PatronusView,
    /** How the landing needs of the view stand */
    landingNeeds: { kind: "fresh" } as FactState,
    /** The session derives nothing while it is set: the view stays as it was, `stale` once there is one */
    viewHeld: false,
    /** Every derivation fails with this while it is set */
    viewFailure: null as string | null,
    /** By robot id, the pull requests the news of its merge says its landing closes (`domain_policy::landed`) */
    closes: {} as Record<string, LandedPullRequest[]>,
    commitMapCalls: [] as GetRobotCommitMapParams[],
  },
  branches: {
    current: {} as Record<string, unknown>,
    stacks: {} as Record<string, unknown>,
    /** The count of finished syncs, and of the patches that changed the table of the session (`tableRevision`) */
    syncCount: 0,
    tableRevision: 0,
    /** The sync a merge waits for (`whenSynced`) ends with it */
    synced: Promise.resolve(),
    /** The repository the main window shows */
    selected: "/repo",
  },
}))

vi.mock("@/lib/bindings", async importOriginal => ({
  // the constants of the bindings (the defaults of the settings Rust reads); the commands are the test's
  ...(await importOriginal<typeof import("@/lib/bindings")>()),
  commands: {
    getPatronusFailingRobots: async (params: { robotIds: string[] }) => {
      patronus.failingCalls.push(params.robotIds)
      return { status: "ok", data: params.robotIds.filter(id => patronus.failing.includes(id)) }
    },
    getRobotFailure: async () => ({ status: "ok", data: { failure: { kind: patronus.failureKind }, commits: null, headBranch: "master" } }),
    getRobotCommitMap: async (params: GetRobotCommitMapParams) => {
      patronus.commitMapCalls.push(params)
      return { status: "ok", data: { commits: [], sourceMissing: false, virtualBehindHead: false } }
    },
    getPatronusRobotProblems: async (params: { robotId: string }) => {
      patronus.problemCalls.push(params.robotId)
      await patronus.problemsHeld
      if (patronus.problems === "offline") {
        return { status: "error", error: "offline" }
      }
      return { status: "ok", data: patronus.problems ?? [{ title: "Changes require review approval", subsystem: "OWNERSHIP_REVIEW_APPROVAL_CHECK" }] }
    },
  },
}))

const { notices } = vi.hoisted(() => ({
  notices: {
    notifications: [] as { title: string; body: string }[],
    permissionRequests: 0,
    fetches: [] as string[],
    /** The repositories of the Update Project runs of a toast (`runUpdateProjectOf`: an update while shown, else a fetch) */
    updates: [] as string[],
    opened: [] as string[],
    pullRequestRefreshes: [] as string[],
    /** The pull requests GitHub lists, by branch */
    pullRequests: {} as Record<string, { number: number; state: string; headRefOid: string }>,
    /** "<repository> <robot>" of each `CloseLandedPullRequests` the page ran */
    closed: [] as string[],
    /** The closes that fail in the outcome, by number */
    closeFailures: [] as number[],
    /** The pull requests of the whole stacks GitHub lists, by root */
    stackPullRequests: {} as Record<string, { number: number; state: string; headRefOid: string }>,
  },
}))

const { logged } = vi.hoisted(() => ({ logged: [] as { title: string; keyValues: Record<string, unknown> }[] }))

vi.mock("@tauri-apps/plugin-log", () => ({
  error: async (title: string, options: { keyValues: Record<string, unknown> }) => {
    logged.push({ title, keyValues: options.keyValues })
  },
}))

vi.mock("@/stores/pull-requests", () => ({
  refreshPullRequests: async (reason: string) => {
    notices.pullRequestRefreshes.push(reason)
  },
  getPullRequest: (name: string) => notices.pullRequests[name],
  getStackPullRequest: (root: string) => notices.stackPullRequests[root],
}))

vi.mock("@/lib/run-intent", async importOriginal => ({
  ...(await importOriginal<typeof import("@/lib/run-intent")>()),
  // the closes of a merged robot: Rust plans them in the session of the robot's repository, with their comments
  runIntent: async (repositoryPath: string, intent: { kind: string; landing: { by: string; robotId?: string } }) => {
    notices.closed.push(`${repositoryPath} ${intent.kind} ${intent.landing.robotId}`)
    const effects = notices.closeFailures.map(number => ({
      effect: { kind: "closePullRequest", branch: "auth", number, comment: "c" },
      status: { kind: "failed", error: { code: "networkError", message: "GitHub is down" } },
    }))
    return { kind: "ran", plan: {}, outcome: { effects, result: { kind: "none" } } }
  },
}))

vi.mock("@/components/ui/toast", async () => (await import("@/test-support/toast-mock")).toastModule)

vi.mock("@tauri-apps/plugin-notification", () => ({
  isPermissionGranted: async () => false,
  requestPermission: async () => {
    notices.permissionRequests++
    return "granted"
  },
  sendNotification: (options: { title: string; body: string }) => notices.notifications.push(options),
}))

vi.mock("@/stores/update-project", async () => {
  const { createStore } = await import("zustand/vanilla")
  // what runs, by repository, as a test sets it: the merge toast shows the update while it runs
  const updateProjectStore = createStore<{ running: Record<string, { run: { kind: "update" | "fetch" }; endedAt: null }> }>(() => ({ running: {} }))
  return {
    updateProjectStore,
    selectRunningUpdate: (state: { running: Record<string, unknown> }, repositoryPath: string) => state.running[repositoryPath],
    runFetchBaseline: async (repositoryPath: string) => {
      notices.fetches.push(repositoryPath)
    },
    runUpdateProjectOf: async (repositoryPath: string) => {
      notices.updates.push(repositoryPath)
    },
  }
})

vi.mock("@tauri-apps/plugin-opener", () => ({
  openUrl: async (url: string) => {
    notices.opened.push(url)
  },
}))

// the session lists the robots a refresh asks for and the page gets its patch
const { rowParts } = vi.hoisted(() => ({
  /** The parts of the view of a row by subject key (`getRowPart`), as the test sets what Rust answers */
  rowParts: { of: (_key: string, _part: string): unknown => undefined },
}))

vi.mock("@/model/store", () => ({
  getRowPart: (key: string, part: string) => rowParts.of(key, part),
  refreshSessionFacts: async (_repositoryPath: string, _family: string, reason: string) => {
    patronus.refreshes.push(reason)
    answerRobots(patronus.list!, reason === "manual")
    return true
  },
}))

vi.mock("@/stores/repository", async () => ({
  ...(await import("@/test-support/selected-repository-mock")),
  getSelectedProject: () => ({ path: branches.selected }),
}))
vi.mock("@/stores/activity", async () => (await import("@/stores/test-store-mocks")).activityMock)

const { fetches } = vi.hoisted(() => ({
  /** When Update Project last fetched the baseline of /repo (ms), as the state of the repository keeps it */
  fetches: { lastFetchTime: undefined as number | undefined },
}))

vi.mock("@/stores/settings/repositoryState", async importOriginal => ({
  ...(await importOriginal<typeof import("@/stores/settings/repositoryState")>()),
  getRepositoryState: (path: string) => (path === "/repo" ? { lastFetchTime: fetches.lastFetchTime } : undefined),
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
      tableRevision: branches.tableRevision,
      baselineBranch: "origin/master",
      operations: {},
    }),
    subscribe: () => () => {},
  },
  getBranch: (name: string) => branches.current[name],
  getStackBranch: (root: string) => branches.stacks[root],
  whenSynced: () => branches.synced,
  // the structure Rust sent: the branches built on `name`, parents first, by the parent each was placed on, and those
  // it is built on, root first
  structureIn: (state: { branches: Record<string, { name: string; stack?: { effectiveBase: string | null } }> }, name: string) => {
    const children = (parent: string): string[] =>
      Object.values(state.branches)
        .filter(branch => branch.stack?.effectiveBase === parent)
        .flatMap(branch => [branch.name, ...children(branch.name)])
    return { descendants: children(name), ancestors: ancestorsOf(state.branches, name) }
  },
  getStackAncestors: (name: string) => ancestorsOf(branches.current as Record<string, { stack?: { effectiveBase: string | null } }>, name),
  useStackBranch: () => undefined,
  useBranchSync: () => undefined,
  useBranchTip: () => undefined,
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

/** What Patronus answers the session: the robots, whether it could, the GitHub repository of a list */
interface RobotsAnswer {
  readonly availability: PatronusAvailability
  readonly robots: readonly PatronusRobot[]
  readonly githubRepository?: string | null
}

function available(...robots: PatronusRobot[]): RobotsAnswer {
  return { availability: { status: "available" }, robots }
}

/** The robots of the session of /repo before Patronus answered */
function emptySessionRobots(): RobotsView {
  return { head: { value: null, state: { kind: "loading" }, seq: 0, observedAt: 0 }, robots: {}, failing: [], needsReview: {}, problemsFailed: {} }
}

/** The robots of the session of /repo, as its view has them */
let sessionRobots = emptySessionRobots()
/** The id of the last news of the session */
let lastNews = 0

/**
 * The session applies `answer` of a list (`full`: the whole window, which replaces the robots) or of a poll, as
 * `repository_model::facts::RobotsFact` does, then reads what it asks after an answer at once: which running robots will
 * fail (`patronus.failing`), the problems of the failed ones (`patronus.problems`: the review approval check unless set,
 * "offline" for a failed read); the page gets the patch (`applyRobotsFacts`) and the news of the robots that merged or
 * failed (`tellRobotNews`)
 */
function answerRobots(answer: RobotsAnswer, full: boolean, poll = false): void {
  const previous = sessionRobots
  const isAvailable = answer.availability.status === "available"
  const robots: Record<string, PatronusRobot> = isAvailable && full ? {} : { ...previous.robots }
  if (isAvailable) {
    for (const answered of answer.robots) {
      const known = previous.robots[answered.id]
      robots[answered.id] = answered.profile == null && known?.profile != null ? { ...answered, profile: known.profile } : answered
    }
  }
  const failed = answer.availability.status === "notLoggedIn" || answer.availability.status === "error"
  const head: RobotsView["head"] = {
    value: {
      availability: answer.availability,
      githubRepository: isAvailable && !poll ? (answer.githubRepository ?? null) : (previous.head.value?.githubRepository ?? null),
    },
    state: failed ? { kind: "stale", reason: internalError("Patronus didn't answer") } : { kind: "fresh" },
    seq: previous.head.seq + 1,
    observedAt: Math.floor(Date.now() / 1000),
  }
  const changed = Object.fromEntries(Object.entries(robots).filter(([id, robot]) => JSON.stringify(previous.robots[id]) !== JSON.stringify(robot)))
  const known = new Set(previous.failing)
  const running = Object.values(robots).filter(robot => robot.status === "running")
  const failing = isAvailable ? running.filter(robot => known.has(robot.id) || patronus.failing.includes(robot.id)).map(robot => robot.id) : previous.failing
  const needsReview = { ...previous.needsReview }
  const problemsFailed = { ...previous.problemsFailed }
  for (const robot of Object.values(robots).filter(it => it.status === "failure" && !(it.id in needsReview))) {
    if (patronus.problems === "offline") {
      problemsFailed[robot.id] = "offline"
    } else {
      needsReview[robot.id] = (patronus.problems ?? [{ subsystem: "OWNERSHIP_REVIEW_APPROVAL_CHECK" }]).some(it => it.subsystem === "OWNERSHIP_REVIEW_APPROVAL_CHECK")
    }
  }
  const finished = Object.values(robots).filter(robot => {
    const before = previous.robots[robot.id]
    return before !== undefined && (before.status === "running" || before.status === "canceling") && robot.status !== before.status
  })
  const news: News[] = finished.flatMap((robot): News[] => {
    if (robot.status === "success" && robot.pushMode !== "dryRun") {
      return [{ id: ++lastNews, event: { kind: "robotMerged", robotId: robot.id, closes: patronus.closes[robot.id] ?? [] } }]
    }
    return robot.status === "failure" ? [{ id: ++lastNews, event: { kind: "robotFailed", robotId: robot.id, needsReview: needsReview[robot.id] ?? null } }] : []
  })
  sessionRobots = { head, robots, failing, needsReview, problemsFailed }
  applyRobotsFacts("/repo", sessionRobots, {
    head,
    robots: changed,
    removedRobots: Object.keys(previous.robots).filter(id => !(id in robots)),
    failing,
    needsReview,
    problemsFailed,
  })
  // the session derives the view of the robots after their answer, which comes in the same patch as the news
  derive()
  // the main window tells the failures with the failing robots (`useReviewStatusRefresh`): a test that tells them watches
  tellRobotNews("/repo", failuresWatched ? news : news.filter(item => item.event.kind === "robotMerged"))
  answerDetails()
}

/**
 * The session reads the details of the robots the window demands (a toast that names the failed checks): Fast Tests
 * failed, as its view has them then
 */
function answerDetails(): void {
  const demanded = currentDemand().robotDetails.filter(id => !patronus.detailCalls.includes(id) && sessionRobots.robots[id] !== undefined)
  if (demanded.length === 0) {
    return
  }
  patronus.detailCalls.push(...demanded)
  const view = emptyView()
  const checks = { failed: 1, failedNames: ["Fast Tests"], running: 2, passed: 3, canceled: 0 }
  const robotDetails = Object.fromEntries(
    demanded.map(id => {
      const value = { robot: sessionRobots.robots[id]!, profileId: null, profileName: null, checks, problems: [], failing: false, progress: null }
      return [id, { value, state: { kind: "fresh" as const }, seq: 1, observedAt: 0 }]
    }),
  )
  applyRobotDetailsFacts("/repo", { ...view.facts, robotDetails }, { robotDetails })
}

/** The Patronus view of the session of /repo, as its view has it */
let sessionPart: PatronusPart = emptyView().patronus

/**
 * The session derives the view of the robots and the table (`patronus.view`, the robot of each row of the robots
 * started on it, `withRowRobots`), as `repository_model::facts::PatronusFact` does: `stale` while it is held, a failure
 * `unavailable` without an earlier view and `stale` with the reason after one; the page gets the patch
 */
function derive(): void {
  const before = sessionPart
  const derived = before.state.kind !== "loading" && before.state.kind !== "unavailable"
  if (patronus.viewHeld) {
    sessionPart = derived ? { ...before, state: { kind: "stale" } } : before
  } else if (patronus.viewFailure !== null) {
    const reason = internalError(patronus.viewFailure)
    sessionPart = derived ? { ...before, state: { kind: "stale", reason } } : { ...before, state: { kind: "unavailable", reason } }
  } else {
    sessionPart = { state: { kind: "fresh" }, landingNeeds: patronus.landingNeeds, view: withRowRobots(patronus.view, sessionRobots.robots) }
  }
  const view = emptyView()
  applySessionFacts("/repo", { ...view, facts: { ...view.facts, robots: sessionRobots }, patronus: sessionPart }, { patronus: {} })
}

/** The session's view offers the restart of `robotId` once it failed, as Rust decides it (`PatronusView.restarts`) */
function offersRestart(robotId: string): void {
  patronus.view = { ...patronus.view, restarts: { [robotId]: { action: "restart", refusal: null } } }
}

/** A test watches the failing and the failed robots (`watchFailures`) */
let failuresWatched = false

/** Tells the failing robots (`watchFailingRobots`) and the failed ones of the news, as the main window does */
function watchFailures(): () => void {
  failuresWatched = true
  const stop = watchFailingRobots()
  return () => {
    failuresWatched = false
    stop()
  }
}

/** The session lists the robots: `patronus.list`, the whole window */
async function list(answer: RobotsAnswer = patronus.list!): Promise<void> {
  answerRobots(answer, true)
  await vi.advanceTimersByTimeAsync(0)
}

/** The session polls the running robots: `patronus.poll` */
async function poll(answer: RobotsAnswer = patronus.poll!): Promise<void> {
  answerRobots(answer, false, true)
  await vi.advanceTimersByTimeAsync(0)
}

/** The branches `name` is built on, root first, by the effective base each was placed on */
function ancestorsOf(table: Record<string, { stack?: { effectiveBase: string | null } }>, name: string): string[] {
  const found: string[] = []
  let base = table[name]?.stack?.effectiveBase ?? null
  while (base !== null && !found.includes(base) && table[base] !== undefined) {
    found.unshift(base)
    base = table[base]?.stack?.effectiveBase ?? null
  }
  return found
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
  return { branches: {}, stacks: {}, landedByRobot: { r1: landed }, restarts: {} }
}

/** The stack auth ← login ← oauth, auth with the open pull request #5 at its tip, and the running robot of oauth listed */
async function listOauthStack(): Promise<PatronusRobot> {
  const stacked = (name: string, parent: string | null) => ({ name, commits: [{ hash: `${name}-tip` }], remoteStatus: null, stack: { effectiveBase: parent } })
  branches.current = { auth: stacked("auth", null), login: stacked("login", "auth"), oauth: stacked("oauth", "login") }
  notices.pullRequests = { auth: { number: 5, state: "open", headRefOid: "auth-tip" } }
  const oauth = robot({ branchName: "oauth", sourceRevision: "oauth-tip", pullRequest: { number: 7, url: "u7" } })
  patronus.list = available(oauth)
  await list()
  return oauth
}

/** The view the session derives from now on, derived now, and the store once it took it */
async function answerView(view: Partial<PatronusView>): Promise<void> {
  patronus.view = { branches: {}, stacks: {}, landedByRobot: {}, restarts: {}, ...view }
  derive()
  await whenRepositoryViewCurrent()
}

const START = 1_000_000_000

/**
 * The actions Rust offers of a robot in these tests, as `domain_policy::actions::robot_actions` decides them (its table
 * test covers each state): a running one cancels, a failed one restarts, the prompts of one that failed (not for the
 * approvals) or whose required check fails; it opens in Patronus
 */
rowParts.of = (key, part) => {
  const robot = part === "actions" && key.startsWith("robot:") ? getRobot(key.slice("robot:".length)) : undefined
  if (robot === undefined) {
    return undefined
  }
  const entry = Object.values(patronusStore.getState().repositories)[0]
  const prompts = robot.status === "failure" ? entry?.needsReview[robot.id] !== true : robot.status === "running" && entry?.failing[robot.id] === true
  return enabled(
    ...(robot.status === "failure" ? (["restart-robot"] as const) : []),
    ...(robot.status === "running" ? (["cancel-robot"] as const) : []),
    ...(prompts ? (["copy-robot-prompt", "copy-robot-full-prompt"] as const) : []),
    "open-robot",
  )
}

beforeEach(() => {
  vi.useFakeTimers()
  vi.setSystemTime(START)
  resetPatronus()
  sessionRobots = emptySessionRobots()
  sessionPart = emptyView().patronus
  lastNews = 0
  patronus.refreshes = []
  patronus.problemCalls = []
  patronus.problems = null
  patronus.problemsHeld = null
  patronus.failingCalls = []
  patronus.failing = []
  patronus.failureKind = "checks"
  patronus.detailCalls = []
  resetRobotDetails()
  resetDemand()
  resetFailingAnnouncements()
  patronus.view = { branches: {}, stacks: {}, landedByRobot: {}, restarts: {} }
  patronus.landingNeeds = { kind: "fresh" }
  patronus.viewHeld = false
  patronus.viewFailure = null
  patronus.closes = {}
  patronus.commitMapCalls = []
  robotCoverageStore.setState({ maps: {} })
  notices.pullRequestRefreshes = []
  resetEventToasts()
  resetToasts()
  notices.notifications = []
  notices.permissionRequests = 0
  notices.fetches = []
  notices.updates = []
  notices.opened = []
  notices.pullRequests = {}
  notices.stackPullRequests = {}
  notices.closed = []
  notices.closeFailures = []
  branches.stacks = {}
  branches.syncCount = 0
  branches.tableRevision = 0
  branches.synced = Promise.resolve()
  branches.selected = "/repo"
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
    await list()
    const span = { by: robotOf("air", "r1", true), carried: { commits: ["c1"], amended: [] } }
    const held = branchFacts({ span, landing: { hold: { kind: "span", ...span } } })
    await answerView({
      branches: { air: held, bazel: held, side: held },
      stacks: { air: stackFacts({ landing: stackHeld("air", "r1") }) },
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
    expect(formatStackHold(getStackLanding("air").hold)).toBe("A Safe Push of the whole air stack is running: wait until it merges, or restart it on the current stack")
    // the robot of the whole stack is no branch's, not even the root's
    expect(getBranchRobot("air")).toBeUndefined()
  })

  it("holds the branches a merged Safe Push landed, also one first seen merged, and lands the rest without them", async () => {
    const stacked = (name: string, parent: string | null) => ({ name, commits: [{ hash: `${name}-tip` }], remoteStatus: null, stack: { effectiveBase: parent } })
    branches.current = { auth: stacked("auth", null), login: stacked("login", "auth"), oauth: stacked("oauth", "login"), sso: stacked("sso", "login") }
    expect(getLandingBranchCount("sso")).toBe(3)
    // the robot of oauth merged elsewhere (`bd`, before a restart): the view says what it landed
    patronus.list = available(robot({ branchName: "oauth", sourceRevision: "oauth-tip", status: "success", finishTime: 200 }))
    await list()
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
    expect(shownToasts).toEqual([])
  })

  it("holds the stack branch once merged Safe Pushes landed every branch of the whole stack, and says who landed them", async () => {
    const stacked = (name: string, parent: string | null) => ({ name, commits: [{ hash: `${name}-tip` }], remoteStatus: null, stack: { effectiveBase: parent } })
    branches.current = { auth: stacked("auth", null), login: stacked("login", "auth"), session: stacked("session", "auth") }
    branches.stacks = { auth: { root: "auth", members: ["auth", "login", "session"], tip: "stack-tip", remoteStatus: null } }
    const login = robot({ id: "login-robot", branchName: "login", sourceRevision: "login-tip", status: "success", finishTime: 200 })
    patronus.list = available(login)
    await list()
    const byLogin = robotOf("login", "login-robot")
    await answerView({ branches: { auth: landedWhole(byLogin), login: landedWhole(byLogin, true) } })
    // the Safe Push of login landed auth and login: session is left, so is the stack branch, which carries it
    expect(getPushHold("auth")).toMatchObject({ kind: "landed", branch: "login" })
    expect(formatStackHold(getStackLanding("auth").hold)).toBeNull()

    // session landed with its own: nothing of the stack is left
    patronus.list = available(login, robot({ id: "session-robot", branchName: "session", sourceRevision: "session-tip", status: "success", finishTime: 300 }))
    await list()
    const bySession = robotOf("session", "session-robot")
    // Rust holds the stack branch: every branch of it landed whole
    const stackLanded: Reason = { code: "stackLanded", root: "auth", by: [byLogin, bySession], own: false, target: "master" }
    await answerView({
      branches: { auth: landedWhole(byLogin), login: landedWhole(byLogin, true), session: landedWhole(bySession, true) },
      stacks: { auth: stackFacts({ landing: { hold: stackLanded } }) },
    })
    expect(formatStackHold(getStackLanding("auth").hold)).toBe("Landed in master with login and session: nothing of it is left to push")
  })

  it("takes the robot of each row from the view of the session, which matched it, never from its own matching", async () => {
    const stacked = (name: string, parent: string | null) => ({ name, commits: [{ hash: `${name}-tip` }], remoteStatus: null, stack: { effectiveBase: parent } })
    branches.current = { auth: stacked("auth", null), login: stacked("login", "auth") }
    // a Safe Push of the IDE on the tip of login: Rust matched it by the tip, the page has no rule of its own
    const ide = robot({ id: "ide", branchName: null, sourceRevision: "login-tip" })
    patronus.list = available(ide)
    await list()
    expect(getBranchRobot("login")).toBeUndefined()
    await answerView({ branches: { login: branchFacts({ robotId: "ide" }) } })
    expect(getBranchRobot("login")).toBe(patronusStore.getState().repositories["/repo\nuser"]?.robots.ide)
    expect(getBranchRobot("auth")).toBeUndefined()
  })

  it("keeps the facts of a branch as the same objects when a patch of the view changes only another branch", async () => {
    branches.current = {
      auth: { name: "auth", commits: [{ hash: "auth-tip" }], remoteStatus: null },
      login: { name: "login", commits: [{ hash: "login-tip" }], remoteStatus: null },
    }
    patronus.list = available(robot({ branchName: "auth", sourceRevision: "auth-tip" }))
    await list()
    const held = () => branchFacts({ landing: { hold: { kind: "own", by: robotOf("auth", "r1"), carried: null } } })
    await answerView({ branches: { auth: held() } })
    const hold = getPushHold("auth")
    expect(hold).toMatchObject({ kind: "own", branch: "auth" })
    await answerView({ branches: { auth: held(), login: branchFacts({ landingNeeds: { state: "known", refusals: [] } }) } })
    expect(getPushHold("auth")).toBe(hold)
  })

  it("waits for the view of the robots as they are while the session derives it, then takes it", async () => {
    patronus.list = available(robot({ branchName: "auth", sourceRevision: "auth-tip" }))
    await list()
    patronus.viewHeld = true
    patronus.list = available(robot({ branchName: "auth", sourceRevision: "auth-tip", status: "canceling" }))
    await list()
    let current = false
    void whenRepositoryViewCurrent().then(() => {
      current = true
    })
    await vi.advanceTimersByTimeAsync(0)
    expect(current).toBe(false)
    patronus.viewHeld = false
    await answerView({})
    await vi.advanceTimersByTimeAsync(0)
    expect(current).toBe(true)
  })

  it("holds nothing for a dry run of the whole stack", async () => {
    setBranches("air", "bazel")
    branches.stacks = { air: { root: "air", members: ["air", "bazel"], tip: "stack-tip", remoteStatus: null } }
    patronus.list = available(robot({ branchName: null, stackRoot: "air", sourceRevision: "stack-tip", pushMode: "dryRun" }))
    await list()
    expect([getPushHold("air"), getPushHold("bazel")]).toEqual([null, null])
  })
})

describe("watchRobotCoverage", () => {
  it("asks the commit map of a running robot once its branch changed since it started, once per tip, and drops it once the robot covers the branch", async () => {
    // a commit after the one the robot took, not pushed
    const ahead = testRemoteStatus({ remoteTip: "tip", unpushedCommits: ["newer"], commitsAhead: 1, myCommitsAhead: 1, action: PUSH })
    branches.current = { auth: { name: "auth", commits: [{ hash: "newer" }], remoteStatus: ahead } }
    patronus.list = available(robot())
    await list()
    const stop = watchRobotCoverage()
    await vi.waitFor(() => expect(Object.keys(robotCoverageStore.getState().maps)).toHaveLength(1))
    expect(patronus.commitMapCalls).toEqual([
      { repositoryPath: "/repo", branchName: "auth", stackRoot: null, sourceRevision: "tip", ancestors: [], baselineBranch: "origin/master" },
    ])
    // the same tip is not asked again
    patronus.list = available(robot({ name: "renamed" }))
    await list()
    await Promise.resolve()
    expect(patronus.commitMapCalls).toHaveLength(1)

    // restarted on the branch as it is: it covers every commit, the map goes
    patronus.list = available(robot({ sourceRevision: "newer" }))
    await list()
    await vi.waitFor(() => expect(robotCoverageStore.getState().maps).toEqual({}))
    expect(patronus.commitMapCalls).toHaveLength(1)
    stop()
  })

  it("asks nothing for a finished robot", async () => {
    branches.current = {
      auth: { name: "auth", commits: [{ hash: "newer" }], remoteStatus: testRemoteStatus({ remoteTip: "tip", unpushedCommits: ["newer"], commitsAhead: 1, action: PUSH }) },
    }
    patronus.list = available(robot({ status: "failure", finishTime: 200 }))
    await list()
    const stop = watchRobotCoverage()
    await Promise.resolve()
    await Promise.resolve()
    expect(patronus.commitMapCalls).toEqual([])
    stop()
  })
})

describe("merges", () => {
  it("tells once in a toast with Update Project and the robot page when a running robot of a branch merged", async () => {
    patronus.list = available(robot())
    await list()
    patronus.poll = available(robot({ status: "success", finishTime: 200 }))
    await poll()
    expect(shownToasts.map(it => [it.title, it.description, it.actionProps?.children, it.data?.secondaryAction?.label])).toEqual([
      ["auth merged into master", "Safe Push", "Update Project", "Open in Patronus"],
    ])
    expect(notices.notifications).toEqual([])
    shownToasts[0]?.data?.secondaryAction?.onClick()
    expect(notices.opened).toEqual(["u"])
    // one click picks the merge up: Update Project fetches first
    shownToasts[0]?.actionProps?.onClick()
    expect(notices.updates).toEqual(["/repo"])
    expect(notices.fetches).toEqual([])

    // the same robot in a later answer is no news
    await list()
    expect(shownToasts).toHaveLength(1)
  })

  it("takes the place of the toast of the robot's start: one toast of the robot", async () => {
    patronus.list = available(robot())
    await list()
    showRobotEventToast({ repositoryPath: "/repo", robot: robot(), type: "success", title: "Safe Push started", description: "auth → master" })
    const started = openToasts()[0]!
    patronus.poll = available(robot({ status: "success", finishTime: 200 }))
    await poll()
    expect(openToasts()).toEqual([started])
    expect(openToasts().map(describeToast)).toEqual([["auth merged into master", "Safe Push", "Update Project", "Open in Patronus"]])
  })

  it("offers Update Project also after a fetch since the merge: the update is what picks it up", async () => {
    patronus.list = available(robot())
    await list()
    setWindowFocus(false)
    // fetched a second after the robot finished
    fetches.lastFetchTime = 201_000
    patronus.poll = available(robot({ status: "success", finishTime: 200 }))
    await poll()
    expect(notices.notifications).toEqual([{ title: "auth merged into master", body: "Safe Push" }])
    setWindowFocus(true)
    expect(shownToasts.map(it => [it.title, it.actionProps?.children, it.data?.secondaryAction?.label])).toEqual([
      ["auth merged into master", "Update Project", "Open in Patronus"],
    ])
    shownToasts[0]?.actionProps?.onClick()
    expect(notices.updates).toEqual(["/repo"])
  })

  it("polls a watched robot of no branch and names its merge by what it tests", async () => {
    patronus.list = available()
    await list()
    // the head of the pull request of someone else: no branch has it; the session listed it after the bot's 👍
    const ofReview = { id: "pr", branchName: null, sourceRevision: "their-head" }
    await poll(available(robot(ofReview)))
    watchRobot(robot(ofReview), "#101 Rework the session store")
    expect(getBranchRobot("auth")).toBeUndefined()
    // the session polls it: the window watches it (`watchedRobots` of its demand)
    expect(currentDemand().watchedRobots).toEqual(["pr"])
    patronus.poll = available(robot({ ...ofReview, status: "success", finishTime: 200 }))
    await poll()
    expect(shownToasts.map(it => it.title)).toEqual(["#101 Rework the session store merged into master"])
  })

  it("tells a merge in the background by an OS notification, and in a toast on focus", async () => {
    patronus.list = available(robot())
    await list()
    setWindowFocus(false)
    patronus.poll = available(robot({ status: "success", finishTime: 200 }))
    await poll()
    // the notification says what merged, as the toast does
    expect(notices.notifications).toEqual([{ title: "auth merged into master", body: "Safe Push" }])
    expect(notices.permissionRequests).toBe(1)
    expect(shownToasts).toEqual([])

    setWindowFocus(true)
    expect(shownToasts.map(it => it.title)).toEqual(["auth merged into master"])
  })

  it("closes in the session of the robot's repository the pull requests the news of the merge says its landing closes, and says so", async () => {
    const oauth = await listOauthStack()
    patronus.view = landedOauthStack()
    patronus.closes = { r1: [{ branch: "auth", number: 5 }] }
    notices.closeFailures = [5]
    patronus.poll = available({ ...oauth, status: "success", finishTime: 200 })
    await poll()
    // which pull requests, and the comment of each, are Rust's
    await vi.waitFor(() => expect(notices.closed).toEqual(["/repo closeLandedPullRequests r1"]))
    await vi.waitFor(() =>
      expect(shownToasts.map(it => [it.title, it.description])).toEqual([
        ["oauth merged into master", "Safe Push · closes the pull request of auth: it landed with oauth"],
        ["Cannot close pull request #5", "GitHub is down"],
      ]),
    )
  })

  it("closes the pull requests in the robot's repository when the main window shows another one", async () => {
    const oauth = await listOauthStack()
    await answerView(landedOauthStack())
    patronus.closes = { r1: [{ branch: "auth", number: 5 }] }
    // the user opened another repository, whose auth has an open pull request too, before the merge came
    branches.selected = "/other"
    patronus.poll = available({ ...oauth, status: "success", finishTime: 200 })
    await poll()

    await vi.waitFor(() => expect(shownToasts).toHaveLength(1))
    expect(notices.closed).toEqual(["/repo closeLandedPullRequests r1"])
    expect(shownToasts.map(it => it.description)).toEqual(["Safe Push · closes the pull request of auth: it landed with oauth"])
  })

  it("closes the pull requests of the branches the whole stack landed", async () => {
    const member = (name: string, parent: string | null) => ({ name, commits: [{ hash: `${name}-tip` }], remoteStatus: null, stack: { effectiveBase: parent } })
    branches.current = { air: member("air", null), bazel: member("bazel", "air"), side: member("side", "air") }
    branches.stacks = { air: { root: "air", members: ["air", "bazel", "side"], tip: "stack-tip", remoteStatus: null } }
    notices.pullRequests = {
      air: { number: 5, state: "open", headRefOid: "air-tip" },
      bazel: { number: 6, state: "open", headRefOid: "bazel-tip" },
    }
    const whole = robot({ branchName: null, stackRoot: "air", sourceRevision: "stack-tip", pullRequest: { number: 12, url: "u12" } })
    patronus.list = available(whole)
    await list()
    patronus.view = {
      branches: {},
      stacks: {},
      landedByRobot: { r1: ["air", "bazel", "side"].map(branch => ({ branch, own: false, after: [] })) },
      restarts: {},
    }
    patronus.closes = {
      r1: [
        { branch: "air", number: 5 },
        { branch: "bazel", number: 6 },
      ],
    }
    // the session polls the robot of the whole stack: no branch has it
    patronus.poll = available({ ...whole, status: "success", finishTime: 200 })
    await poll()
    await vi.waitFor(() => expect(notices.closed).toEqual(["/repo closeLandedPullRequests r1"]))
    expect(shownToasts.map(it => [it.title, it.description])).toEqual([
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
    await list()
    patronus.poll = available({ ...auth, status: "success", finishTime: 200 })
    await poll()
    expect(shownToasts.map(it => it.description)).toEqual(["Safe Push · #12 no longer matches the air stack: auth landed"])
    expect(notices.closed).toEqual([])
  })

  it("does not tell a successful dry run or a robot first seen merged", async () => {
    patronus.list = available(robot({ pushMode: "dryRun" }), robot({ id: "r2", branchName: "other", status: "success" }))
    setBranches("auth", "other")
    await list()
    patronus.poll = available(robot({ pushMode: "dryRun", status: "success" }))
    await poll()
    expect(getBranchRobot("auth")?.status).toBe("success")
    expect(shownToasts).toEqual([])
  })
})

describe("robots of pull requests", () => {
  const ofPullRequest = robot({ id: "pr", startTime: START / 1000, pullRequest: { number: 7, url: "https://github.com/acme/monorepo/pull/7" } })

  it("keeps the GitHub repository of a list through a poll, and refreshes the pull requests when a robot of one merged", async () => {
    patronus.list = { ...available(ofPullRequest), githubRepository: "acme/monorepo" }
    await list()
    expect(patronusStore.getState().repositories["/repo\nuser"]?.githubRepository).toBe("acme/monorepo")
    patronus.poll = { ...available({ ...ofPullRequest, status: "success", finishTime: 200 }), githubRepository: null }
    await poll()
    expect(patronusStore.getState().repositories["/repo\nuser"]?.githubRepository).toBe("acme/monorepo")
    expect(notices.pullRequestRefreshes).toEqual(["afterWrite"])
  })
})

describe("facts without an answer", () => {
  it("waits for Patronus until the view of the robots it listed first is in, then never again for a change of them", async () => {
    // not asked yet: whether it lands the repository is unknown
    expect(isPatronusPending()).toBe(true)
    patronus.viewHeld = true
    patronus.list = available(robot({ status: "running" }))
    await list()
    // listed, its view not derived yet: what it holds is unknown
    expect(isPatronusPending()).toBe(true)
    patronus.viewHeld = false
    await answerView({})
    expect(isPatronusPending()).toBe(false)
    // a poll that changes the robot lags the view a moment, which no row waits for
    patronus.viewHeld = true
    patronus.poll = available(robot({ status: "failure", finishTime: 200 }))
    await poll()
    expect(patronusStore.getState().repositories["/repo\nuser"]?.viewState).toEqual({ kind: "stale" })
    expect(isPatronusPending()).toBe(false)
  })

  it("takes a failure of Patronus as its answer, and a list of no robot as having its view", async () => {
    patronus.list = { availability: { status: "error", message: "503" }, robots: [] }
    await list()
    expect(isPatronusPending()).toBe(false)
    resetPatronus()
    patronus.list = available()
    await list()
    expect(isPatronusPending()).toBe(false)
  })

  it("keeps the last view through a failed derivation of it, and says the failure while no view stands for the robots", async () => {
    patronus.viewFailure = "the table is gone"
    await list()
    // no view ever derived: what the robots hold is unknown, which is no "none"
    const entry = () => patronusStore.getState().repositories["/repo\nuser"]
    expect(isPatronusPending()).toBe(false)
    expect(entry()?.listed).toBe(true)
    expect(entry()?.viewState).toMatchObject({ kind: "unavailable", reason: { message: "the table is gone" } })

    // the session derives it again after the next answer of Patronus
    patronus.viewFailure = null
    await list()
    expect(entry()?.viewState.kind).toBe("fresh")
    const view = entry()?.view
    expect(view).not.toBeNull()

    // a later failure keeps that view, stale: its holds and landing ancestors stay
    patronus.viewFailure = "the table is gone"
    patronus.list = available(robot({ id: "r2" }))
    await list()
    expect(entry()?.view).toBe(view)
    expect(entry()?.viewState.kind).toBe("stale")
  })

  it("waits for why a failed robot failed until its problems come, and no longer once Patronus didn't give them", async () => {
    const failed = robot({ status: "failure" })
    expect(isRobotProblemsPending(failed)).toBe(false)
    // the session listed it and reads its problems
    const listed = {
      ...sessionRobots,
      head: { value: { availability: { status: "available" as const }, githubRepository: null }, state: { kind: "fresh" as const }, seq: 1, observedAt: 1 },
      robots: { r1: failed },
    }
    applyRobotsFacts("/repo", listed, null)
    expect(isRobotProblemsPending(failed)).toBe(true)
    applyRobotsFacts("/repo", { ...listed, problemsFailed: { r1: "offline" } }, { problemsFailed: { r1: "offline" } })
    expect(isRobotProblemsPending(failed)).toBe(false)
    // asked again after the next answer: they came
    applyRobotsFacts("/repo", { ...listed, needsReview: { r1: true } }, { needsReview: { r1: true }, problemsFailed: {} })
    expect(patronusStore.getState().repositories["/repo\nuser"]?.needsReview).toEqual({ r1: true })
    expect(isRobotProblemsPending(failed)).toBe(false)
    // a robot that didn't fail waits for nothing
    expect(isRobotProblemsPending(robot({ status: "running" }))).toBe(false)
  })
})

describe("failing robots", () => {
  it("tells once that a running robot will fail, with Cancel Safe Push and the failed checks", async () => {
    const stop = watchFailures()
    patronus.list = available(robot())
    patronus.failing = ["r1"]
    await list()
    await vi.advanceTimersByTimeAsync(0)
    expect(patronus.detailCalls).toEqual(["r1"])
    expect(shownToasts.map(it => [it.title, it.description, it.actionProps?.children, it.data?.secondaryAction?.label])).toEqual([
      ["A required check of auth failed", "Fast Tests · the Safe Push will fail", "Cancel Safe Push", "Open in Patronus"],
    ])
    // still failing in the next answer: told once
    await list()
    expect(shownToasts).toHaveLength(1)
    stop()
  })
})

describe("the toast of a robot follows the robot", () => {
  it("drops Cancel Safe Push once the robot was canceled elsewhere, in place: the toast keeps the robot page", async () => {
    const stop = watchFailures()
    patronus.list = available(robot())
    patronus.failing = ["r1"]
    await list()
    await vi.advanceTimersByTimeAsync(0)
    expect(openToasts().map(describeToast)).toEqual([["A required check of auth failed", "Fast Tests · the Safe Push will fail", "Cancel Safe Push", "Open in Patronus"]])
    const shown = openToasts()[0]!
    // canceled in the Patronus page: no news, the poll says it
    patronus.poll = available(robot({ status: "canceled", finishTime: 200 }))
    await poll()
    expect(openToasts()).toEqual([shown])
    expect(openToasts().map(describeToast)).toEqual([["A required check of auth failed", "Fast Tests · the Safe Push will fail", "Open in Patronus", undefined]])
    shown.actionProps!.onClick!()
    expect(notices.opened).toEqual(["u"])
    stop()
  })

  it("says in place that the robot was restarted elsewhere, and opens the newer one", async () => {
    patronus.problems = []
    patronus.failureKind = "infrastructure"
    offersRestart("r1")
    const stop = watchFailures()
    patronus.list = available(robot())
    await list()
    await vi.advanceTimersByTimeAsync(0)
    patronus.poll = available(robot({ status: "failure", finishTime: 200 }))
    await poll()
    expect(openToasts().map(describeToast)).toEqual([["Safe Push of auth failed", "Fast Tests · auth → master", "Restart Safe Push", "Open in Patronus"]])
    // restarted elsewhere (the IDE, bd): a new robot of the branch
    patronus.list = available(robot({ status: "failure", finishTime: 200 }), robot({ id: "r2", startTime: 300, url: "u2" }))
    await list()
    expect(openToasts().map(it => [it.type, ...describeToast(it)])).toEqual([["info", "Safe Push of auth was restarted", "auth → master", "Open in Patronus", undefined]])
    openToasts()[0]!.actionProps!.onClick!()
    expect(notices.opened).toEqual(["u2"])
    stop()
  })
})

describe("push holds", () => {
  it("holds a branch whose Safe Push runs and the ancestors it lands; not once the robot no longer runs", async () => {
    const stacked = (name: string, parent: string | null) => ({ name, commits: [{ hash: `${name}-tip` }], remoteStatus: null, stack: { effectiveBase: parent } })
    branches.current = { auth: stacked("auth", null), login: stacked("login", "auth"), oauth: stacked("oauth", "login") } as unknown as typeof branches.current
    patronus.list = available(robot({ id: "login-run", branchName: "login", sourceRevision: "login-tip" }))
    await list()
    const span = { by: robotOf("login", "login-run"), carried: null }
    await answerView({
      branches: {
        auth: branchFacts({ span, landing: { hold: { kind: "span", ...span } } }),
        login: branchFacts({ landing: { hold: { kind: "own", by: robotOf("login", "login-run"), carried: null } } }),
      },
    })
    expect(getPushHold("login")).toMatchObject({ kind: "own", branch: "login", robot: { id: "login-run" } })
    expect(getPushHold("auth")).toMatchObject({ kind: "span", branch: "login", robot: { id: "login-run" } })
    expect(getPushHold("oauth")).toBeNull()
    // canceled meanwhile: the view of the running robot holds nothing until the next one comes
    patronus.list = available(robot({ id: "login-run", branchName: "login", sourceRevision: "login-tip", status: "canceling" }))
    await list()
    expect(getPushHold("login")).toBeNull()
    expect(getPushHold("auth")).toBeNull()
  })

  it("holds only the ancestors the view says the commit of a running Safe Push has, and counts their commits", async () => {
    const stacked = (name: string, parent: string | null) => ({ name, commits: [{ hash: `${name}-tip` }], remoteStatus: null, stack: { effectiveBase: parent } })
    // a restack placed env below auth after the Safe Push of login started
    branches.current = { env: stacked("env", null), auth: stacked("auth", "env"), login: stacked("login", "auth") } as unknown as typeof branches.current
    patronus.list = available(robot({ id: "login-run", branchName: "login", sourceRevision: "login-old" }))
    await list()
    const span = { by: robotOf("login", "login-run"), carried: { commits: ["auth-original"], amended: [] } }
    await answerView({ branches: { auth: branchFacts({ span, landing: { hold: { kind: "span", ...span } } }) } })
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
    await list()
    await vi.advanceTimersByTimeAsync(0)
    patronus.poll = available(robot({ status: "failure", finishTime: 200 }))
    await poll()
  }

  function toasts() {
    return shownToasts.map(it => [it.title, it.description, it.actionProps?.children, it.data?.secondaryAction?.label])
  }

  it("tells once that a robot failed, with Copy Prompt for Agent and the failed checks, once its problems are read", async () => {
    patronus.problems = []
    const stop = watchFailures()
    await failRunningRobot()
    expect(toasts()).toEqual([["Safe Push of auth failed", "Fast Tests · auth → master", "Copy Prompt for Agent", "Open in Patronus"]])
    // a later answer with the same robot tells nothing
    patronusStore.setState(state => ({ repositories: { ...state.repositories } }))
    await poll()
    expect(shownToasts).toHaveLength(1)
    stop()
  })

  it("is replaced in place by the toast of its restart, a newer robot of the branch", async () => {
    patronus.problems = []
    patronus.failureKind = "infrastructure"
    offersRestart("r1")
    const stop = watchFailures()
    await failRunningRobot()
    const failed = openToasts()[0]!
    expect(describeToast(failed)).toEqual(["Safe Push of auth failed", "Fast Tests · auth → master", "Restart Safe Push", "Open in Patronus"])
    // what the restart says once Patronus answered it (`announceRobot` of `robot-run.ts`)
    showRobotToast("Safe Push restarted", robot({ id: "r2", startTime: 300 }))
    expect(openToasts()).toEqual([failed])
    expect(openToasts().map(describeToast)).toEqual([["Safe Push restarted", "auth → master", "Open in Patronus", undefined]])
    stop()
  })

  it("leaves the toast of the start that replaced a robot whose failure is told after it", async () => {
    patronus.problems = []
    const stop = watchFailures()
    patronus.list = available(robot())
    await list()
    await vi.advanceTimersByTimeAsync(0)
    // the start that replaced r1 told its robot first (`tellOutcome` of `useSafePush.ts`), before the failure of r1
    showRobotToast("Safe Push started", robot({ id: "r2", startTime: 300 }))
    const started = openToasts()[0]!
    patronus.poll = available(robot({ status: "failure", finishTime: 200 }))
    await poll()
    expect(openToasts()).toEqual([started])
    expect(openToasts().map(describeToast)).toEqual([["Safe Push started", "auth → master", "Open in Patronus", undefined]])
    stop()
  })

  it("tells a robot whose builds never started with Restart Safe Push: the CI failed, not the branch", async () => {
    patronus.problems = []
    patronus.failureKind = "infrastructure"
    offersRestart("r1")
    const stop = watchFailures()
    await failRunningRobot()
    expect(toasts()).toEqual([["Safe Push of auth failed", "Fast Tests · auth → master", "Restart Safe Push", "Open in Patronus"]])
    stop()
  })

  it("tells a massive failure to check the target first", async () => {
    patronus.problems = []
    patronus.failureKind = "massive"
    const stop = watchFailures()
    await failRunningRobot()
    expect(toasts()).toEqual([["Safe Push of auth failed", "Fast Tests · many tests failed, check master · auth → master", "Copy Prompt for Agent", "Open in Patronus"]])
    stop()
  })

  it('tells a robot that failed for missing approvals "needs review", with its pull request', async () => {
    notices.pullRequests = { auth: { number: 12, state: "open", headRefOid: "tip" } }
    const stop = watchFailures()
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

  it("tells no second time a robot told failing while it ran: its toast, while open, says in place that it failed", async () => {
    patronus.problems = []
    patronus.failing = ["r1"]
    const stop = watchFailures()
    await failRunningRobot()
    await vi.advanceTimersByTimeAsync(0)
    expect(toasts()).toEqual([["Safe Push of auth failed", "Fast Tests · auth → master", "Copy Prompt for Agent", "Open in Patronus"]])
    stop()
  })

  it("tells nothing of a robot told failing whose toast is gone: Activity says it failed", async () => {
    patronus.problems = []
    patronus.failing = ["r1"]
    const stop = watchFailures()
    patronus.list = available(robot())
    await list()
    await vi.advanceTimersByTimeAsync(0)
    // its 10 s are over
    toastModule.toast.close(openToasts()[0]!.id)
    patronus.poll = available(robot({ status: "failure", finishTime: 200 }))
    await poll()
    await vi.advanceTimersByTimeAsync(0)
    expect(toasts()).toEqual([["A required check of auth failed", "Fast Tests · the Safe Push will fail", "Cancel Safe Push", "Open in Patronus"]])
    expect(openToasts()).toEqual([])
    stop()
  })

  it("tells a robot whose problems didn't come without them", async () => {
    patronus.problems = "offline"
    const stop = watchFailures()
    await failRunningRobot()
    // the session's news says why is unknown: a plain failure
    expect(toasts()).toEqual([["Safe Push of auth failed", "Fast Tests · auth → master", "Copy Prompt for Agent", "Open in Patronus"]])
    stop()
  })
})
