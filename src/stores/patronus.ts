import { useMemo } from "react"
import { createStore } from "zustand/vanilla"
import { useStore } from "zustand"
import type { KnownRobotStatus, PatronusAvailability, PatronusMergeRequest, PatronusPushMode, PatronusRobot, PatronusRobotStatus } from "@/lib/bindings"
import type { LandingAncestor, LandingDescendant } from "@/lib/patronus"
import { toast } from "@/components/ui/toast"
import { commands } from "@/lib/bindings"
import {
  REVIEW_APPROVAL_SUBSYSTEM,
  findBranchRobot,
  findLandingAncestor,
  findLandingDescendants,
  findMergedRobots,
  formatMergedRobotTitle,
  formatPushMode,
  isPushingRobot,
  isRobotActive,
  openRobot,
} from "@/lib/patronus"
import type { RefreshReason } from "@/lib/refresh-gate"
import { closeLandedPullRequests } from "@/lib/landed-stack"
import { announce, isWindowFocused, resetOsNotification, setWindowFocus } from "@/lib/os-notification"
import { createRefreshGates } from "@/lib/refresh-gate"
import type { RobotEstimate } from "@/lib/robot-eta"
import { estimateRobotDuration, robotSample } from "@/lib/robot-eta"
import { stackAncestors } from "@/lib/stack"
import { recordRobot } from "@/stores/activity"
import type { RobotHistoryEntry } from "@/stores/robot-history"
import { historySamples, recordFinishedRobots, robotHistoryStore } from "@/stores/robot-history"
import type { BranchSyncState } from "@/stores/branch-sync"
import { branchSyncStore, useBranchRemoteTip, useBranchSync, useBranchTip } from "@/stores/branch-sync"
import { refreshPullRequests } from "@/stores/pull-requests"
import { getEffectiveBranchPrefix, getSelectedProject, useEffectiveBranchPrefix, useSelectedProject } from "@/stores/repository"
import { runFetchBaseline } from "@/stores/update-project"

/** A sync asks for new robots at most this often: robots are started elsewhere, a sync never starts one */
export const SYNC_REFRESH_GAP_MS = 5 * 60_000
/** On window focus, robots older than this are asked for */
export const FOCUS_REFRESH_AGE_MS = 120_000
/** Focus events closer together than this ask nothing */
export const MIN_FOCUS_GAP_MS = 60_000
/** A running robot of a branch is polled this often while the window is focused */
export const POLL_INTERVAL_MS = 60_000
/** A running robot that pushes is polled this often while the window is not focused, to tell when it merged */
export const BACKGROUND_POLL_INTERVAL_MS = 5 * 60_000
/** A focus asks for the running robots at once if the last poll is at least this old */
export const MIN_POLL_GAP_MS = 50_000
/** An incremental list starts this long before the previous list started, so a robot started meanwhile is not missed */
export const LIST_OVERLAP_MS = 120_000
/** After a `/patronus` comment, how often the bot's reaction and the robot are asked for */
export const COMMAND_REPLY_INTERVAL_MS = 3_000
/** How long a `/patronus` comment waits for the bot to start the robot */
export const COMMAND_REPLY_TIMEOUT_MS = 60_000

/** The robots of one repository and branch prefix */
export interface RepositoryPatronus {
  /** null until the first answer */
  readonly availability: PatronusAvailability | null
  /** By robot id */
  readonly robots: Readonly<Record<string, PatronusRobot>>
  /** `Date.now()` when the last answered list request started, 0 = never: the next list is a full one */
  readonly listedFrom: number
  /** `Date.now()` of the last answer, 0 = never */
  readonly fetchedAt: number
  readonly isRefreshing: boolean
  /** By robot id, of the failed robots of the branches: failed for missing code-owner approvals */
  readonly needsReview: Readonly<Record<string, boolean>>
  /**
   * `owner/name` of the GitHub repository the branches are mirrored to (the mirror of the committed Space settings, or
   * `branchdeck.githubRepository`), null without
   * one: the branches are reviewed in its pull requests, and a Safe Push of a branch with a pull request is a
   * `/patronus` comment of it
   */
  readonly githubRepository: string | null
}

export interface PatronusState {
  /** By `repositoryKey`; in memory only, Patronus is asked again after a restart */
  readonly repositories: Readonly<Record<string, RepositoryPatronus>>
}

export const patronusStore = createStore<PatronusState>(() => ({ repositories: {} }))

const EMPTY: RepositoryPatronus = { availability: null, robots: {}, listedFrom: 0, fetchedAt: 0, isRefreshing: false, needsReview: {}, githubRepository: null }

function repositoryKey(repositoryPath: string, branchPrefix: string): string {
  return `${repositoryPath}\n${branchPrefix}`
}

function currentKey(): string | null {
  const repositoryPath = getSelectedProject()?.path ?? ""
  const branchPrefix = getEffectiveBranchPrefix()
  return repositoryPath === "" || branchPrefix === "" ? null : repositoryKey(repositoryPath, branchPrefix)
}

// ---- hooks ----

function useRepositoryKey(): string {
  return repositoryKey(useSelectedProject()?.path ?? "", useEffectiveBranchPrefix())
}

/** The robots of the selected repository, for the status line */
export function useRepositoryPatronus(): RepositoryPatronus {
  const key = useRepositoryKey()
  return useStore(patronusStore, state => state.repositories[key] ?? EMPTY)
}

/** The robot of the branch (see `findBranchRobot`); re-renders only when it changes */
export function useBranchRobot(branchName: string): PatronusRobot | undefined {
  const key = useRepositoryKey()
  const localTip = useBranchTip(branchName)
  const remoteTip = useBranchRemoteTip(branchName)
  return useStore(patronusStore, state => findBranchRobot(state.repositories[key]?.robots, branchName, localTip, remoteTip))
}

/**
 * The nearest ancestor of a stacked branch whose robot lands it (`findLandingAncestor`): a Safe Push of the branch waits
 * for it; null for a root and while none runs
 */
export function useLandingAncestor(branchName: string): LandingAncestor | null {
  const key = useRepositoryKey()
  // a string, so the store notifies only when the ancestors change
  const ancestors = useBranchSync(state => stackAncestors(branchName, name => state.branches[name]).map(it => it.name).join("\n"))
  return useStore(patronusStore, state => landingAncestorIn(state.repositories[key]?.robots, ancestors))
}

/** Non-hook variant of `useLandingAncestor`, for menus built on open */
export function getLandingAncestor(branchName: string): LandingAncestor | null {
  const key = currentKey()
  if (key === null) {
    return null
  }
  const branches = branchSyncStore.getState().branches
  const ancestors = stackAncestors(branchName, name => branches[name]).map(it => it.name).join("\n")
  return landingAncestorIn(patronusStore.getState().repositories[key]?.robots, ancestors)
}

/** The landing ancestor of `ancestors` (names, root first, one per line), the same object while its robot is the same */
function landingAncestorIn(robots: Readonly<Record<string, PatronusRobot>> | undefined, ancestors: string): LandingAncestor | null {
  if (ancestors === "" || robots === undefined) {
    return null
  }
  const branches = branchSyncStore.getState().branches
  const found = findLandingAncestor(ancestors.split("\n"), (name) => {
    const branch = branches[name]
    return findBranchRobot(robots, name, branch?.commits[0]?.hash, branch?.remoteStatus?.remoteTip ?? null)
  })
  if (found === null) {
    return null
  }
  const cached = landingCache.get(found.robot)
  if (cached?.branch === found.branch) {
    return cached
  }
  landingCache.set(found.robot, found)
  return found
}

/** Keeps `landingAncestorIn` stable for `useStore`: one result per robot object */
const landingCache = new WeakMap<PatronusRobot, LandingAncestor>()

/**
 * The branch built on this one whose running robot lands it (`findLandingDescendants`): its row says so, and a Safe Push
 * of the branch waits for it; null while none runs
 */
export function useLandingSpan(branchName: string): LandingDescendant | null {
  const robots = useRepositoryRobots()
  return useBranchSync(state => robots === undefined ? null : landingSpansIn(robots, state).get(branchName) ?? null)
}

/**
 * Every landing span of the selected repository, ancestor → the branch whose running robot lands it, for the stack
 * graph (`landingSegments`); renders again only when a span changes, not on every sync event
 */
export function useLandingSpans(): ReadonlyMap<string, string> {
  const robots = useRepositoryRobots()
  const key = useBranchSync(state => robots === undefined ? "" : [...landingSpansIn(robots, state)].map(([name, span]) => `${name}\t${span.branch}`).join("\n"))
  return useMemo(() => new Map(key === "" ? [] : key.split("\n").map(line => line.split("\t") as [string, string])), [key])
}

/** Non-hook variant of `useLandingSpan`, for menus built on open */
export function getLandingSpan(branchName: string): LandingDescendant | null {
  const key = currentKey()
  const robots = key === null ? undefined : patronusStore.getState().repositories[key]?.robots
  return robots === undefined ? null : landingSpansIn(robots, branchSyncStore.getState()).get(branchName) ?? null
}

interface LandingSpans {
  readonly robots: Readonly<Record<string, PatronusRobot>>
  readonly branches: BranchSyncState["branches"]
  readonly order: BranchSyncState["branchOrder"]
  readonly spans: ReadonlyMap<string, LandingDescendant>
}

/** The last landing spans: every row asks for its own, they are computed once per change of the robots or the branches */
let lastLandingSpans: LandingSpans | null = null

function landingSpansIn(robots: Readonly<Record<string, PatronusRobot>>, state: BranchSyncState): ReadonlyMap<string, LandingDescendant> {
  const { branches, branchOrder: order } = state
  const last = lastLandingSpans
  if (last !== null && last.robots === robots && last.branches === branches && last.order === order) {
    return last.spans
  }
  const spans = findLandingDescendants(
    order,
    name => stackAncestors(name, it => branches[it]).map(it => it.name),
    (name) => {
      const branch = branches[name]
      return findBranchRobot(robots, name, branch?.commits[0]?.hash, branch?.remoteStatus?.remoteTip ?? null)
    },
  )
  // the same object while the same robot lands the branch, so its row doesn't render again on every sync event
  for (const [name, span] of spans) {
    const previous = last?.spans.get(name)
    if (previous !== undefined && previous.branch === span.branch && previous.robot === span.robot) {
      spans.set(name, previous)
    }
  }
  lastLandingSpans = { robots, branches, order, spans }
  return spans
}

/** What an estimate depends on of a robot */
type EstimatedRobot = Pick<PatronusRobot, "pushMode" | "targetBranch" | "profile">

/**
 * How long a robot like this one usually runs (`estimateRobotDuration`), from the robots listed for the repository and
 * the history of finished ones; null without history. Renders again only when the estimate changes. No request.
 */
export function useRobotEstimate(robot: EstimatedRobot | undefined): RobotEstimate | null {
  const key = useRepositoryKey()
  const repositoryPath = useSelectedProject()?.path ?? ""
  const history = useStore(robotHistoryStore, state => state.items)
  const pushMode = robot?.pushMode
  const targetBranch = robot?.targetBranch
  const profileId = robot?.profile == null ? undefined : robot.profile.id
  const profileName = robot?.profile?.name ?? null
  return useStore(patronusStore, (state) => {
    const robots = state.repositories[key]?.robots
    if (robots === undefined || pushMode === undefined || targetBranch === undefined) {
      return null
    }
    const profile = profileId === undefined ? null : { id: profileId, name: profileName }
    return estimateIn(robots, history, repositoryPath, { pushMode, targetBranch, profile })
  })
}

/** Non-hook variant of `useRobotEstimate`, for the Dock */
export function getRobotEstimate(robot: EstimatedRobot): RobotEstimate | null {
  const key = currentKey()
  const robots = key === null ? undefined : patronusStore.getState().repositories[key]?.robots
  return robots === undefined ? null : estimateIn(robots, robotHistoryStore.getState().items, getSelectedProject()?.path ?? "", robot)
}

interface Estimates {
  readonly robots: Readonly<Record<string, PatronusRobot>>
  readonly history: readonly RobotHistoryEntry[]
  readonly repositoryPath: string
  /** By push mode, target and profile */
  readonly values: Map<string, RobotEstimate | null>
}

/** The estimates of the last robots and history: every slot asks for its own, they are computed once per change */
let lastEstimates: Estimates | null = null

function estimateIn(robots: Readonly<Record<string, PatronusRobot>>, history: readonly RobotHistoryEntry[], repositoryPath: string, robot: EstimatedRobot): RobotEstimate | null {
  let estimates = lastEstimates
  if (estimates === null || estimates.robots !== robots || estimates.history !== history || estimates.repositoryPath !== repositoryPath) {
    estimates = { robots, history, repositoryPath, values: new Map() }
    lastEstimates = estimates
  }
  const profile = robot.profile
  const cacheKey = `${robot.pushMode}\n${robot.targetBranch}\n${profile == null ? "?" : `${profile.id ?? ""}\n${profile.name ?? ""}`}`
  let estimate = estimates.values.get(cacheKey)
  if (estimate === undefined) {
    const samples = [...Object.values(robots).map(robotSample), ...historySamples(history, repositoryPath)]
    estimate = estimateRobotDuration(robot, samples, Date.now())
    estimates.values.set(cacheKey, estimate)
  }
  return estimate
}

/** The robot failed for missing code-owner approvals (its problems are read once, see `loadReviewProblems`) */
export function useRobotNeedsReview(robotId: string | undefined): boolean {
  const key = useRepositoryKey()
  return useStore(patronusStore, state => robotId !== undefined && state.repositories[key]?.needsReview[robotId] === true)
}

/** By robot id, the failed robots of the branches that failed for missing code-owner approvals; re-renders only when they change */
export function useRobotsNeedingReview(): Readonly<Record<string, boolean>> {
  const key = useRepositoryKey()
  return useStore(patronusStore, state => state.repositories[key]?.needsReview ?? EMPTY.needsReview)
}

/** Non-hook variant of `useRobotNeedsReview`, for menus built on open */
export function isRobotNeedingReview(robotId: string): boolean {
  const key = currentKey()
  return key !== null && patronusStore.getState().repositories[key]?.needsReview[robotId] === true
}

/** The robots of the selected repository, by id; re-renders only when they change, not on every refresh */
export function useRepositoryRobots(): Readonly<Record<string, PatronusRobot>> | undefined {
  const key = useRepositoryKey()
  return useStore(patronusStore, state => state.repositories[key]?.robots)
}

/** The GitHub repository the branches are reviewed in (see `RepositoryPatronus.githubRepository`) */
export function usePatronusGitHubRepository(): string | null {
  const key = useRepositoryKey()
  return useStore(patronusStore, state => state.repositories[key]?.githubRepository ?? null)
}

/** Non-hook variant of `usePatronusGitHubRepository`, for menus built on open */
export function getPatronusGitHubRepository(): string | null {
  const key = currentKey()
  return key === null ? null : patronusStore.getState().repositories[key]?.githubRepository ?? null
}

/** Non-hook variant of `useBranchRobot`, for menus built on open */
export function getBranchRobot(branchName: string): PatronusRobot | undefined {
  const key = currentKey()
  if (key === null) {
    return undefined
  }
  const branch = branchSyncStore.getState().branches[branchName]
  return findBranchRobot(patronusStore.getState().repositories[key]?.robots, branchName, branch?.commits[0]?.hash, branch?.remoteStatus?.remoteTip ?? null)
}

// ---- updates ----

function update(key: string, patch: (entry: RepositoryPatronus) => Partial<RepositoryPatronus>): void {
  patronusStore.setState((state) => {
    const entry = state.repositories[key] ?? EMPTY
    return { repositories: { ...state.repositories, [key]: { ...entry, ...patch(entry) } } }
  })
}

function sameRobot(a: PatronusRobot, b: PatronusRobot): boolean {
  return a.status === b.status
    && a.sourceRevision === b.sourceRevision
    && a.finishTime === b.finishTime
    && a.name === b.name
    && a.branchName === b.branchName
    && a.cancellationReason === b.cancellationReason
    && a.pushMode === b.pushMode
    && a.targetBranch === b.targetBranch
    && a.startTime === b.startTime
    && a.pullRequest?.number === b.pullRequest?.number
    && a.profile?.id === b.profile?.id
    && a.profile?.name === b.profile?.name
}

/**
 * The robots after an answer: a full list replaces them, an incremental list or a poll adds and updates. A robot that
 * did not change keeps its object, so the chips of other branches don't re-render; nothing changed = the same record.
 */
export function mergeRobots(
  previous: Readonly<Record<string, PatronusRobot>>,
  incoming: readonly PatronusRobot[],
  replace: boolean,
): Readonly<Record<string, PatronusRobot>> {
  const next: Record<string, PatronusRobot> = replace ? {} : { ...previous }
  let changed = replace && Object.keys(previous).length !== incoming.length
  for (const answered of incoming) {
    const known = previous[answered.id]
    // a list doesn't name the profile: the one a poll, the details or the start named stays
    const robot = answered.profile == null && known?.profile != null ? { ...answered, profile: known.profile } : answered
    if (known !== undefined && sameRobot(known, robot)) {
      next[robot.id] = known
    }
    else {
      next[robot.id] = robot
      changed = true
    }
  }
  return changed ? next : previous
}

/** Stores robots of an answer (with `patch`), and tells about the robots that merged since the last answer */
function storeRobots(key: string, incoming: readonly PatronusRobot[], replace: boolean, patch: Partial<RepositoryPatronus>): void {
  const previous = patronusStore.getState().repositories[key]?.robots ?? EMPTY.robots
  const robots = mergeRobots(previous, incoming, replace)
  update(key, () => ({ ...patch, robots }))
  if (robots !== previous) {
    // what the estimates learn from, also after a restart
    recordFinishedRobots(key.slice(0, key.lastIndexOf("\n")), Object.values(robots))
    for (const robot of findMergedRobots(previous, robots)) {
      announceMerge(key, robot)
    }
  }
}

// ---- merges ----

/** The merged robots already told about, once each */
const announcedMerges = new Set<string>()

/**
 * A robot merged a branch: a toast with Fetch, so the header learns that an update brings the branch in, and "Open in
 * Patronus". While no window is focused, an OS notification says it at once (unless Settings turned it off), and the
 * toast waits for the focus.
 * A stacked branch landed its ancestors too: their open pull requests are closed (`closeLandedPullRequests`), and the
 * toast says so.
 */
function announceMerge(key: string, robot: PatronusRobot): void {
  if (announcedMerges.has(robot.id)) {
    return
  }
  announcedMerges.add(robot.id)
  if (robot.pullRequest != null) {
    // Patronus closed the pull request with the label `merged`
    void refreshPullRequests("push")
  }
  const closed = closeLandedPullRequests(robot)
  const description = closed === null ? formatPushMode(robot.pushMode) : `${formatPushMode(robot.pushMode)} · ${closed}`
  recordRobot(formatMergedRobotTitle(robot), description, robot, key.split("\n")[0])
  const notification = { title: formatMergedRobotTitle(robot), body: `${formatPushMode(robot.pushMode)} · Fetch and update to pick it up` }
  announce("patronusMerges", notification, () => showMergeToast(key, robot, description))
}

function showMergeToast(key: string, robot: PatronusRobot, description: string): void {
  const [repositoryPath] = key.split("\n") as [string, string]
  const id = toast.add({
    type: "success",
    title: formatMergedRobotTitle(robot),
    description,
    timeout: 10_000,
    actionProps: {
      children: "Fetch",
      onClick: () => {
        toast.close(id)
        void runFetchBaseline(repositoryPath)
      },
    },
    // Fetch is the primary action, the robot page the second one
    data: { kind: "robot", openRobot: () => openRobot(robot) },
  })
}

// ---- refresh ----

/**
 * The list policy: a sync asks for new robots every five minutes at most, a focus when the robots are older than two
 * minutes, the refresh button always (a full list). A push asks nothing: whether a robot tests an older commit is
 * known locally. Running robots are polled separately (`fetch-updates`, see below).
 */
const listGates = createRefreshGates({
  isDue: (reason, sinceStart, sinceAnswer) => {
    switch (reason) {
      case "manual":
        return true
      case "sync":
        return sinceStart >= SYNC_REFRESH_GAP_MS
      case "focus":
        return sinceStart >= MIN_FOCUS_GAP_MS && sinceAnswer >= FOCUS_REFRESH_AGE_MS
      case "push":
      case "poll":
        return false
    }
  },
  followUpReasons: new Set<RefreshReason>(["manual"]),
})

const pollGates = createRefreshGates({
  isDue: (reason, sinceStart) => reason === "poll" && sinceStart >= MIN_POLL_GAP_MS,
  followUpReasons: new Set<RefreshReason>(),
})

let pollTimer: ReturnType<typeof setTimeout> | undefined

/**
 * Stores an answer of Patronus: its robots (a full list replaces them), its availability and, for a list (not a poll),
 * the GitHub repository. A failure keeps the robots shown: a failed refresh does not make them go away. Never throws;
 * true when Patronus answered with robots.
 */
async function applyAnswer(key: string, request: ReturnType<typeof commands.getPatronusRobots>, replace: boolean, isList: boolean): Promise<boolean> {
  let availability: PatronusAvailability
  try {
    const result = await request
    if (result.status === "ok" && result.data.availability.status === "available") {
      const robots = result.data.robots
      storeRobots(key, robots, replace, {
        availability: result.data.availability,
        fetchedAt: Date.now(),
        ...(isList && { githubRepository: result.data.githubRepository ?? null }),
      })
      return true
    }
    availability = result.status === "ok" ? result.data.availability : { status: "error", message: result.error.message }
  }
  catch (error) {
    availability = { status: "error", message: error instanceof Error ? error.message : String(error) }
  }
  update(key, () => ({ availability, fetchedAt: Date.now() }))
  return false
}

/**
 * Asks Patronus for the robots of the selected repository, if the reason allows it now (see the policy above): the
 * first time and on the refresh button the last 7 days, else the robots started since shortly before the last list.
 * Never fails: what went wrong is the availability. A repository that is not on Space is asked again only by the
 * refresh button.
 */
export function refreshPatronus(reason: RefreshReason): Promise<void> {
  const key = currentKey()
  if (key === null) {
    return Promise.resolve()
  }
  const entry = patronusStore.getState().repositories[key] ?? EMPTY
  if (entry.availability?.status === "notPatronus" && reason !== "manual") {
    return Promise.resolve()
  }
  const [repositoryPath, branchPrefix] = key.split("\n") as [string, string]
  const listed = listGates.refresh(key, reason, entry.fetchedAt, async (current) => {
    const listedFrom = patronusStore.getState().repositories[key]?.listedFrom ?? 0
    const full = current === "manual" || listedFrom === 0
    const startedAt = Date.now()
    update(key, () => ({ isRefreshing: true }))
    try {
      const since = full ? null : Math.floor((listedFrom - LIST_OVERLAP_MS) / 1000)
      const answered = await applyAnswer(key, commands.getPatronusRobots({ repositoryPath, branchPrefix, since, background: current !== "manual" }), full, true)
      if (answered) {
        update(key, () => ({ listedFrom: startedAt }))
      }
    }
    finally {
      update(key, () => ({ isRefreshing: false }))
    }
  })
  return listed.then(() => {
    schedulePoll(false)
    void loadReviewProblems(key)
  })
}

const problemsInFlight = new Set<string>()

/**
 * Reads the problems of the failed robots of the branches once (a finished robot does not change; the backend caches
 * them too), to name a robot that failed for missing approvals "Needs review".
 */
async function loadReviewProblems(key: string): Promise<void> {
  const [repositoryPath, branchPrefix] = key.split("\n") as [string, string]
  const entry = patronusStore.getState().repositories[key]
  if (entry === undefined) {
    return
  }
  for (const branch of Object.values(branchSyncStore.getState().branches)) {
    const robot = findBranchRobot(entry.robots, branch.name, branch.commits[0]?.hash, branch.remoteStatus?.remoteTip ?? null)
    if (robot === undefined || robot.status !== "failure" || robot.id in entry.needsReview || problemsInFlight.has(robot.id)) {
      continue
    }
    problemsInFlight.add(robot.id)
    try {
      const result = await commands.getPatronusRobotProblems({ repositoryPath, branchPrefix, robotId: robot.id })
      if (result.status === "ok") {
        const needsReview = result.data.some(problem => problem.subsystem === REVIEW_APPROVAL_SUBSYSTEM)
        update(key, previous => ({ needsReview: { ...previous.needsReview, [robot.id]: needsReview } }))
      }
    }
    catch {
      // an extra: the chip says "Failed"
    }
    finally {
      problemsInFlight.delete(robot.id)
    }
  }
}

/** The running robots of the branches of the selected repository, with the status the app knows */
function activeBranchRobots(key: string): PatronusRobot[] {
  const robots = patronusStore.getState().repositories[key]?.robots
  const active = new Map<string, PatronusRobot>()
  for (const branch of Object.values(branchSyncStore.getState().branches)) {
    const robot = findBranchRobot(robots, branch.name, branch.commits[0]?.hash, branch.remoteStatus?.remoteTip ?? null)
    if (robot !== undefined && isRobotActive(robot)) {
      active.set(robot.id, robot)
    }
  }
  return [...active.values()]
}

/**
 * Arms the poll timer when a branch has a running robot: every minute while the window is focused, every five minutes
 * while it is not and a robot pushes (to tell when it merged; a dry run waits for the focus). `restart` replaces a
 * pending timer (after a poll, on a focus change), else a pending one is kept.
 */
function schedulePoll(restart: boolean): void {
  if (pollTimer !== undefined && !restart) {
    return
  }
  clearTimeout(pollTimer)
  pollTimer = undefined
  const key = currentKey()
  if (key === null) {
    return
  }
  const active = activeBranchRobots(key)
  const windowFocused = isWindowFocused()
  if (windowFocused ? active.length === 0 : !active.some(isPushingRobot)) {
    return
  }
  pollTimer = setTimeout(() => {
    pollTimer = undefined
    void pollPatronus()
  }, windowFocused ? POLL_INTERVAL_MS : BACKGROUND_POLL_INTERVAL_MS)
}

/** Asks Patronus whether the running robots of the branches changed; only the changed ones come back */
export function pollPatronus(): Promise<void> {
  const key = currentKey()
  if (key === null) {
    return Promise.resolve()
  }
  const known: KnownRobotStatus[] = activeBranchRobots(key).map(robot => ({ id: robot.id, status: robot.status }))
  if (known.length === 0) {
    return Promise.resolve()
  }
  const [repositoryPath, branchPrefix] = key.split("\n") as [string, string]
  return pollGates.refresh(key, "poll", 0, async () => {
    try {
      await applyAnswer(key, commands.pollPatronusRobots({ repositoryPath, branchPrefix, robots: known }), false, false)
    }
    finally {
      schedulePoll(true)
      void loadReviewProblems(key)
    }
  })
}

// ---- actions ----

/** The repository and the branch prefix of the selected repository; throws without one */
export function patronusRequest(): { repositoryPath: string, branchPrefix: string } {
  const key = currentKey()
  if (key === null) {
    throw new Error("No repository is selected")
  }
  const [repositoryPath, branchPrefix] = key.split("\n") as [string, string]
  return { repositoryPath, branchPrefix }
}

/** Patronus answers for the selected repository (it is on Space): the start actions are offered */
export function usePatronusAvailable(): boolean {
  const key = useRepositoryKey()
  return useStore(patronusStore, (state) => {
    const status = state.repositories[key]?.availability?.status
    return status !== undefined && status !== "notPatronus"
  })
}

/** Non-hook variant of `usePatronusAvailable`, for menus built on open */
export function isPatronusAvailable(): boolean {
  const key = currentKey()
  const status = key === null ? undefined : patronusStore.getState().repositories[key]?.availability?.status
  return status !== undefined && status !== "notPatronus"
}

/** Adds or replaces a robot the user just started, restarted or canceled; it is polled while it runs */
export function upsertPatronusRobot(robot: PatronusRobot): void {
  const key = currentKey()
  if (key === null) {
    return
  }
  storeRobots(key, [robot], false, {})
  schedulePoll(false)
}

/**
 * The merge profile an answer named for a robot (its details), for the estimate of its time: kept on the stored robot,
 * as a list never names it. Asks nothing.
 */
export function rememberRobotProfile(robot: PatronusRobot): void {
  const key = currentKey()
  const known = key === null ? undefined : patronusStore.getState().repositories[key]?.robots[robot.id]
  if (key !== null && known !== undefined && robot.profile != null && known.profile == null) {
    storeRobots(key, [{ ...known, profile: robot.profile }], false, {})
  }
}

/**
 * Starts a Safe Push or a Dry Run of the pushed virtual branch (the caller pushes it first when the remote does not have
 * its tip), with the merge request whose approvals its review check counts. Resolves to the robot, now
 * polled while it runs, with the profile it was started with; throws what Patronus said. The caller announces it
 * (`announceRobot`, with the profile name).
 */
export async function startPatronusRobot(
  branchName: string,
  mode: PatronusPushMode,
  profileId: string | null,
  mergeRequest: PatronusMergeRequest | null,
  profileName: string | null = null,
): Promise<PatronusRobot> {
  const result = await commands.startPatronusRobot({
    ...patronusRequest(),
    baselineBranch: branchSyncStore.getState().baselineBranch ?? "",
    branchName,
    mode,
    profileId,
    mergeRequest,
  })
  if (result.status === "error") {
    throw new Error(result.error.message)
  }
  // the form knows the name of the profile Patronus may not have written into the robot yet
  const robot = result.data.profile?.name == null && profileId !== null ? { ...result.data, profile: { id: profileId, name: profileName } } : result.data
  upsertPatronusRobot(robot)
  return robot
}

/**
 * Starts a Safe Push (`/patronus merge`) or a Dry Run (`/patronus dry-run`) of the pushed branch from its GitHub pull
 * request: Patronus links the robot to the pull request only from its comment, so the approvals of the pull request
 * count and Patronus closes it once merged. Posts the comment, then waits up to a minute for the bot: 👎 throws its
 * reason; after 👍, the new robot of the pull request is listed, stored and returned (polled while it runs).
 */
export async function startPatronusFromPullRequest(pullRequestNumber: number, mode: PatronusPushMode): Promise<PatronusRobot> {
  const key = currentKey()
  if (key === null) {
    throw new Error("No repository is selected")
  }
  const { repositoryPath, branchPrefix } = patronusRequest()
  const postedAt = Date.now()
  const posted = await commands.postPatronusCommand({ repositoryPath, pullRequestNumber, mode })
  if (posted.status === "error") {
    throw new Error(posted.error.message)
  }
  const replyParams = { repositoryPath, pullRequestNumber, commentId: posted.data.id }
  let accepted = false
  let rejectedOnce = false
  while (Date.now() - postedAt < COMMAND_REPLY_TIMEOUT_MS) {
    await new Promise(resolve => setTimeout(resolve, COMMAND_REPLY_INTERVAL_MS))
    if (!accepted) {
      const reply = await commands.readPatronusCommandReply(replyParams)
      if (reply.status === "error") {
        throw new Error(reply.error.message)
      }
      if (reply.data.status === "rejected") {
        // the bot reacts first and comments right after: one more look for the reason
        if (reply.data.reason !== null || rejectedOnce) {
          throw new Error(reply.data.reason ?? "Patronus did not start the robot")
        }
        rejectedOnce = true
        continue
      }
      accepted = reply.data.status === "accepted"
      if (!accepted) {
        continue
      }
    }
    const since = Math.floor((postedAt - LIST_OVERLAP_MS) / 1000)
    await applyAnswer(key, commands.getPatronusRobots({ repositoryPath, branchPrefix, since, background: false }), false, false)
    const robot = Object.values(patronusStore.getState().repositories[key]?.robots ?? {})
      .find(it => it.pullRequest?.number === pullRequestNumber && it.startTime * 1000 >= postedAt - LIST_OVERLAP_MS)
    if (robot !== undefined) {
      schedulePoll(false)
      return robot
    }
  }
  throw new Error(`Patronus did not start a robot within a minute: see pull request #${pullRequestNumber}`)
}

async function robotAction(command: typeof commands.restartPatronusRobot, robot: PatronusRobot): Promise<PatronusRobot> {
  const result = await command({ ...patronusRequest(), robotId: robot.id })
  if (result.status === "error") {
    throw new Error(result.error.message)
  }
  upsertPatronusRobot(result.data)
  return result.data
}

/** Starts the robot again on the newest commit of its branch; resolves to the new robot, throws what Patronus said */
export function restartPatronusRobot(robot: PatronusRobot): Promise<PatronusRobot> {
  return robotAction(commands.restartPatronusRobot, robot)
}

/** Asks Patronus to cancel the robot; resolves to it canceling, throws what Patronus said */
export function cancelPatronusRobot(robot: PatronusRobot): Promise<PatronusRobot> {
  return robotAction(commands.cancelPatronusRobot, robot)
}

/** The current status of the robot (one row read), stored; null when Patronus can't answer */
export async function fetchPatronusRobotStatus(robot: PatronusRobot): Promise<PatronusRobotStatus | null> {
  const result = await commands.getPatronusRobotStatus({ ...patronusRequest(), robotId: robot.id })
  if (result.status === "error") {
    return null
  }
  if (result.data !== robot.status) {
    upsertPatronusRobot({ ...robot, status: result.data })
  }
  return result.data
}

/**
 * While the window is not focused, only a robot that pushes is polled, slowly; on focus the merges told meanwhile get
 * their toasts and the running robots are asked for at once
 */
export function setPatronusWindowFocus(focused: boolean): void {
  setWindowFocus(focused)
  if (!focused) {
    schedulePoll(true)
    return
  }
  clearTimeout(pollTimer)
  pollTimer = undefined
  void pollPatronus().then(() => schedulePoll(false))
}

/** Forgets the gates, the timer and the data (tests) */
export function resetPatronus(): void {
  problemsInFlight.clear()
  listGates.reset()
  pollGates.reset()
  clearTimeout(pollTimer)
  pollTimer = undefined
  resetOsNotification()
  announcedMerges.clear()
  patronusStore.setState({ repositories: {} })
}
