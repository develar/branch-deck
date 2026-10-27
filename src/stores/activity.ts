import { useStore } from "zustand"
import { useShallow } from "zustand/react/shallow"
import { createStore } from "zustand/vanilla"
import type { ActivityLog, DroppedCommit, LandedBranch, PageActivity, PatronusRobot, ReviewRequest, UpdateProjectResult } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { logError, setErrorRecorder } from "@/lib/errors"
import { appliedUpdateToast, foundLandedNews } from "@/lib/landed"
import type { GitHubRepository } from "@/lib/pull-request"
import { describeRequestActivity } from "@/lib/review-requests"
import { describeUpdateState } from "@/lib/update-state"
import { isMainWindow } from "@/lib/window-label"
import { presentActivity } from "@/presenters/activity"
import { getSelectedProject, useSelectedProjectPath } from "@/stores/repository"

/*
 * Activity: what Branch Deck did in a repository, newest first, read from the journals (spec/repository/activity.spec.md).
 * The operations of the remote, of the app and of `bd`, are the journal of their repository; what only the page knows
 * (an error it told, an update, what landed, a rewrite, a review request, an approval, the news of a robot) it records
 * in the journal of the app data directory (`record_activity`). The main window reads the log (`read_activity`) when
 * it starts, after each write and operation of its own (`refreshActivity`), and when a journal changes
 * (`activity-changed`); `presentActivity` words the entries. The Undo of a rewrite lives here, in memory, by entry id.
 */

/** How long the Undo of a rewrite lasts, the toast's and the one in Activity alike */
export const UNDO_WINDOW_MS = 10_000

export type ActivityKind = "push" | "update" | "landed" | "patronus" | "rewrite" | "review" | "error"

/** What an entry expands to, or opens */
export type ActivityDetails =
  /** The remote commits a force push dropped, the newest first, and how many more */
  | { readonly type: "dropped"; readonly commits: readonly DroppedCommit[]; readonly more: number }
  /** The landed report, as the island showed it */
  | { readonly type: "landed"; readonly upstream: string; readonly report: readonly LandedBranch[] }
  /** The robot page in the Patronus web UI */
  | { readonly type: "robot"; readonly url: string }
  /** A pull request on GitHub */
  | { readonly type: "pullRequest"; readonly url: string }

export interface ActivityEntry {
  readonly id: string
  readonly repositoryPath: string
  /** Milliseconds since the epoch */
  readonly time: number
  readonly kind: ActivityKind
  readonly title: string
  readonly description?: string
  /** A refused update, a push that dropped remote commits or found the remote moved, a failing robot */
  readonly warning?: boolean
  readonly details?: ActivityDetails
  /**
   * The Undo of a rewrite of this session: it works until `until` (ms), as the toast's; `undone` once the user undid it,
   * from the toast or from Activity
   */
  readonly undo?: { readonly until: number; readonly undone?: boolean }
  /** The operation of the entry: the id of its lines in the app's log (an error a command named, an operation of the remote) */
  readonly operationId?: string
}

/** The Undo of a rewrite of this session */
interface UndoState {
  readonly until: number
  readonly undone: boolean
}

interface ActivityState {
  /** The log as the last read answered it; null before the first */
  readonly log: ActivityLog | null
  /** The entries of every repository, newest first, in their words */
  readonly items: readonly ActivityEntry[]
  /** The Undo of the rewrites of this session, by entry id */
  readonly undo: Readonly<Record<string, UndoState>>
}

export const activityStore = createStore<ActivityState>(() => ({ log: null, items: [], undo: {} }))

/** The GitHub repository the pull requests of the entries are linked in; set by `startActivityReads` */
let gitHubRepositoryOf: () => GitHubRepository | null = () => null

/** The entries of `log` in their words, with the Undo of this session's rewrites */
function itemsOf(log: ActivityLog | null, undo: Readonly<Record<string, UndoState>>): ActivityEntry[] {
  if (log === null) {
    return []
  }
  return presentActivity(log, gitHubRepositoryOf()).map(entry => {
    const state = undo[entry.id]
    return state === undefined ? entry : { ...entry, undo: { until: state.until, ...(state.undone && { undone: true }) } }
  })
}

function setLog(log: ActivityLog): void {
  activityStore.setState(state => ({ log, items: itemsOf(log, state.undo) }))
}

function setUndo(id: string, undo: UndoState): void {
  activityStore.setState(state => {
    const next = { ...state.undo, [id]: undo }
    return { undo: next, items: itemsOf(state.log, next) }
  })
}

/** The read in flight, and whether another one was asked meanwhile: reads run one at a time, the last one wins */
let reading: Promise<void> | null = null
let readAgain = false

/** Reads the log again (`read_activity`); resolves once the store has a read made after the call. Main window only */
export function refreshActivity(): Promise<void> {
  if (!isMainWindow) {
    return Promise.resolve()
  }
  if (reading !== null) {
    readAgain = true
    return reading
  }
  const read = async (): Promise<void> => {
    do {
      readAgain = false
      const answer = await commands.readActivity()
      if (answer.status === "ok") {
        setLog(answer.data)
      } else {
        logError("Cannot read Activity", answer.error)
      }
    } while (readAgain)
  }
  reading = read().finally(() => {
    reading = null
  })
  return reading
}

/**
 * Reads the log now and links its pull requests in the GitHub repository `gitHubRepository` answers, again when
 * `subscribe` says it changed; returns the unsubscribe. The main window's (`useActivityWindowLink`)
 */
export function startActivityReads(gitHubRepository: () => GitHubRepository | null, subscribe: (changed: () => void) => () => void): () => void {
  gitHubRepositoryOf = gitHubRepository
  let shown = gitHubRepository()
  const unsubscribe = subscribe(() => {
    const now = gitHubRepository()
    if (now?.owner !== shown?.owner || now?.name !== shown?.name) {
      shown = now
      activityStore.setState(state => ({ items: itemsOf(state.log, state.undo) }))
    }
  })
  void refreshActivity()
  return unsubscribe
}

// ---- what the page records ----

/** What the page records of a repository (`PageActivity` of `record_activity`) */
export interface PageInput {
  readonly kind: PageActivity["kind"]
  readonly title: string
  readonly description?: string
  readonly warning?: boolean
  readonly details?: Exclude<ActivityDetails, { readonly type: "dropped" }>
  readonly operationId?: string
}

/** The details of `input` as the journal of the page keeps them */
function pageDetails(details: PageInput["details"]): PageActivity["details"] {
  if (details === undefined) {
    return null
  }
  return details.type === "landed" ? { type: "landed", upstream: details.upstream, report: [...details.report] } : { ...details }
}

/**
 * Records what the page told of `repositoryPath` in the journal of the page (`record_activity`); the entry shows at once.
 * Resolves to its id; null in a sub-window, which records nothing (the main window does), and when the write failed
 * (logged, never toasted: a toast would record its error again)
 */
export async function recordActivity(repositoryPath: string, input: PageInput): Promise<string | null> {
  if (!isMainWindow || repositoryPath === "") {
    return null
  }
  const activity: PageActivity = {
    repositoryPath,
    kind: input.kind,
    title: input.title,
    description: input.description ?? null,
    warning: input.warning === true,
    details: pageDetails(input.details),
    operationId: input.operationId ?? null,
  }
  const recorded = await commands.recordActivity(activity)
  if (recorded.status === "error") {
    logError("Cannot record in Activity", recorded.error)
    return null
  }
  activityStore.setState(state => {
    const log: ActivityLog = { journals: state.log?.journals ?? [], page: [...(state.log?.page ?? []), recorded.data] }
    return { log, items: itemsOf(log, state.undo) }
  })
  return recorded.data.id
}

/** The undo functions of this session's rewrites, by entry id, while their window lasts */
const undoActions = new Map<string, () => void>()

/**
 * A history rewrite of `repositoryPath` with an Undo (squash, fixups, move, rename, stack, unstack, ungroup). Returns
 * the undo to run from the toast: whichever Undo comes first, the toast's or the one in Activity, runs `undo`, and a
 * second one while it runs joins it. `undo` resolves to whether it undid the rewrite: only then is the entry marked
 * undone and its Undo gone; an undo that failed (an error toast said why) is offered again, in its window.
 */
export function recordRewrite(repositoryPath: string, title: string, undo: () => Promise<boolean>, description?: string): () => void {
  const until = Date.now() + UNDO_WINDOW_MS
  let state: "offered" | "running" | "done" = "offered"
  let entryId: string | null = null
  const run = () => {
    if (state !== "offered") {
      return
    }
    state = "running"
    void undo().then(
      undone => finish(undone),
      () => finish(false),
    )
  }
  const finish = (undone: boolean) => {
    state = undone ? "done" : "offered"
    if (undone && entryId !== null) {
      undoActions.delete(entryId)
      setUndo(entryId, { until, undone: true })
    }
  }
  void recordActivity(repositoryPath, { kind: "rewrite", title, description }).then(id => {
    if (id === null) {
      return
    }
    entryId = id
    if (state === "done") {
      setUndo(id, { until, undone: true })
      return
    }
    undoActions.set(id, run)
    setUndo(id, { until, undone: false })
    setTimeout(() => undoActions.delete(id), Math.max(0, until - Date.now()))
  })
  return run
}

/**
 * Whether the Undo of the entry is still offered: in its window, not used yet; the Activity window, which doesn't hold
 * the undo itself, asks the main window, which checks `canUndoActivity`
 */
export function isUndoOpen(entry: ActivityEntry, now: number = Date.now()): boolean {
  return entry.undo !== undefined && entry.undo.undone !== true && now < entry.undo.until
}

/** Whether the Undo of the entry still works: in its window, not used yet, in the session that did the rewrite */
export function canUndoActivity(entry: ActivityEntry, now: number = Date.now()): boolean {
  return isUndoOpen(entry, now) && undoActions.has(entry.id)
}

/** The Undo of a rewrite from Activity */
export function undoActivity(entry: ActivityEntry): void {
  if (canUndoActivity(entry)) {
    undoActions.get(entry.id)?.()
  }
}

/**
 * Forgets the log of a repository (Clear Log): the journal of its repository and the entries of the page of it
 * (`clear_activity`), then reads the log again; resolves to the error of a failed clear
 */
export async function clearActivity(repositoryPath: string): Promise<string | null> {
  const cleared = await commands.clearActivity({ repositoryPath })
  // a still undismissed report is recorded again by the next sync
  landedRecorded.delete(repositoryPath)
  for (const entry of activityStore.getState().items) {
    if (entry.repositoryPath === repositoryPath) {
      undoActions.delete(entry.id)
    }
  }
  await refreshActivity()
  return cleared.status === "error" ? cleared.error.message : null
}

/**
 * The landed commits recorded by each repository in this session, also while their record is on its way: the sync
 * right after an update records none of them again
 */
const landedRecorded = new Map<string, Set<string>>()

/** `report` is recorded in `repositoryPath` */
function noteLanded(repositoryPath: string, report: readonly LandedBranch[]): void {
  const ids = landedRecorded.get(repositoryPath) ?? new Set<string>()
  for (const branch of report) {
    for (const commit of branch.commits) {
      ids.add(commit.id)
    }
  }
  landedRecorded.set(repositoryPath, ids)
}

/** An Update Project: applied (with what landed), or refused for a conflict or uncommitted changes; up to date is no news */
export function recordUpdate(repositoryPath: string, { check, applied, landed }: UpdateProjectResult): void {
  const status = check.state.status
  if (status === "clean" && applied !== null) {
    noteLanded(repositoryPath, landed)
    const details: PageInput["details"] = landed.length === 0 ? undefined : { type: "landed", upstream: check.upstream, report: landed }
    void recordActivity(repositoryPath, { kind: "update", ...appliedUpdateToast(check, applied, landed), details })
  } else if (status === "conflict" || status === "blocked") {
    const description = `${describeUpdateState(check.state, check.upstream).summary}. Nothing was changed.`
    void recordActivity(repositoryPath, { kind: "update", title: `Didn't update ${check.branch}`, description, warning: true })
  }
}

/** The landed commits of the entries of a repository: a report is recorded once, by the update or by the sync after one */
function recordedLandedCommits(repositoryPath: string): Set<string> {
  const ids = new Set<string>(landedRecorded.get(repositoryPath))
  for (const entry of activityStore.getState().items) {
    if (entry.repositoryPath === repositoryPath && entry.details?.type === "landed") {
      for (const branch of entry.details.report) {
        for (const commit of branch.commits) {
          ids.add(commit.id)
        }
      }
    }
  }
  return ids
}

/**
 * The commits of the report of a sync that no entry has yet: an update made outside Branch Deck (the IDE, `git pull`),
 * found by the sync after it. An update made here recorded its report already.
 */
export function recordLanded(repositoryPath: string, report: readonly LandedBranch[], upstream: string): void {
  if (report.length === 0 || !isMainWindow) {
    return
  }
  const recorded = recordedLandedCommits(repositoryPath)
  const found = report.map(branch => ({ ...branch, commits: branch.commits.filter(commit => !recorded.has(commit.id)) })).filter(branch => branch.commits.length > 0)
  const news = foundLandedNews(found, upstream)
  if (news === null) {
    return
  }
  noteLanded(repositoryPath, found)
  void recordActivity(repositoryPath, { kind: "landed", ...news, details: { type: "landed", upstream, report: found } })
}

/**
 * News of a robot the session told: it merged ("auth merged into master" / "Safe Push"), failed or is failing, in the
 * words of its toast, with the robot page. A robot the user started, restarted or canceled is an operation of the
 * journal of its repository instead
 */
export function recordRobotNews(repositoryPath: string, title: string, description: string, robot: PatronusRobot, warning = false): void {
  void recordActivity(repositoryPath, { kind: "patronus", title, description, ...(warning && { warning }), details: { type: "robot", url: robot.url } })
}

/**
 * A pull request that waits for the user's review, told by a toast or an OS notification: "alice requested your review
 * on #123" / its title, with the pull request on GitHub
 */
export function recordReviewRequest(repositoryPath: string, request: ReviewRequest): void {
  void recordActivity(repositoryPath, { kind: "review", ...describeRequestActivity(request), details: { type: "pullRequest", url: request.url } })
}

/** An approval of the user's pull request made a branch ready: the words of its toast (`describeApprovalNews`) */
export function recordApproval(repositoryPath: string, title: string, description: string, url: string): void {
  void recordActivity(repositoryPath, { kind: "review", title, description, details: { type: "pullRequest", url } })
}

// every error the user was told about (`notifyError`), in the repository the main window shows; errors.ts can't import
// the store, which imports errors.ts
setErrorRecorder((title, description, operationId) => {
  void recordActivity(getSelectedProject()?.path ?? "", { kind: "error", title, description, ...(operationId !== undefined && { operationId }) })
})

// ---- days ----

export interface ActivityDay {
  /** The start of the local day, ms */
  start: number
  /** "Today", "Yesterday" or the date */
  label: string
  entries: ActivityEntry[]
}

const dayFormatter = new Intl.DateTimeFormat(undefined, { dateStyle: "medium" })

function startOfDay(time: number): number {
  return new Date(time).setHours(0, 0, 0, 0)
}

/** The entries (newest first) by local day, newest first: "Today", "Yesterday", then the date */
export function groupActivityByDay(entries: readonly ActivityEntry[], now: number): ActivityDay[] {
  const today = startOfDay(now)
  // a millisecond before today is yesterday, whatever the length of the day
  const yesterday = startOfDay(today - 1)
  const days: ActivityDay[] = []
  for (const entry of entries) {
    const start = startOfDay(entry.time)
    const last = days.at(-1)
    if (last?.start === start) {
      last.entries.push(entry)
      continue
    }
    const label = start === today ? "Today" : start === yesterday ? "Yesterday" : dayFormatter.format(start)
    days.push({ start, label, entries: [entry] })
  }
  return days
}

// ---- hooks ----

const noEntries: readonly ActivityEntry[] = []

/** The log of a repository, newest first */
export function selectActivity(state: Pick<ActivityState, "items">, repositoryPath: string | undefined): ActivityEntry[] {
  return state.items.filter(entry => entry.repositoryPath === repositoryPath)
}

/** The log of the selected repository, newest first */
export function useActivity(): readonly ActivityEntry[] {
  const repositoryPath = useSelectedProjectPath()
  const entries = useStore(
    activityStore,
    useShallow((state: ActivityState) => selectActivity(state, repositoryPath)),
  )
  return entries.length === 0 ? noEntries : entries
}

/** Whether the selected repository has a log: the header hides the Activity button without one */
export function useHasActivity(): boolean {
  const repositoryPath = useSelectedProjectPath()
  return useStore(activityStore, state => state.items.some(entry => entry.repositoryPath === repositoryPath))
}
