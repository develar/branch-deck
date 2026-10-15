import { createStore } from "zustand/vanilla"
import { useStore } from "zustand"
import type { KnownRobotStatus, PatronusAvailability, PatronusMergeRequest, PatronusPushMode, PatronusRobot, PatronusRobotStatus } from "@/lib/bindings"
import { isPermissionGranted, requestPermission, sendNotification } from "@tauri-apps/plugin-notification"
import { toast } from "@/components/ui/toast"
import { commands } from "@/lib/bindings"
import {
  REVIEW_APPROVAL_SUBSYSTEM,
  findBranchRobot,
  findMergedRobots,
  formatMergedRobotTitle,
  formatPushMode,
  isPushingRobot,
  isRobotActive,
  openRobot,
} from "@/lib/patronus"
import type { RefreshReason } from "@/lib/refresh-gate"
import { createRefreshGates } from "@/lib/refresh-gate"
import { recordRobot } from "@/stores/activity"
import { branchSyncStore, useBranchRemoteTip, useBranchTip } from "@/stores/branch-sync"
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
   * `owner/name` of the GitHub repository the branches are mirrored to (`branchdeck.githubRepository`), null without
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

/** Stores robots of an answer (with `patch`), and tells about the robots that merged since the last answer */
function storeRobots(key: string, incoming: readonly PatronusRobot[], replace: boolean, patch: Partial<RepositoryPatronus>): void {
  const previous = patronusStore.getState().repositories[key]?.robots ?? EMPTY.robots
  const robots = mergeRobots(previous, incoming, replace)
  update(key, () => ({ ...patch, robots }))
  if (robots !== previous) {
    for (const robot of findMergedRobots(previous, robots)) {
      announceMerge(key, robot)
    }
  }
}

// ---- merges ----

/** The merged robots already told about, once each */
const announcedMerges = new Set<string>()
/** Merges that happened while the window was not focused: their toasts show on focus */
let pendingMergeToasts: { key: string, robot: PatronusRobot }[] = []
/** null until the first notification asks the OS */
let notificationsAllowed: boolean | null = null

/**
 * A robot merged a branch: a toast with Fetch, so the header learns that an update brings the branch in, and "Open in
 * Patronus". While the window is not focused, an OS notification says it at once, and the toast waits for the focus.
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
  recordRobot(formatMergedRobotTitle(robot), formatPushMode(robot.pushMode), robot, key.split("\n")[0])
  if (windowFocused) {
    showMergeToast(key, robot)
    return
  }
  pendingMergeToasts.push({ key, robot })
  void notifyOs(formatMergedRobotTitle(robot), `${formatPushMode(robot.pushMode)} · Fetch and update to pick it up`)
}

function showMergeToast(key: string, robot: PatronusRobot): void {
  const [repositoryPath] = key.split("\n") as [string, string]
  const id = toast.add({
    type: "success",
    title: formatMergedRobotTitle(robot),
    description: formatPushMode(robot.pushMode),
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

/** Asks for the permission on the first notification, never at startup; a notification is an extra, it never fails */
async function notifyOs(title: string, body: string): Promise<void> {
  try {
    if (notificationsAllowed === null) {
      notificationsAllowed = await isPermissionGranted() || await requestPermission() === "granted"
    }
    if (notificationsAllowed) {
      sendNotification({ title, body })
    }
  }
  catch {
    // no notification plugin (the E2E mock), or the OS refused
  }
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
 * Starts a Safe Push or a Dry Run of the pushed virtual branch (the caller pushes it first when the remote does not have
 * its tip), with the merge request whose approvals its review check counts. Resolves to the robot, now
 * polled while it runs; throws what Patronus said. The caller announces it (`announceRobot`, with the profile name).
 */
export async function startPatronusRobot(
  branchName: string,
  mode: PatronusPushMode,
  profileId: string | null,
  mergeRequest: PatronusMergeRequest | null,
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
  upsertPatronusRobot(result.data)
  return result.data
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

/**
 * Opens a Space merge request of the pushed branch into the target branch, so the code owners approve it there; resolves
 * to it, throws what Patronus said. The review checks of the branch ask again afterwards (the backend forgets them).
 */
export async function createPatronusMergeRequest(branchName: string): Promise<PatronusMergeRequest> {
  const result = await commands.createPatronusMergeRequest({
    ...patronusRequest(),
    baselineBranch: branchSyncStore.getState().baselineBranch ?? "",
    branchName,
    recheck: false,
  })
  if (result.status === "error") {
    throw new Error(result.error.message)
  }
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

/**
 * While the window is not focused, only a robot that pushes is polled, slowly; on focus the merges told meanwhile get
 * their toasts and the running robots are asked for at once
 */
export function setPatronusWindowFocus(focused: boolean): void {
  windowFocused = focused
  if (!focused) {
    schedulePoll(true)
    return
  }
  const toasts = pendingMergeToasts
  pendingMergeToasts = []
  for (const { key, robot } of toasts) {
    showMergeToast(key, robot)
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
  windowFocused = true
  announcedMerges.clear()
  pendingMergeToasts = []
  notificationsAllowed = null
  patronusStore.setState({ repositories: {} })
}
