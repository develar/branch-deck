import { useMemo } from "react"
import { createStore } from "zustand/vanilla"
import { useStore } from "zustand"
import type { KnownRobotStatus, PatronusAvailability, PatronusMergeRequest, PatronusPushMode, PatronusRobot, PatronusRobotStatus } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { REVIEW_APPROVAL_SUBSYSTEM, findBranchRobot, isRobotActive, needsPatronusAttention } from "@/lib/patronus"
import type { RefreshReason } from "@/lib/refresh-gate"
import { createRefreshGates } from "@/lib/refresh-gate"
import { branchSyncStore, useBranchRemoteTip, useBranchSync, useBranchTip } from "@/stores/branch-sync"
import { getEffectiveBranchPrefix, getSelectedProject, useEffectiveBranchPrefix, useSelectedProject } from "@/stores/repository"

/** A sync asks for new robots at most this often: robots are started elsewhere, a sync never starts one */
export const SYNC_REFRESH_GAP_MS = 5 * 60_000
/** On window focus, robots older than this are asked for */
export const FOCUS_REFRESH_AGE_MS = 120_000
/** Focus events closer together than this ask nothing */
export const MIN_FOCUS_GAP_MS = 60_000
/** A running robot of a branch is polled this often while the window is focused */
export const POLL_INTERVAL_MS = 60_000
/** A focus asks for the running robots at once if the last poll is at least this old */
export const MIN_POLL_GAP_MS = 50_000
/** An incremental list starts this long before the previous list started, so a robot started meanwhile is not missed */
export const LIST_OVERLAP_MS = 120_000

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
}

export interface PatronusState {
  /** By `repositoryKey`; in memory only, Patronus is asked again after a restart */
  readonly repositories: Readonly<Record<string, RepositoryPatronus>>
}

export const patronusStore = createStore<PatronusState>(() => ({ repositories: {} }))

const EMPTY: RepositoryPatronus = { availability: null, robots: {}, listedFrom: 0, fetchedAt: 0, isRefreshing: false, needsReview: {} }

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

/** The robot failed for missing code-owner approvals (its problems are read once, see `loadReviewProblems`) */
export function useRobotNeedsReview(robotId: string | undefined): boolean {
  const key = useRepositoryKey()
  return useStore(patronusStore, state => robotId !== undefined && state.repositories[key]?.needsReview[robotId] === true)
}

/** Non-hook variant of `useRobotNeedsReview`, for menus built on open */
export function isRobotNeedingReview(robotId: string): boolean {
  const key = currentKey()
  return key !== null && patronusStore.getState().repositories[key]?.needsReview[robotId] === true
}

/** A robot of a branch failed on the commit the branch is at: the outgoing indicator gets a dot */
export function usePatronusNeedsAttention(): boolean {
  const key = useRepositoryKey()
  const robots = useStore(patronusStore, state => state.repositories[key]?.robots)
  const branches = useBranchSync(state => state.branches)
  return useMemo(() => Object.values(branches).some((branch) => {
    const localTip = branch.commits[0]?.hash
    const robot = findBranchRobot(robots, branch.name, localTip, branch.remoteStatus?.remoteTip ?? null)
    return robot !== undefined && needsPatronusAttention(robot, localTip)
  }), [robots, branches])
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
  for (const robot of incoming) {
    const known = previous[robot.id]
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
let windowFocused = typeof document === "undefined" || document.hasFocus()

/**
 * Stores an answer of Patronus: its robots (a full list replaces them) and its availability. A failure keeps the robots
 * shown: a failed refresh does not make them go away. Never throws; true when Patronus answered with robots.
 */
async function applyAnswer(key: string, request: ReturnType<typeof commands.getPatronusRobots>, replace: boolean): Promise<boolean> {
  let availability: PatronusAvailability
  try {
    const result = await request
    if (result.status === "ok" && result.data.availability.status === "available") {
      const robots = result.data.robots
      update(key, previous => ({ availability: result.data.availability, robots: mergeRobots(previous.robots, robots, replace), fetchedAt: Date.now() }))
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
      const answered = await applyAnswer(key, commands.getPatronusRobots({ repositoryPath, branchPrefix, since, background: current !== "manual" }), full)
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
function activeBranchRobots(key: string): KnownRobotStatus[] {
  const robots = patronusStore.getState().repositories[key]?.robots
  const known = new Map<string, KnownRobotStatus>()
  for (const branch of Object.values(branchSyncStore.getState().branches)) {
    const robot = findBranchRobot(robots, branch.name, branch.commits[0]?.hash, branch.remoteStatus?.remoteTip ?? null)
    if (robot !== undefined && isRobotActive(robot)) {
      known.set(robot.id, { id: robot.id, status: robot.status })
    }
  }
  return [...known.values()]
}

/**
 * Arms the poll timer when a branch has a running robot and the window is focused; `restart` replaces a pending timer
 * (after a poll), else a pending one is kept.
 */
function schedulePoll(restart: boolean): void {
  if (pollTimer !== undefined && !restart) {
    return
  }
  clearTimeout(pollTimer)
  pollTimer = undefined
  const key = currentKey()
  if (!windowFocused || key === null || activeBranchRobots(key).length === 0) {
    return
  }
  pollTimer = setTimeout(() => {
    pollTimer = undefined
    void pollPatronus()
  }, POLL_INTERVAL_MS)
}

/** Asks Patronus whether the running robots of the branches changed; only the changed ones come back */
export function pollPatronus(): Promise<void> {
  const key = currentKey()
  if (key === null) {
    return Promise.resolve()
  }
  const known = activeBranchRobots(key)
  if (known.length === 0) {
    return Promise.resolve()
  }
  const [repositoryPath, branchPrefix] = key.split("\n") as [string, string]
  return pollGates.refresh(key, "poll", 0, async () => {
    try {
      await applyAnswer(key, commands.pollPatronusRobots({ repositoryPath, branchPrefix, robots: known }), false)
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
  update(key, previous => ({ robots: mergeRobots(previous.robots, [robot], false) }))
  schedulePoll(false)
}

/**
 * Starts a Safe Push or a Dry Run of the pushed virtual branch (the caller pushes it first when the remote does not have
 * its tip, `justPushed`), with the merge request whose approvals its review check counts. Resolves to the robot, now
 * polled while it runs; throws what Patronus said.
 */
export async function startPatronusRobot(
  branchName: string,
  mode: PatronusPushMode,
  profileId: string | null,
  mergeRequest: PatronusMergeRequest | null,
  justPushed: boolean,
): Promise<PatronusRobot> {
  const result = await commands.startPatronusRobot({
    ...patronusRequest(),
    baselineBranch: branchSyncStore.getState().baselineBranch ?? "",
    branchName,
    mode,
    profileId,
    mergeRequest,
    justPushed,
  })
  if (result.status === "error") {
    throw new Error(result.error.message)
  }
  upsertPatronusRobot(result.data)
  return result.data
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

/** Nothing is polled while the window is not focused; on focus the running robots are asked for at once */
export function setPatronusWindowFocus(focused: boolean): void {
  windowFocused = focused
  if (!focused) {
    clearTimeout(pollTimer)
    pollTimer = undefined
    return
  }
  void pollPatronus().then(() => schedulePoll(false))
}

/** Forgets the gates, the timer and the data (tests) */
export function resetPatronus(): void {
  problemsInFlight.clear()
  listGates.reset()
  pollGates.reset()
  clearTimeout(pollTimer)
  pollTimer = undefined
  windowFocused = true
  patronusStore.setState({ repositories: {} })
}
