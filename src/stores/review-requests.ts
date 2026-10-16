import { createStore } from "zustand/vanilla"
import { useStore } from "zustand"
import { toast } from "@/components/ui/toast"
import type { PullRequestAvailability, ReviewRequest } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { announce } from "@/lib/os-notification"
import { openPullRequestUrl } from "@/lib/pull-request"
import type { RefreshReason } from "@/lib/refresh-gate"
import { createRefreshGates } from "@/lib/refresh-gate"
import {
  MIN_POLL_GAP_MS,
  POLL_INTERVAL_MS,
  describeNewRequests,
  findNewRequests,
  mergeReviewRequests,
  nextPollDelay,
  reviewRequestKey,
} from "@/lib/review-requests"
import { ACTION_TOAST_TIMEOUT_MS } from "@/lib/toast-message"
import { recordReviewRequest } from "@/stores/activity"
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
}

/** The pull request the review dialog shows */
export interface OpenReview {
  readonly repositoryPath: string
  readonly number: number
  /** The request as it was when it opened: it stays while the request is withdrawn */
  readonly request: ReviewRequest
}

export interface ReviewRequestsState {
  /** By repository path; in memory only, GitHub is asked again after a restart */
  readonly repositories: Readonly<Record<string, RepositoryReviewRequests>>
  readonly openReview: OpenReview | null
}

export const reviewRequestsStore = createStore<ReviewRequestsState>(() => ({ repositories: {}, openReview: null }))

const EMPTY: RepositoryReviewRequests = { availability: null, requests: [], totalCount: 0, fetchedAt: 0, checkedAt: 0, isRefreshing: false, unread: new Set() }

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
  return useEntry(entry => entry.availability?.status === "available" || entry.requests.length > 0 ? entry.totalCount : 0)
}

export function useOpenReview(): OpenReview | null {
  return useStore(reviewRequestsStore, state => state.openReview)
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
  reviewRequestsStore.setState((state) => {
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
      }
      else {
        availability = { status: "error", message: result.error.message }
      }
    }
    catch (error) {
      availability = { status: "error", message: error instanceof Error ? error.message : String(error) }
    }
    applyAvailability(repositoryPath, availability)
  })
}

/** `owner/name` while `gh` lists the user's pull requests of the repository: the backend need not resolve it again */
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
 * none without `gh`, logged out or off GitHub. Without `gh` the rows go (the card hides); after an error they stay.
 */
function applyAvailability(repositoryPath: string, availability: PullRequestAvailability): void {
  const now = Date.now()
  const failed = availability.status === "error" || availability.status === "rateLimited"
  const failureCount = failed ? (failures.get(repositoryPath) ?? 0) + 1 : 0
  failures.set(repositoryPath, failureCount)
  const delay = nextPollDelay(availability, failureCount)
  if (availability.status === "rateLimited" && delay !== null) {
    retryAt.set(repositoryPath, now + delay)
  }
  else {
    retryAt.delete(repositoryPath)
  }
  const gone = availability.status === "notGitHub" || availability.status === "ghMissing" || availability.status === "notAuthenticated"
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

// ---- the dialog ----

/** Opens the review dialog of a request of the repository (it must be the selected one), marking it read */
export function openReviewRequest(repositoryPath: string, number: number): void {
  if (repositoryPath !== currentRepositoryPath()) {
    return
  }
  const entry = reviewRequestsStore.getState().repositories[repositoryPath] ?? EMPTY
  const request = entry.requests.find(it => it.number === number)
  if (request === undefined) {
    return
  }
  markRead(repositoryPath, request)
  reviewRequestsStore.setState({ openReview: { repositoryPath, number, request } })
}

/** Closes the review dialog; its review refs go (`pruneReviewRefs`) */
export function closeReviewRequest(): void {
  const open = reviewRequestsStore.getState().openReview
  if (open === null) {
    return
  }
  reviewRequestsStore.setState({ openReview: null })
  pruneReviewRefs(open.repositoryPath)
}

/**
 * Opens the pull request on GitHub from the card or the dialog: it is read then, and the next focus asks GitHub at once
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

function markRead(repositoryPath: string, request: ReviewRequest): void {
  const key = reviewRequestKey(request)
  update(repositoryPath, (entry) => {
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

/**
 * Deletes the review refs of the repository but those of the open dialog (`prune_review_refs`, one CAS transaction):
 * after an answer with the requests (once per repository per start, then after a fetch wrote one) and when the dialog
 * closes; never after a failed answer. A failure is tried again the next time.
 */
function pruneReviewRefs(repositoryPath: string): void {
  if (prunedRepositories.has(repositoryPath)) {
    return
  }
  const open = reviewRequestsStore.getState().openReview
  const keep = open?.repositoryPath === repositoryPath ? [open.number] : []
  // the refs of the open dialog stay: its close prunes them
  if (keep.length === 0) {
    prunedRepositories.add(repositoryPath)
  }
  void commands.pruneReviewRefs({ repositoryPath, keep }).then((result) => {
    if (result.status === "error") {
      prunedRepositories.delete(repositoryPath)
    }
  }, () => prunedRepositories.delete(repositoryPath))
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
  reviewRequestsStore.setState({ repositories: {}, openReview: null })
}
