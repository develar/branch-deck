import type { PullRequestAvailability, ReviewRequest } from "@/lib/bindings"
import { cutList } from "@/lib/text"
import { formatRelativeTime } from "@/lib/time"

/*
 * The pull requests of others that wait for the user's review ("Your Turn", spec/repository/review-requests.spec.md):
 * their ages, their order, what is new in an answer, and how often GitHub is asked.
 */

/** GitHub is asked this often, focused or not */
export const POLL_INTERVAL_MS = 5 * 60_000
/** A poll closer than this to the last request asks nothing: a sync or a focus asked meanwhile */
export const MIN_POLL_GAP_MS = 4.5 * 60_000
/** A failing GitHub is asked at most this rarely: the interval doubles per failure up to it */
export const MAX_BACKOFF_MS = 30 * 60_000
/** After GitHub's rate limit, the next request waits at least this long */
export const RATE_LIMIT_WAIT_MS = 15 * 60_000
/** The rows of the card before "Show N More" */
export const VISIBLE_ROW_CAP = 3

/** A request waits long after this much time on working days (Monday to Friday, local time) */
export const WORKING_DAY_MS = 24 * 60 * 60_000

/**
 * Whether `since` (ms) is a working day before `now` (ms) at least: Saturdays and Sundays don't count, so a request of
 * Friday afternoon is not old on Monday morning
 */
export function hasWaitedWorkingDay(since: number, now: number): boolean {
  let waited = 0
  let cursor = since
  while (cursor < now) {
    const day = new Date(cursor)
    const nextDay = new Date(day.getFullYear(), day.getMonth(), day.getDate() + 1).getTime()
    const end = Math.min(nextDay, now)
    const weekday = day.getDay()
    if (weekday !== 0 && weekday !== 6) {
      waited += end - cursor
      if (waited >= WORKING_DAY_MS) {
        return true
      }
    }
    cursor = end
  }
  return false
}

/** The requests, waiting the longest first; the same time by number */
export function sortReviewRequests(requests: readonly ReviewRequest[]): ReviewRequest[] {
  return requests.toSorted((a, b) => a.waitingSince - b.waitingSince || a.number - b.number)
}

/** "5 minutes", "3 hours", "2 days": how long from `since` to `now` (both ms), at least a minute */
function formatDuration(since: number, now: number): string {
  const minutes = Math.max(1, Math.floor((now - since) / 60_000))
  if (minutes < 60) {
    return minutes === 1 ? "1 minute" : `${minutes} minutes`
  }
  const hours = Math.floor(minutes / 60)
  if (hours < 24) {
    return hours === 1 ? "1 hour" : `${hours} hours`
  }
  const days = Math.floor(hours / 24)
  return days === 1 ? "1 day" : `${days} days`
}

/** "waiting 2 days", or "re-requested 3 hours ago" after an earlier review of the user */
export function formatWaiting(request: Pick<ReviewRequest, "waitingSince" | "reRequested">, now: number): string {
  const since = request.waitingSince * 1000
  return request.reRequested ? `re-requested ${formatRelativeTime(since, now)}` : `waiting ${formatDuration(since, now)}`
}

/** "oldest waiting 4 days": after the count in the header of the collapsed card; `oldest` is the first of the sorted requests */
export function formatOldestWaiting(oldest: Pick<ReviewRequest, "waitingSince">, now: number): string {
  return `oldest waiting ${formatDuration(oldest.waitingSince * 1000, now)}`
}

/** A request as the user was asked: a re-request is a new one */
export function reviewRequestKey(request: Pick<ReviewRequest, "number" | "waitingSince">): string {
  return `${request.number}@${request.waitingSince}`
}

/**
 * The requests of `next` the user was not told about yet: those whose key (`reviewRequestKey`) `previous` lacks.
 * `previous` is null for the first answer after a start or a repository switch, which is silent: nothing is new then.
 */
export function findNewRequests(previous: readonly ReviewRequest[] | null, next: readonly ReviewRequest[]): ReviewRequest[] {
  if (previous === null) {
    return []
  }
  const known = new Set(previous.map(reviewRequestKey))
  return next.filter(request => !known.has(reviewRequestKey(request)))
}

/** The new requests one toast names; one more is named too (`cutList`) */
const NAMED_REQUESTS = 3

/**
 * "alice requested your review" / "#123 Fix the login": a toast and an OS notification, one for several requests, the
 * first ones named and the count of the rest, never "1 more"
 */
export function describeNewRequests(news: readonly ReviewRequest[]): { title: string; description: string } {
  const [first] = news
  if (news.length === 1 && first !== undefined) {
    return { title: first.requestedBy === null ? "Your review was requested" : `${first.requestedBy} requested your review`, description: `#${first.number} ${first.title}` }
  }
  const { shown, more } = cutList(news, NAMED_REQUESTS)
  const lines = shown.map(request => `#${request.number} ${request.title}`)
  return { title: `${news.length} reviews requested`, description: (more === 0 ? lines : [...lines, `${more} more`]).join(" · ") }
}

/** The Activity entry of a request: "alice requested your review on #123" / "Fix the login" */
export function describeRequestActivity(request: ReviewRequest): { title: string; description: string } {
  const on = `#${request.number}`
  return { title: request.requestedBy === null ? `Review requested on ${on}` : `${request.requestedBy} requested your review on ${on}`, description: request.title }
}

function sameRequest(a: ReviewRequest, b: ReviewRequest): boolean {
  for (const key of Object.keys(a) as (keyof ReviewRequest)[]) {
    if (a[key] !== b[key]) {
      return false
    }
  }
  return Object.keys(a).length === Object.keys(b).length
}

/**
 * The sorted requests of an answer with the objects of `previous` kept where nothing changed, and `previous` itself
 * when the whole list is the same: a poll that brings nothing new re-renders nothing
 */
export function mergeReviewRequests(previous: readonly ReviewRequest[], incoming: readonly ReviewRequest[]): readonly ReviewRequest[] {
  const byNumber = new Map(previous.map(request => [request.number, request]))
  const next = sortReviewRequests(incoming).map(request => {
    const known = byNumber.get(request.number)
    return known !== undefined && sameRequest(known, request) ? known : request
  })
  return next.length === previous.length && next.every((request, index) => request === previous[index]) ? previous : next
}

/**
 * The rows the card shows before "Show N More": the oldest `cap`, and every unread request on top of them, in their
 * order; an unread request takes the place of a read one while the rows are full, so a poll doesn't grow the list. The
 * named exception to the cut rule of `cutList`: "Show 1 More" stays, a fourth row would come and go with the polls
 */
export function pickVisibleRequests(sorted: readonly ReviewRequest[], unread: ReadonlySet<string>, cap: number = VISIBLE_ROW_CAP): readonly ReviewRequest[] {
  const unreadCount = sorted.filter(request => unread.has(reviewRequestKey(request))).length
  let readSlots = Math.max(0, cap - unreadCount)
  const visible = sorted.filter(request => {
    if (unread.has(reviewRequestKey(request))) {
      return true
    }
    return readSlots-- > 0
  })
  return visible.length === sorted.length ? sorted : visible
}

/**
 * When to ask GitHub again after an answer, ms; null to wait for a focus or the refresh button. `failures` counts the
 * failed answers in a row, this one included: an error doubles the interval up to 30 min, the rate limit waits 15 min
 * at least; without a token GitHub accepts, without access to the repository or off GitHub nothing is polled (a focus or
 * the refresh button asks again).
 */
export function nextPollDelay(availability: PullRequestAvailability, failures: number): number | null {
  switch (availability.status) {
    case "available":
      return POLL_INTERVAL_MS
    case "notGitHub":
    case "signedOut":
    case "notAuthenticated":
    case "ssoRequired":
    case "noAccess":
      return null
    case "rateLimited":
      return Math.max(RATE_LIMIT_WAIT_MS, backoff(failures))
    case "error":
      return backoff(failures)
  }
}

function backoff(failures: number): number {
  return Math.min(MAX_BACKOFF_MS, POLL_INTERVAL_MS * 2 ** Math.max(0, failures))
}

/** `owner/name` of a pull request URL (`https://github.com/acme/monorepo/pull/12`); null for another URL */
export function repositoryOfPullRequestUrl(url: string): { owner: string; name: string } | null {
  const match = /^https:\/\/github\.com\/([^/]+)\/([^/]+)\/pull\/\d+/.exec(url)
  return match === null ? null : { owner: match[1]!, name: match[2]! }
}

/** The GitHub search of all pull requests that wait for the user's review in the repository: "Open All on GitHub" */
export function reviewRequestsSearchUrl(repository: { owner: string; name: string }): string {
  const query = "is:pr is:open archived:false draft:false user-review-requested:@me"
  return `https://github.com/${repository.owner}/${repository.name}/pulls?q=${encodeURIComponent(query)}`
}

/**
 * "#123 · alice": the start of the muted meta of a row; "#123 · agent-bot · requested by dave" when someone else than the
 * author asked for the review (a pull request a bot opened): that one waits for the user
 */
export function formatRequestMeta(request: ReviewRequest): string {
  const parts = [`#${request.number}`]
  if (request.author !== null) {
    parts.push(request.author)
  }
  if (request.requestedBy !== null && request.requestedBy !== request.author) {
    parts.push(`requested by ${request.requestedBy}`)
  }
  return parts.join(" · ")
}
