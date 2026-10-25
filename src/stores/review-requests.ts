import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import type { FactsChanges, FactsView, News, PullRequestAvailability, PullRequestReviewEvent, ReviewRequest } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { closeEventToasts, showEventToast, toastKeys } from "@/lib/event-toast"
import { formatShortHash } from "@/lib/hash"
import { announce } from "@/lib/os-notification"
import { describeRobot, openRobotUrl } from "@/lib/patronus"
import { openPullRequestUrl } from "@/lib/pull-request"
import type { ReviewFeedback } from "@/lib/pull-request-review"
import { reviewToastActionLabel } from "@/lib/pull-request-review"
import type { ReviewIntent } from "@/lib/review-request-actions"
import { isApprovedCommit } from "@/lib/review-request-actions"
import { describeNewRequests, mergeReviewRequests, reviewRequestKey } from "@/lib/review-requests"
import { showRepository } from "@/lib/show-repository"
import { refreshSessionFacts } from "@/model/store"
import { recordReview, recordReviewRequest, recordRobot } from "@/stores/activity"
import type { Operations } from "@/stores/operations"
import { operationsStore, runOperation } from "@/stores/operations"
import { startPatronusFromReviewedPullRequest, watchRobot } from "@/stores/patronus"
import { getGitHubRepository } from "@/stores/pull-requests"
import { getSelectedProject, repositoryStore, useSelectedProjectPath } from "@/stores/repository"

/*
 * "Your Turn": the pull requests of others that wait for the user's review in the selected repository
 * (spec/repository/review-requests.spec.md). The session of the repository lists them (after a sync, on a focus, every
 * five minutes focused or not, by the refresh button: `remote_facts::scheduler`) and tells which are new (its news,
 * `reviewRequested`); the store keeps them as its view has them (`applyReviewRequestsFacts`), with what is the page's:
 * which are unread, the reviews the user submitted from the app, the toasts of the news.
 */

/** The review requests of one repository */
export interface RepositoryReviewRequests {
  /** null until the first answer */
  readonly availability: PullRequestAvailability | null
  /** Waiting the longest first; kept when a refresh fails */
  readonly requests: readonly ReviewRequest[]
  /** All that wait, also beyond the 50 GitHub returns */
  readonly totalCount: number
  /** `Date.now()` of the last answer with the requests, 0 = never: the age of the rows after a failure */
  readonly checkedAt: number
  /** The refresh button asked, and the answer is not in yet */
  readonly isRefreshing: boolean
  /** `reviewRequestKey`s of the requests that came after the first answer and were not opened yet */
  readonly unread: ReadonlySet<string>
  /** The reviews the user submitted from the app in this session, by pull request number */
  readonly reviewed: Readonly<Record<number, SubmittedReview>>
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
  checkedAt: 0,
  isRefreshing: false,
  unread: new Set(),
  reviewed: {},
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
  const repositoryPath = useSelectedProjectPath() ?? ""
  return useStore(operationsStore, state => getReviewBusy(state.running, repositoryPath)[number])
}

const REVIEW_ACTIONS: ReadonlySet<string> = new Set<ReviewAction>(["approve", "requestChanges", "safePush"])

/** The subject of the writes of the user to pull request `number` on review: one at a time */
function reviewSubject(number: number): string {
  return `review:${number}`
}

/**
 * What runs for the pull requests of the repository now (a review being submitted, a Safe Push being asked for), by
 * number: the operations of their writes (`runOperation`)
 */
export function getReviewBusy(running: Operations, repositoryPath: string): Readonly<Record<number, ReviewAction>> {
  const busy: Record<number, ReviewAction> = {}
  for (const operation of running) {
    if (operation.repositoryPath === repositoryPath && operation.subject.startsWith("review:") && REVIEW_ACTIONS.has(operation.id)) {
      busy[Number(operation.subject.slice("review:".length))] = operation.id as ReviewAction
    }
  }
  return busy
}

/**
 * Runs a write of the user to pull request `number` as one operation (`runOperation`): a second click joins it, another
 * write of the same pull request waits for it (said by `feedback`); resolves to false when it did not run
 */
function runReviewWrite(
  repositoryPath: string,
  number: number,
  action: ReviewAction,
  label: string,
  feedback: ReviewFeedback,
  failure: string,
  run: () => Promise<boolean>,
): Promise<boolean> {
  return runOperation({ id: action, subject: reviewSubject(number), objects: [], kind: "remote", label, repositoryPath }, run, reason => feedback.error(failure, reason)).then(
    done => done === true,
  )
}

// ---- the facts ----

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
 * The review requests of the session of `repositoryPath` as its view has them now (`facts.reviewRequests`, with the
 * patch that brought them, `change`; null for a snapshot) and the news of the session the window tells now: the
 * requests a listing added are unread until opened and told once. A row keeps its object where nothing changed, so a
 * poll that brings nothing new re-renders nothing; an answer with the requests prunes the review refs. Nothing is
 * stored for a session GitHub has not answered yet
 */
export function applyReviewRequestsFacts(repositoryPath: string, facts: FactsView, change: FactsChanges | null, news: readonly News[]): void {
  const part = facts.reviewRequests
  const entry = reviewRequestsStore.getState().repositories[repositoryPath]
  if ((change === null || change.reviewRequests != null) && !(entry === undefined && part.head.value === null)) {
    const known = entry ?? EMPTY
    const requests = mergeReviewRequests(known.requests, part.requests)
    const keys = new Set(requests.map(reviewRequestKey))
    const unread = [...known.unread].every(key => keys.has(key)) ? known.unread : new Set([...known.unread].filter(key => keys.has(key)))
    update(repositoryPath, { availability: part.head.value, requests, totalCount: part.totalCount, checkedAt: part.checkedAt * 1000, unread })
    if (part.head.value?.status === "available") {
      pruneReviewRefs(repositoryPath)
    }
  }
  const requested = new Set(news.flatMap(({ event }) => (event.kind === "reviewRequested" ? event.numbers : [])))
  if (requested.size > 0) {
    const requests = (reviewRequestsStore.getState().repositories[repositoryPath] ?? EMPTY).requests.filter(request => requested.has(request.number))
    update(repositoryPath, latest => ({ unread: new Set([...latest.unread, ...requests.map(reviewRequestKey)]) }))
    tellNews(repositoryPath, requests)
  }
}

/**
 * Asks the session of the selected repository to list the review requests again (the refresh button, a review the user
 * submitted, a sign-in), `isRefreshing` until the answer is in the store; never fails: what went wrong is the
 * availability, and the rows stay
 */
export async function refreshReviewRequests(): Promise<void> {
  const repositoryPath = currentRepositoryPath()
  if (repositoryPath === null) {
    return
  }
  update(repositoryPath, { isRefreshing: true })
  try {
    await refreshSessionFacts(repositoryPath, "reviewRequests", "manual")
  } finally {
    update(repositoryPath, { isRefreshing: false })
  }
}

/** `owner/name` while GitHub lists the user's pull requests of the repository: the backend need not resolve it again */
function githubRepositoryOfPullRequests(): string | null {
  const repository = getGitHubRepository()
  return repository === null ? null : `${repository.owner}/${repository.name}`
}

// ---- news ----

/** The requests of `numbers` that still wait for the user's review in the repository: listed, not reviewed */
function waitingRequests(repositoryPath: string, numbers: readonly number[]): ReviewRequest[] {
  const entry = reviewRequestsStore.getState().repositories[repositoryPath] ?? EMPTY
  return numbers.flatMap(number => entry.requests.find(request => request.number === number && entry.reviewed[number] === undefined) ?? [])
}

/**
 * New requests: one toast "alice requested your review" (or "3 reviews requested") with "Review", which opens the first
 * one that still waits; in the background an OS notification and the toast on focus. Each is an Activity entry. The
 * toast of one request is the toast of its pull request (`toastKeys.review`): the review the user submits takes its
 * place; a toast of several closes once none of them waits, and while the main window shows another repository
 */
function tellNews(repositoryPath: string, news: readonly ReviewRequest[]): void {
  const first = news[0]
  if (first === undefined) {
    return
  }
  for (const request of news) {
    recordReviewRequest(repositoryPath, request)
  }
  const { title, description } = describeNewRequests(news)
  const numbers = news.map(request => request.number)
  const key = news.length === 1 ? toastKeys.review(repositoryPath, first.number) : toastKeys.reviewRequests(repositoryPath, numbers)
  announce("reviewRequests", { title, body: description }, () =>
    showEventToast({
      key,
      watch: [reviewRequestsStore, repositoryStore],
      render: () => {
        const [waiting] = waitingRequests(repositoryPath, numbers)
        if (waiting === undefined || currentRepositoryPath() !== repositoryPath) {
          return null
        }
        return {
          type: "info",
          title,
          description,
          action: {
            label: "Review",
            onClick: () => {
              closeEventToasts(key)
              openReviewRequest(repositoryPath, waiting.number)
            },
          },
        }
      },
    }),
  )
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
    void refreshSessionFacts(repositoryPath, "reviewRequests", "afterWrite")
  }
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
    toast: ({ key, title, description, target }) =>
      showEventToast({
        key,
        render: () => ({
          type: "success",
          title,
          description,
          action: { label: reviewToastActionLabel(target), onClick: () => (target.kind === "robot" ? openRobotUrl(target.url) : openRequestOnGitHub(request)) },
        }),
      }),
    error: notifyError,
  }
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
  const failure = reviewFailure(event, number)
  const label = event === "approve" ? `Approving #${number}` : `Requesting changes on #${number}`
  const submitted = await runReviewWrite(repositoryPath, number, event, label, feedback, failure, async () => {
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
        feedback.error(failure, result.error)
        return false
      }
      return true
    } catch (error) {
      feedback.error(failure, error)
      return false
    }
  })
  if (!submitted) {
    return false
  }
  const title = describeReview(event, number)
  // the review is the next state of the toast of the request (`tellNews`), before the request it closes leaves
  feedback.toast({ key: toastKeys.review(repositoryPath, number), title, description: request.title, target: { kind: "pullRequest", url: request.url } })
  markRead(repositoryPath, request)
  update(repositoryPath, entry => ({ reviewed: { ...entry.reviewed, [number]: { event, commitId } } }))
  recordReview(repositoryPath, title, request)
  void refreshReviewRequests()
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
  return runReviewWrite(repositoryPath, number, "safePush", `Starting the Safe Push of #${number}`, feedback, SAFE_PUSH_FAILURE, () =>
    startReviewedSafePush(repositoryPath, request, commitId, githubRepository, feedback),
  )
}

/** The Safe Push of `approveAndSafePush` once the commit is approved: the head checked, then the `/patronus merge` comment */
async function startReviewedSafePush(
  repositoryPath: string,
  request: ReviewRequest,
  commitId: string,
  githubRepository: ReturnType<typeof githubRepositoryOfRequests>,
  feedback: ReviewFeedback,
): Promise<boolean> {
  const { number } = request
  try {
    const head = await commands.getPullRequestHead({ repositoryPath, githubRepository, pullRequestNumber: number })
    if (head.status === "error") {
      feedback.error(SAFE_PUSH_FAILURE, head.error)
      return false
    }
    if (head.data !== commitId) {
      feedback.error(SAFE_PUSH_FAILURE, `#${number} has new commits since you reviewed ${formatShortHash(commitId)}: review ${formatShortHash(head.data)} first`)
      return false
    }
    const robot = await startPatronusFromReviewedPullRequest(number, githubRepository)
    const title = "Safe Push started"
    // the start after the approval is its next state: one toast of the pull request
    const key = toastKeys.review(repositoryPath, number)
    if (robot === null) {
      // the robot is not the user's: the pull request tells how it goes
      feedback.toast({ key, title, description: `#${number} ${request.title}`, target: { kind: "pullRequest", url: request.url } })
      recordReview(repositoryPath, `${title} on #${number}`, request)
    } else {
      watchRobot(robot, `#${number} ${request.title}`)
      const description = describeRobot(robot)
      feedback.toast({ key, title, description, target: { kind: "robot", url: robot.url } })
      recordRobot(title, description, robot)
    }
    return true
  } catch (error) {
    feedback.error(SAFE_PUSH_FAILURE, error)
    return false
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

/** Forgets the data (tests) */
export function resetReviewRequests(): void {
  prunedRepositories.clear()
  reviewRequestsStore.setState({ repositories: {} })
}
