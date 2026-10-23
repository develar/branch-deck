import { z } from "zod"
import { useStore } from "zustand"
import { useShallow } from "zustand/react/shallow"
import type { LandedBranch, PatronusRobot, ReviewRequest, UpdateProjectResult } from "@/lib/bindings"
import { setErrorRecorder } from "@/lib/errors"
import { appliedUpdateToast, landedNews } from "@/lib/landed"
import { describePush } from "@/lib/push-summary"
import { buildRemoteMovedTexts } from "@/lib/remote-status"
import { describeRequestActivity } from "@/lib/review-requests"
import { describeUpdateState } from "@/lib/update-state"
import type { RemoteStatus } from "@/stores/branch-sync"
import { getSelectedProject, useSelectedProjectPath } from "@/stores/repository"
import { isMainWindow } from "@/stores/settings/app-store"
import { createPersistentStore } from "@/stores/settings/persistent-store"
import type { ArrayStoreState } from "@/stores/settings/persistent-store"

/*
 * Activity: what Branch Deck did in a repository (pushes, updates, landed reports, Patronus robots, history rewrites,
 * errors) and the review requests it told about, newest first. Toasts pass, the log stays: persisted by the main window, at most `MAX_ACTIVITY_ENTRIES` and
 * `MAX_ACTIVITY_AGE_MS`. See spec/repository/activity.spec.md.
 */

/** At most this many entries over all repositories; the oldest go first */
export const MAX_ACTIVITY_ENTRIES = 200
/** Entries older than this go */
export const MAX_ACTIVITY_AGE_MS = 30 * 24 * 60 * 60 * 1000
/** How long the Undo of a rewrite lasts, the toast's and the one in Activity alike */
export const UNDO_WINDOW_MS = 10_000

const ActivityKindSchema = z.enum(["push", "update", "landed", "patronus", "rewrite", "review", "error"])

export type ActivityKind = z.infer<typeof ActivityKindSchema>

const DroppedCommitSchema = z.object({
  id: z.string(),
  subject: z.string(),
  author: z.string(),
  authorTime: z.number(),
})

const LandedBranchSchema = z.object({
  name: z.string().nullable(),
  commits: z.array(
    z.object({
      id: z.string(),
      subject: z.string(),
      upstream: z.object({ id: z.string(), author: z.string(), committer: z.string(), time: z.number() }).nullable(),
    }),
  ),
  remaining: z.number(),
  landedAt: z.number(),
})

/** What an entry expands to, or opens */
const ActivityDetailsSchema = z.discriminatedUnion("type", [
  /** The remote commits a force push dropped, the newest first, and how many more */
  z.object({ type: z.literal("dropped"), commits: z.array(DroppedCommitSchema), more: z.number() }),
  /** The landed report, as the island showed it */
  z.object({ type: z.literal("landed"), upstream: z.string(), report: z.array(LandedBranchSchema) }),
  /** The robot page in the Patronus web UI */
  z.object({ type: z.literal("robot"), url: z.string() }),
  /** A pull request on GitHub: one that waits for the user's review */
  z.object({ type: z.literal("pullRequest"), url: z.string() }),
])

export type ActivityDetails = z.infer<typeof ActivityDetailsSchema>

const ActivityEntrySchema = z.object({
  id: z.string(),
  repositoryPath: z.string(),
  /** Milliseconds since the epoch */
  time: z.number(),
  kind: ActivityKindSchema,
  title: z.string(),
  description: z.string().optional(),
  /** A refused update, a push that dropped remote commits or found the remote moved */
  warning: z.boolean().optional(),
  details: ActivityDetailsSchema.optional(),
  /**
   * The Undo of a rewrite: it works until `until` (ms) in the session that did the rewrite, as the toast's; `undone`
   * once the user undid it, from the toast or from Activity
   */
  undo: z.object({ until: z.number(), undone: z.boolean().optional() }).optional(),
  /** The operation of a failed command (`CommandError.operation_id`): the id its lines in the app's log carry */
  operationId: z.string().optional(),
})

export type ActivityEntry = z.infer<typeof ActivityEntrySchema>

// an entry that doesn't parse (a kind of a newer version) drops the log, as any invalid stored value: it is a record
const ActivityLogSchema = z.array(ActivityEntrySchema).default([])

export const activityStore = createPersistentStore("activity", { schema: ActivityLogSchema, isArray: true, isMainOnly: true })

/** The undo functions of this session's rewrites, by entry id, while their window lasts */
const undoActions = new Map<string, () => void>()

export interface ActivityInput extends Omit<ActivityEntry, "id" | "repositoryPath" | "time"> {
  /** The selected repository by default */
  repositoryPath?: string
}

let lastId = 0

function nextId(time: number): string {
  lastId = Math.max(lastId + 1, time)
  return lastId.toString(36)
}

/** The entries within the age and the count limits, newest first */
export function pruneActivity(items: readonly ActivityEntry[], now: number): ActivityEntry[] {
  return items.filter(entry => now - entry.time < MAX_ACTIVITY_AGE_MS).slice(0, MAX_ACTIVITY_ENTRIES)
}

/**
 * Adds an entry to the log of its repository (the selected one by default) and returns it; null in a sub-window, which
 * doesn't own the log (the Activity window is told it over its link), and without a repository.
 */
export function recordActivity(input: ActivityInput): ActivityEntry | null {
  const { repositoryPath = getSelectedProject()?.path, ...rest } = input
  if (!isMainWindow || repositoryPath === undefined || repositoryPath === "") {
    return null
  }
  const time = Date.now()
  const entry: ActivityEntry = { id: nextId(time), repositoryPath, time, ...rest }
  activityStore.setState(state => ({ items: pruneActivity([entry, ...state.items], time) }))
  return entry
}

function updateEntry(id: string, update: (entry: ActivityEntry) => ActivityEntry): void {
  activityStore.setState(state => ({ items: state.items.map(entry => (entry.id === id ? update(entry) : entry)) }))
}

/** Forgets the log of a repository */
export function clearActivity(repositoryPath: string): void {
  const { items } = activityStore.getState()
  for (const entry of items) {
    if (entry.repositoryPath === repositoryPath) {
      undoActions.delete(entry.id)
    }
  }
  activityStore.setState({ items: items.filter(entry => entry.repositoryPath !== repositoryPath) })
}

// ---- what is recorded ----

/**
 * A history rewrite with an Undo (squash, fixups, move, rename, stack, unstack, ungroup). Returns the undo to run from
 * the toast: whichever Undo comes first, the toast's or the one in Activity, runs `undo`, and a second one while it runs
 * joins it. `undo` resolves to whether it undid the rewrite: only then is the entry marked undone and its Undo gone; an
 * undo that failed (an error toast said why) is offered again, in its window.
 */
export function recordRewrite(title: string, undo: () => Promise<boolean>, description?: string): () => void {
  const entry = recordActivity({ kind: "rewrite", title, description, undo: { until: Date.now() + UNDO_WINDOW_MS } })
  let state: "offered" | "running" | "done" = "offered"
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
    if (undone && entry !== null) {
      undoActions.delete(entry.id)
      updateEntry(entry.id, it => (it.undo === undefined ? it : { ...it, undo: { ...it.undo, undone: true } }))
    }
  }
  if (entry !== null) {
    undoActions.set(entry.id, run)
    setTimeout(() => undoActions.delete(entry.id), UNDO_WINDOW_MS)
  }
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
 * A push in the words of its toast (`describePush`): "Pushed feature-auth" / "3 commits"; a force push that dropped
 * remote commits keeps them. `before` is the remote status the push was made against, `moved` the fetched status when
 * the remote moved and nothing was pushed.
 */
export function recordPush(repositoryPath: string, branchName: string, before: RemoteStatus | null, moved: RemoteStatus | null): void {
  if (moved !== null) {
    const texts = buildRemoteMovedTexts(branchName, moved)
    recordActivity({ repositoryPath, kind: "push", title: texts.title, description: texts.description, warning: true })
    return
  }
  const news = describePush(branchName, before)
  const dropped = before?.droppedCommits ?? []
  const details: ActivityDetails | undefined =
    news.warning !== true || before === null || dropped.length === 0
      ? undefined
      : { type: "dropped", commits: [...dropped], more: Math.max(0, before.commitsBehind - dropped.length) }
  recordActivity({ repositoryPath, kind: "push", ...news, details })
}

/** An Update Project: applied (with what landed), or refused for a conflict or uncommitted changes; up to date is no news */
export function recordUpdate(repositoryPath: string, { check, applied, landed }: UpdateProjectResult): void {
  const status = check.state.status
  if (status === "clean" && applied !== null) {
    const details: ActivityDetails | undefined = landed.length === 0 ? undefined : { type: "landed", upstream: check.upstream, report: landed }
    recordActivity({ repositoryPath, kind: "update", ...appliedUpdateToast(check, applied, landed), details })
  } else if (status === "conflict" || status === "blocked") {
    const description = `${describeUpdateState(check.state, check.upstream).summary}. Nothing was changed.`
    recordActivity({ repositoryPath, kind: "update", title: `Didn't update ${check.branch}`, description, warning: true })
  }
}

/** The landed commits of the entries of a repository: a report is recorded once, by the update or by the sync after one */
function recordedLandedCommits(repositoryPath: string): Set<string> {
  const ids = new Set<string>()
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
  const news = landedNews(found, upstream)
  if (news === null) {
    return
  }
  recordActivity({
    repositoryPath,
    kind: "landed",
    title: news.title,
    description: news.description === undefined ? "Found by a sync" : `${news.description} · found by a sync`,
    details: { type: "landed", upstream, report: found },
  })
}

/**
 * A robot started, restarted, canceled or merged, in the words of its toast: "Safe Push started" / "feature-auth →
 * master · profile Large Scale Change" (`describeRobot`), "feature-auth merged into master" / "Safe Push"; with the
 * robot page.
 */
export function recordRobot(title: string, description: string, robot: PatronusRobot, repositoryPath?: string): void {
  recordActivity({ repositoryPath, kind: "patronus", title, description, details: { type: "robot", url: robot.url } })
}

/**
 * A pull request that waits for the user's review, told by a toast or an OS notification: "alice requested your review
 * on #123" / its title, with the pull request on GitHub
 */
export function recordReviewRequest(repositoryPath: string, request: ReviewRequest): void {
  recordActivity({ repositoryPath, kind: "review", ...describeRequestActivity(request), details: { type: "pullRequest", url: request.url } })
}

/** A review the user submitted from the app: "Approved #123" / its title, with the pull request on GitHub */
export function recordReview(repositoryPath: string, title: string, request: ReviewRequest): void {
  recordActivity({ repositoryPath, kind: "review", title, description: request.title, details: { type: "pullRequest", url: request.url } })
}

/** An approval of the user's pull request made a branch ready: the words of its toast (`describeApprovalNews`) */
export function recordApproval(repositoryPath: string, title: string, description: string, url: string): void {
  recordActivity({ repositoryPath, kind: "review", title, description, details: { type: "pullRequest", url } })
}

/** A pull request the app created: "Created pull request #123" / the reviewers and the assignee, with it on GitHub */
export function recordPullRequestCreated(title: string, description: string, url: string, warning: boolean, repositoryPath?: string): void {
  recordActivity({ kind: "review", title, description, ...(warning && { warning }), details: { type: "pullRequest", url }, repositoryPath })
}

/**
 * A pull request the user closed in the app: "Closed pull request #12" / "#12 no longer matches the air stack: auth
 * landed", with it on GitHub
 */
export function recordPullRequestClosed(title: string, description: string, url: string): void {
  recordActivity({ kind: "review", title, description, details: { type: "pullRequest", url } })
}

/**
 * A pull request the app retargeted from a virtual branch to the baseline: "Retargeted #8 to master" / "login · it went
 * into auth", with it on GitHub
 */
export function recordRetarget(title: string, description: string, url: string): void {
  recordActivity({ kind: "review", title, description, details: { type: "pullRequest", url } })
}

// every error the user was told about (`notifyError`); errors.ts can't import the store, which imports errors.ts
setErrorRecorder((title, description, operationId) => recordActivity({ kind: "error", title, description, ...(operationId !== undefined && { operationId }) }))

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

/** The log of the selected repository, newest first */
export function useActivity(): readonly ActivityEntry[] {
  const repositoryPath = useSelectedProjectPath()
  const entries = useStore(
    activityStore,
    useShallow((state: ArrayStoreState<ActivityEntry>) => selectActivity(state, repositoryPath)),
  )
  return entries.length === 0 ? noEntries : entries
}

/** The log of a repository, newest first */
export function selectActivity(state: ArrayStoreState<ActivityEntry>, repositoryPath: string | undefined): ActivityEntry[] {
  return state.items.filter(entry => entry.repositoryPath === repositoryPath)
}

/** Whether the selected repository has a log: the header hides the Activity button without one */
export function useHasActivity(): boolean {
  const repositoryPath = useSelectedProjectPath()
  return useStore(activityStore, state => state.items.some(entry => entry.repositoryPath === repositoryPath))
}
