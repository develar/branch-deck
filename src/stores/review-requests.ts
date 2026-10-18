import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import { toast } from "@/components/ui/toast"
import type { PullRequestAvailability, PullRequestReviewEvent, ReviewRequest } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { formatShortHash } from "@/lib/hash"
import { announce } from "@/lib/os-notification"
import { describeRobot, openRobotUrl } from "@/lib/patronus"
import { openPullRequestUrl } from "@/lib/pull-request"
import type { ReviewFeedback } from "@/lib/pull-request-review"
import { reviewToastActionLabel } from "@/lib/pull-request-review"
import type { RefreshReason } from "@/lib/refresh-gate"
import { createRefreshGates } from "@/lib/refresh-gate"
import type { ReviewIntent } from "@/lib/review-request-actions"
import { isApprovedCommit } from "@/lib/review-request-actions"
import { MIN_POLL_GAP_MS, POLL_INTERVAL_MS, describeNewRequests, findNewRequests, mergeReviewRequests, nextPollDelay, reviewRequestKey } from "@/lib/review-requests"
import { showRepository } from "@/lib/show-repository"
import { ACTION_TOAST_TIMEOUT_MS, showToast } from "@/lib/toast-message"
import { recordReview, recordReviewRequest, recordRobot } from "@/stores/activity"
import { startPatronusFromReviewedPullRequest, watchRobot } from "@/stores/patronus"
import { getGitHubRepository } from "@/stores/pull-requests"
import { getSelectedProject, useSelectedProjectPath } from "@/stores/repository"

/*
 * "Your Turn": the pull requests of others that wait for the user's review in the selected repository
 * (spec/repository/review-requests.spec.md). GitHub is polled every five minutes, focused or not; news is told once.
 */

/** Syncs closer together than this ask nothing more: the poll keeps the requests current */
export const MIN_SYNC_GAP_MS = 60_000
/** On window focus, requests older than this are asked for */
export const FOCUS_REFRESH_AGE_MS = 120_000

/** The review requests of one repository */
export interface RepositoryReviewRequests {
  /** null until the first answer */
  readonly availability: PullRequestAvailability | null
  /** Waiting the longest first; kept when a refresh fails */
  readonly requests: readonly ReviewRequest[]
  /** All that wait, also beyond the 50 GitHub returns */
  readonly totalCount: number
  /** `Date.now()` of the last answer, 0 = never */
  readonly fetchedAt: number
  /** `Date.now()` of the last answer with the requests, 0 = never: the age of the rows after a failure */
  readonly checkedAt: number
  readonly isRefreshing: boolean
  /** `reviewRequestKey`s of the requests that came after the first answer and were not opened yet */
  readonly unread: ReadonlySet<string>
  /** The reviews the user submitted from the app in this session, by pull request number */
  readonly reviewed: Readonly<Record<number, SubmittedReview>>
  /** What runs for a pull request now (a review being submitted, a Safe Push being asked for), by number */
  readonly busy: Readonly<Record<number, ReviewAction>>
}

/** A review the user submitted from the app: the review says it, and keeps the request once it left the list */
export interface SubmittedReview {
  readonly event: PullRequestReviewEvent
  /** The commit the review is on */
  readonly commitId: string
}

/** A write of the user to a pull request on review */
export type ReviewAction = PullRequestReviewEvent | "safePush"

export interface ReviewRequestsState {
  /** By repository path; in memory only, GitHub is asked again after a restart */
  readonly repositories: Readonly<Record<string, RepositoryReviewRequests>>
}

export const reviewRequestsStore = createStore<ReviewRequestsState>(() => ({ repositories: {} }))

const EMPTY: RepositoryReviewRequests = {
  availability: null,
  requests: [],
  totalCount: 0,
  fetchedAt: 0,
  checkedAt: 0,
  isRefreshing: false,
  unread: new Set(),
  reviewed: {},
  busy: {},
}

// ---- hooks ----

function useEntry<T>(select: (entry: RepositoryReviewRequests) => T): T {
  const repositoryPath = useSelectedProjectPath() ?? ""
  return useStore(reviewRequestsStore, state => select(state.repositories[repositoryPath] ?? EMPTY))
}

/** The requests of the selected repository, waiting the longest first; the same array while nothing changed */
export function useReviewRequests(): readonly ReviewRequest[] {
  return useEntry(entry => entry.requests)
}

export function useReviewRequestsAvailability(): PullRequestAvailability | null {
  return useEntry(entry => entry.availability)
}

export function useReviewRequestsTotalCount(): number {
  return useEntry(entry => entry.totalCount)
}

export function useReviewRequestsCheckedAt(): number {
  return useEntry(entry => entry.checkedAt)
}

export function useIsRefreshingReviewRequests(): boolean {
  return useEntry(entry => entry.isRefreshing)
}

export function useUnreadReviewRequests(): ReadonlySet<string> {
  return useEntry(entry => entry.unread)
}

/** What waits for the user's review, for the badge of the Dock */
export function useReviewRequestCount(): number {
  return useEntry(entry => (entry.availability?.status === "available" || entry.requests.length > 0 ? entry.totalCount : 0))
}

/** The review the user submitted on pull request `number` of the selected repository in this session, if any */
export function useSubmittedReview(number: number): SubmittedReview | undefined {
  return useEntry(entry => entry.reviewed[number])
}

/** What runs for pull request `number` of the selected repository now, if anything */
export function useReviewAction(number: number): ReviewAction | undefined {
  return useEntry(entry => entry.busy[number])
}

// ---- refresh ----

/**
 * A poll asks every five minutes (not closer than 4.5 min to the last request), a sync at most once a minute, a focus
 * when the requests are older than two minutes; the refresh button always, after the request in flight if any.
 */
const gates = createRefreshGates({
  isDue: (reason, sinceStart, sinceAnswer) => {
    switch (reason) {
      case "manual":
        return true
      case "poll":
        return sinceStart >= MIN_POLL_GAP_MS
      case "sync":
        return sinceStart >= MIN_SYNC_GAP_MS
      case "focus":
        return sinceStart >= MIN_SYNC_GAP_MS && sinceAnswer >= FOCUS_REFRESH_AGE_MS
      case "push":
        return false
    }
  },
  followUpReasons: new Set<RefreshReason>(["manual"]),
})

let pollTimer: ReturnType<typeof setTimeout> | undefined
/** Runs on every poll tick too (`onReviewRequestsPoll`) */
let pollListener: () => void = () => {}

/**
 * Runs `listener` on each poll tick (every five minutes, later after failures): the main window refreshes the user's
 * pull requests then while a row waits for an approval. Returns the unsubscribe.
 */
export function onReviewRequestsPoll(listener: () => void): () => void {
  pollListener = listener
  return () => {
    if (pollListener === listener) {
      pollListener = () => {}
    }
  }
}
/** The repository the last refresh asked for: another one is a switch */
let lastRepositoryPath: string | null = null
/** The repositories whose next answer tells nothing new: the first one after a switch */
const silentRepositories = new Set<string>()
/** The failed answers in a row, by repository */
const failures = new Map<string, number>()
/** No request before this (`Date.now()`) but the refresh button: GitHub's rate limit */
const retryAt = new Map<string, number>()
/** A pull request was opened on GitHub from the card: the next focus asks at once (an approved one leaves the list) */
let refreshOnFocus = false
/** The repositories whose review refs were pruned and none was written since */
const prunedRepositories = new Set<string>()

function currentRepositoryPath(): string | null {
  const path = getSelectedProject()?.path ?? ""
  return path === "" ? null : path
}

function update(repositoryPath: string, patch: Partial<RepositoryReviewRequests> | ((entry: RepositoryReviewRequests) => Partial<RepositoryReviewRequests>)): void {
  reviewRequestsStore.setState(state => {
    const entry = state.repositories[repositoryPath] ?? EMPTY
    return { repositories: { ...state.repositories, [repositoryPath]: { ...entry, ...(typeof patch === "function" ? patch(entry) : patch) } } }
  })
}

/**
 * Asks GitHub for the requests of the selected repository, if the reason allows it now (see the policy above). One
 * request per repository at a time; another repository than the last one cancels the poll of that one, and its answer
 * tells nothing new. Never fails: what went wrong is the availability, and the rows stay.
 */
export function refreshReviewRequests(reason: RefreshReason): Promise<void> {
  const repositoryPath = currentRepositoryPath()
  if (repositoryPath === null) {
    return Promise.resolve()
  }
  if (repositoryPath !== lastRepositoryPath) {
    if (lastRepositoryPath !== null) {
      silentRepositories.add(repositoryPath)
    }
    lastRepositoryPath = repositoryPath
    clearTimeout(pollTimer)
    pollTimer = undefined
  }
  if (reason === "focus" && refreshOnFocus) {
    refreshOnFocus = false
    reason = "manual"
  }
  const entry = reviewRequestsStore.getState().repositories[repositoryPath] ?? EMPTY
  if (reason !== "manual" && (entry.availability?.status === "notGitHub" || Date.now() < (retryAt.get(repositoryPath) ?? 0))) {
    return Promise.resolve()
  }
  return gates.refresh(repositoryPath, reason, entry.fetchedAt, async () => {
    update(repositoryPath, { isRefreshing: true })
    let availability: PullRequestAvailability
    try {
      const result = await commands.getReviewRequests({ repositoryPath, githubRepository: githubRepositoryOfPullRequests() })
      if (result.status === "ok") {
        availability = result.data.availability
        if (availability.status === "available") {
          applyRequests(repositoryPath, result.data.requests, result.data.totalCount)
        }
      } else {
        availability = { status: "error", message: result.error.message }
      }
    } catch (error) {
      availability = { status: "error", message: error instanceof Error ? error.message : String(error) }
    }
    applyAvailability(repositoryPath, availability)
  })
}

/** `owner/name` while GitHub lists the user's pull requests of the repository: the backend need not resolve it again */
function githubRepositoryOfPullRequests(): string | null {
  const repository = getGitHubRepository()
  return repository === null ? null : `${repository.owner}/${repository.name}`
}

/** Stores the requests of an answer and tells the new ones */
function applyRequests(repositoryPath: string, incoming: readonly ReviewRequest[], totalCount: number): void {
  const entry = reviewRequestsStore.getState().repositories[repositoryPath] ?? EMPTY
  const requests = mergeReviewRequests(entry.requests, incoming)
  const switched = silentRepositories.delete(repositoryPath)
  const news = findNewRequests(entry.checkedAt === 0 || switched ? null : entry.requests, requests)
  const keys = new Set(requests.map(reviewRequestKey))
  const unread = new Set([...entry.unread].filter(key => keys.has(key)))
  for (const request of news) {
    unread.add(reviewRequestKey(request))
  }
  const now = Date.now()
  update(repositoryPath, {
    requests,
    totalCount: Math.max(totalCount, requests.length),
    checkedAt: now,
    unread: unread.size === entry.unread.size && [...unread].every(key => entry.unread.has(key)) ? entry.unread : unread,
  })
  if (news.length > 0) {
    tellNews(repositoryPath, news)
  }
  pruneReviewRefs(repositoryPath)
}

/**
 * Stores the availability of an answer and arms the next poll: every five minutes, later after failures (`nextPollDelay`),
 * none without a token GitHub accepts or off GitHub. Signed out the rows go (the card hides); after an error, or
 * without access to the repository, they stay with the hint.
 */
function applyAvailability(repositoryPath: string, availability: PullRequestAvailability): void {
  const now = Date.now()
  const failed = availability.status === "error" || availability.status === "rateLimited"
  const failureCount = failed ? (failures.get(repositoryPath) ?? 0) + 1 : 0
  failures.set(repositoryPath, failureCount)
  const delay = nextPollDelay(availability, failureCount)
  if (availability.status === "rateLimited" && delay !== null) {
    retryAt.set(repositoryPath, now + delay)
  } else {
    retryAt.delete(repositoryPath)
  }
  const gone = availability.status === "notGitHub" || availability.status === "signedOut" || availability.status === "notAuthenticated"
  update(repositoryPath, {
    availability,
    fetchedAt: now,
    isRefreshing: false,
    ...(gone && { requests: EMPTY.requests, totalCount: 0, unread: EMPTY.unread }),
  })
  schedulePoll(repositoryPath, delay)
}

/** Arms the poll of the repository, replacing a pending one; null disarms it */
function schedulePoll(repositoryPath: string, delay: number | null): void {
  clearTimeout(pollTimer)
  pollTimer = undefined
  if (delay === null || repositoryPath !== currentRepositoryPath()) {
    return
  }
  pollTimer = setTimeout(() => {
    pollTimer = undefined
    if (repositoryPath !== currentRepositoryPath()) {
      return
    }
    // offline: nothing to ask, the next poll may be online
    if (typeof navigator !== "undefined" && navigator.onLine === false) {
      schedulePoll(repositoryPath, POLL_INTERVAL_MS)
      return
    }
    void refreshReviewRequests("poll")
    pollListener()
  }, delay)
}

// ---- news ----

/**
 * New requests: one toast "alice requested your review" (or "3 reviews requested") with "Review", which opens the first
 * one; in the background an OS notification and the toast on focus. Each is an Activity entry.
 */
function tellNews(repositoryPath: string, news: readonly ReviewRequest[]): void {
  for (const request of news) {
    recordReviewRequest(repositoryPath, request)
  }
  const { title, description } = describeNewRequests(news)
  const first = news[0]!
  announce("reviewRequests", { title, body: description }, () => {
    const id = toast.add({
      type: "info",
      title,
      description,
      timeout: ACTION_TOAST_TIMEOUT_MS,
      actionProps: {
        children: "Review",
        onClick: () => {
          toast.close(id)
          openReviewRequest(repositoryPath, first.number)
        },
      },
    })
  })
}

// ---- the review ----

/**
 * Opens the review of a request of the repository (it must be the selected one) in its window, marking it read.
 * `intent` is the write the row's menu asked for, or "review" (D on the row).
 */
export function openReviewRequest(repositoryPath: string, number: number, intent: ReviewIntent | null = null): void {
  if (repositoryPath !== currentRepositoryPath()) {
    return
  }
  const entry = reviewRequestsStore.getState().repositories[repositoryPath] ?? EMPTY
  const index = entry.requests.findIndex(it => it.number === number)
  if (index === -1) {
    return
  }
  const request = entry.requests[index]!
  markRead(repositoryPath, request)
  reviewWindow.open(repositoryPath, request, index, intent)
}

/**
 * Opens the pull request on GitHub from the card or the review: it is read then, and the next focus asks GitHub at once
 * (an approved pull request leaves the list)
 */
export function openRequestOnGitHub(request: ReviewRequest, path = ""): void {
  const repositoryPath = currentRepositoryPath()
  if (repositoryPath !== null) {
    markRead(repositoryPath, request)
  }
  refreshOnFocus = true
  openPullRequestUrl(`${request.url}${path}`)
}

// ---- reviews ----

/** "owner/name" of the repository the requests were read from: the review goes there */
function githubRepositoryOfRequests(repositoryPath: string): string | null {
  const availability = reviewRequestsStore.getState().repositories[repositoryPath]?.availability
  if (availability?.status === "available") {
    return `${availability.owner}/${availability.name}`
  }
  return repositoryPath === currentRepositoryPath() ? githubRepositoryOfPullRequests() : null
}

/**
 * The feedback of a write asked for in the main window: a toast whose action opens the pull request (it is read then)
 * or the robot, and the error toast
 */
export function mainReviewFeedback(request: ReviewRequest): ReviewFeedback {
  return {
    toast: ({ title, description, target }) =>
      showToast({
        title,
        description,
        action: { children: reviewToastActionLabel(target), onClick: () => (target.kind === "robot" ? openRobotUrl(target.url) : openRequestOnGitHub(request)) },
      }),
    error: notifyError,
  }
}

function setBusy(repositoryPath: string, number: number, action: ReviewAction | null): void {
  update(repositoryPath, entry => ({
    busy: action === null ? Object.fromEntries(Object.entries(entry.busy).filter(([key]) => key !== String(number))) : { ...entry.busy, [number]: action },
  }))
}

/** "Approved #123", "Requested changes on #123" */
export function describeReview(event: PullRequestReviewEvent, number: number): string {
  return event === "approve" ? `Approved #${number}` : `Requested changes on #${number}`
}

/** "Cannot approve pull request #123" */
function reviewFailure(event: PullRequestReviewEvent, number: number): string {
  return event === "approve" ? `Cannot approve pull request #${number}` : `Cannot request changes on pull request #${number}`
}

/**
 * Submits a review of the user on `commitId`, the commit the user reviewed (`submit_pull_request_review`), in the
 * repository whose request it is: an approval or a request for changes with `body`. Said by `feedback` (a toast with
 * "Open Pull Request", in the window that asked) and in Activity; GitHub is asked at once, so the reviewed request
 * leaves the list (the review keeps it). Resolves to false when GitHub refused (told).
 */
export async function submitReview(
  repositoryPath: string,
  request: ReviewRequest,
  event: PullRequestReviewEvent,
  commitId: string,
  body: string | null = null,
  feedback: ReviewFeedback = mainReviewFeedback(request),
): Promise<boolean> {
  const { number } = request
  setBusy(repositoryPath, number, event)
  try {
    const result = await commands.submitPullRequestReview({
      repositoryPath,
      githubRepository: githubRepositoryOfRequests(repositoryPath),
      pullRequestNumber: number,
      event,
      commitId,
      body,
    })
    if (result.status === "error") {
      feedback.error(reviewFailure(event, number), result.error.message)
      return false
    }
  } catch (error) {
    feedback.error(reviewFailure(event, number), error)
    return false
  } finally {
    setBusy(repositoryPath, number, null)
  }
  markRead(repositoryPath, request)
  update(repositoryPath, entry => ({ reviewed: { ...entry.reviewed, [number]: { event, commitId } } }))
  const title = describeReview(event, number)
  feedback.toast({ title, description: request.title, target: { kind: "pullRequest", url: request.url } })
  recordReview(repositoryPath, title, request)
  void refreshReviewRequests("manual")
  return true
}

/**
 * Approves `commitId` (unless the user approved it already) and starts a Safe Push of the pull request: the
 * `/patronus merge` comment, which deletes its branch once merged. Only the approved commit is merged: a pull request
 * that got commits since is refused before the comment. The 👍 of the bot is the start ("Safe Push started", with the
 * robot when the user's robots list it); a refusal says the bot's reason. Patronus is asked for the selected
 * repository: a request of another one switches the main window to it first (`showRepository`). Resolves to whether it
 * started.
 */
export async function approveAndSafePush(
  repositoryPath: string,
  request: ReviewRequest,
  commitId: string,
  feedback: ReviewFeedback = mainReviewFeedback(request),
): Promise<boolean> {
  const { number } = request
  // Patronus is asked for the repository the main window shows: it switches to the request's one first
  const notShown = repositoryPath === currentRepositoryPath() ? null : await showRepository(repositoryPath)
  if (notShown !== null) {
    feedback.error(SAFE_PUSH_FAILURE, notShown)
    return false
  }
  const entry = reviewRequestsStore.getState().repositories[repositoryPath] ?? EMPTY
  if (!isApprovedCommit(entry.reviewed[number], commitId) && !(await submitReview(repositoryPath, request, "approve", commitId, null, feedback))) {
    return false
  }
  const githubRepository = githubRepositoryOfRequests(repositoryPath)
  setBusy(repositoryPath, number, "safePush")
  try {
    const head = await commands.getPullRequestHead({ repositoryPath, githubRepository, pullRequestNumber: number })
    if (head.status === "error") {
      feedback.error(SAFE_PUSH_FAILURE, head.error.message)
      return false
    }
    if (head.data !== commitId) {
      feedback.error(SAFE_PUSH_FAILURE, `#${number} has new commits since you reviewed ${formatShortHash(commitId)}: review ${formatShortHash(head.data)} first`)
      return false
    }
    const robot = await startPatronusFromReviewedPullRequest(number, githubRepository)
    const title = "Safe Push started"
    if (robot === null) {
      // the robot is not the user's: the pull request tells how it goes
      feedback.toast({ title, description: `#${number} ${request.title}`, target: { kind: "pullRequest", url: request.url } })
      recordReview(repositoryPath, `${title} on #${number}`, request)
    } else {
      watchRobot(robot, `#${number} ${request.title}`)
      const description = describeRobot(robot)
      feedback.toast({ title, description, target: { kind: "robot", url: robot.url } })
      recordRobot(title, description, robot)
    }
    return true
  } catch (error) {
    feedback.error(SAFE_PUSH_FAILURE, error)
    return false
  } finally {
    setBusy(repositoryPath, number, null)
  }
}

const SAFE_PUSH_FAILURE = "Cannot start the Safe Push"

/** The request is read: opened in the app or on GitHub */
export function markReviewRequestRead(repositoryPath: string, request: ReviewRequest): void {
  markRead(repositoryPath, request)
}

function markRead(repositoryPath: string, request: ReviewRequest): void {
  const key = reviewRequestKey(request)
  update(repositoryPath, entry => {
    if (!entry.unread.has(key)) {
      return {}
    }
    const unread = new Set(entry.unread)
    unread.delete(key)
    return { unread }
  })
}

// ---- review refs ----

/** A review fetch wrote `refs/branch-deck/review/<n>/…` into the repository: the next prune deletes it */
export function noteReviewRefsWritten(repositoryPath: string): void {
  prunedRepositories.delete(repositoryPath)
}

/** The review window as this store sees it; set by its main-window side (`pull-request-review-window.ts`), which imports this store */
export interface ReviewWindowConnection {
  /** The request the window shows in the repository, whose refs stay; null when it shows none */
  readonly shownNumber: (repositoryPath: string) => number | null
  /** Shows the request in the window, opened or reused */
  readonly open: (repositoryPath: string, request: ReviewRequest, index: number, intent: ReviewIntent | null) => void
}

let reviewWindow: ReviewWindowConnection = { shownNumber: () => null, open: () => {} }

export function connectReviewWindow(connection: ReviewWindowConnection): void {
  reviewWindow = connection
}

/** The review window closed: the refs of the repository go */
export function pruneReviewRefsOf(repositoryPath: string): void {
  prunedRepositories.delete(repositoryPath)
  pruneReviewRefs(repositoryPath)
}

/**
 * Deletes the review refs of the repository but those of the request the review window shows (`prune_review_refs`,
 * one CAS transaction): after an answer with the requests (once per repository per start, then after a fetch wrote one)
 * and when the window closes; never after a failed answer. A failure is tried again the next time.
 */
function pruneReviewRefs(repositoryPath: string): void {
  if (prunedRepositories.has(repositoryPath)) {
    return
  }
  const inWindow = reviewWindow.shownNumber(repositoryPath)
  const keep = inWindow === null ? [] : [inWindow]
  // the refs of the window stay: its close prunes them
  if (keep.length === 0) {
    prunedRepositories.add(repositoryPath)
  }
  void commands.pruneReviewRefs({ repositoryPath, keep }).then(
    result => {
      if (result.status === "error") {
        prunedRepositories.delete(repositoryPath)
      }
    },
    () => prunedRepositories.delete(repositoryPath),
  )
}

/** Forgets the gates, the timer and the data (tests) */
export function resetReviewRequests(): void {
  gates.reset()
  clearTimeout(pollTimer)
  pollTimer = undefined
  lastRepositoryPath = null
  silentRepositories.clear()
  failures.clear()
  retryAt.clear()
  refreshOnFocus = false
  prunedRepositories.clear()
  reviewRequestsStore.setState({ repositories: {} })
}
