import { describe, expect, it } from "vite-plus/test"
import {
  MAX_BACKOFF_MS,
  POLL_INTERVAL_MS,
  RATE_LIMIT_WAIT_MS,
  describeNewRequests,
  describeRequestActivity,
  findNewRequests,
  formatOldestWaiting,
  formatRequestMeta,
  formatWaiting,
  hasWaitedWorkingDay,
  mergeReviewRequests,
  nextPollDelay,
  pickVisibleRequests,
  repositoryOfPullRequestUrl,
  reviewRequestKey,
  reviewRequestsSearchUrl,
  sortReviewRequests,
} from "@/lib/review-requests"
import { reviewRequest as request } from "@/test-support/review-request"

/** Local time: 2026-10-16 is a Friday */
function at(day: number, hour: number): number {
  return new Date(2026, 9, day, hour).getTime()
}

describe("working-day age", () => {
  it("is old after a day of working time, the weekend skipped", () => {
    // Thursday 10:00 → Friday 10:00
    expect(hasWaitedWorkingDay(at(15, 10), at(16, 9))).toBe(false)
    expect(hasWaitedWorkingDay(at(15, 10), at(16, 10))).toBe(true)
    // Friday 15:00 → Monday 10:00 is 9 + 10 working hours
    expect(hasWaitedWorkingDay(at(16, 15), at(19, 10))).toBe(false)
    expect(hasWaitedWorkingDay(at(16, 15), at(19, 15))).toBe(true)
    // asked on Saturday: the age starts on Monday
    expect(hasWaitedWorkingDay(at(17, 12), at(19, 23))).toBe(false)
    expect(hasWaitedWorkingDay(at(17, 12), at(20, 0))).toBe(true)
  })

  it("says how long a request waits, or when the user was asked again", () => {
    const now = at(19, 12)
    expect(formatWaiting({ waitingSince: (now - 30_000) / 1000, reRequested: false }, now)).toBe("waiting 1 minute")
    expect(formatWaiting({ waitingSince: (now - 5 * 60_000) / 1000, reRequested: false }, now)).toBe("waiting 5 minutes")
    expect(formatWaiting({ waitingSince: (now - 3 * 3_600_000) / 1000, reRequested: false }, now)).toBe("waiting 3 hours")
    expect(formatWaiting({ waitingSince: (now - 50 * 3_600_000) / 1000, reRequested: false }, now)).toBe("waiting 2 days")
    expect(formatWaiting({ waitingSince: (now - 3 * 3_600_000) / 1000, reRequested: true }, now)).toBe("re-requested 3 hours ago")
    expect(formatOldestWaiting(request(1, { waitingSince: (now - 4 * 86_400_000) / 1000 }), now)).toBe("oldest waiting 4 days")
  })
})

describe("formatRequestMeta", () => {
  it("names who asked for the review when it is not the author", () => {
    // a pull request a bot opened: the one who asked waits for the user
    expect(formatRequestMeta(request(830, { author: "agent-bot", requestedBy: "dave" }))).toBe("#830 · agent-bot · requested by dave")
    expect(formatRequestMeta(request(830, { author: "dave", requestedBy: "dave" }))).toBe("#830 · dave")
    expect(formatRequestMeta(request(830, { requestedBy: null }))).toBe("#830 · alice")
    expect(formatRequestMeta(request(830, { author: null, requestedBy: null }))).toBe("#830")
  })
})

describe("order and rows", () => {
  it("sorts waiting the longest first", () => {
    const sorted = sortReviewRequests([request(1, { waitingSince: 30 }), request(2, { waitingSince: 10 }), request(3, { waitingSince: 10 })])
    expect(sorted.map(it => it.number)).toEqual([2, 3, 1])
  })

  it("keeps the objects and the list of an answer that changed nothing", () => {
    const first = mergeReviewRequests([], [request(2), request(1)])
    expect(first.map(it => it.number)).toEqual([1, 2])
    expect(mergeReviewRequests(first, [request(1), request(2)])).toBe(first)

    const changed = mergeReviewRequests(first, [request(1), request(2, { title: "Renamed" })])
    expect(changed).not.toBe(first)
    expect(changed[0]).toBe(first[0])
    expect(changed[1]!.title).toBe("Renamed")
  })

  it("shows the oldest three and every unread request, an unread one in the place of a read one", () => {
    const sorted = sortReviewRequests([1, 2, 3, 4, 5].map(it => request(it)))
    expect(pickVisibleRequests(sorted, new Set()).map(it => it.number)).toEqual([1, 2, 3])
    expect(pickVisibleRequests(sorted, new Set([reviewRequestKey(sorted[4]!)])).map(it => it.number)).toEqual([1, 2, 5])
    const unread = new Set(sorted.slice(1).map(reviewRequestKey))
    expect(pickVisibleRequests(sorted, unread).map(it => it.number)).toEqual([2, 3, 4, 5])
    // within the cap: the list itself
    const two = sorted.slice(0, 2)
    expect(pickVisibleRequests(two, new Set())).toBe(two)
  })
})

describe("new requests", () => {
  it("is silent for the first answer after a start or a repository switch", () => {
    expect(findNewRequests(null, [request(1)])).toEqual([])
  })

  it("tells the requests not known yet, a re-request too", () => {
    const previous = [request(1), request(2)]
    const reRequested = request(2, { waitingSince: 9_000, reRequested: true })
    expect(findNewRequests(previous, [request(1), reRequested, request(3)]).map(it => it.number)).toEqual([2, 3])
    expect(findNewRequests(previous, [request(1)])).toEqual([])
  })

  it("tells one request by who asked, several in one notice", () => {
    expect(describeNewRequests([request(12, { requestedBy: "bob" })])).toEqual({ title: "bob requested your review", description: "#12 Change 12" })
    expect(describeNewRequests([request(12, { requestedBy: null })]).title).toBe("Your review was requested")
    expect(describeNewRequests([1, 2, 3].map(it => request(it)))).toEqual({ title: "3 reviews requested", description: "#1 Change 1 · #2 Change 2 · #3 Change 3" })
    expect(describeNewRequests([1, 2, 3, 4, 5].map(it => request(it))).description).toBe("#1 Change 1 · #2 Change 2 · #3 Change 3 · 2 more")
    // never "1 more": the fourth is named
    expect(describeNewRequests([1, 2, 3, 4].map(it => request(it))).description).toBe("#1 Change 1 · #2 Change 2 · #3 Change 3 · #4 Change 4")
    expect(describeRequestActivity(request(7, { requestedBy: "carol" }))).toEqual({ title: "carol requested your review on #7", description: "Change 7" })
  })
})

describe("polling", () => {
  it("polls every five minutes, backs off on errors, waits out the rate limit and stops without a token", () => {
    expect(nextPollDelay({ status: "available", owner: "acme", name: "monorepo" }, 0)).toBe(POLL_INTERVAL_MS)
    expect(nextPollDelay({ status: "error", message: "offline" }, 1)).toBe(2 * POLL_INTERVAL_MS)
    expect(nextPollDelay({ status: "error", message: "offline" }, 2)).toBe(4 * POLL_INTERVAL_MS)
    expect(nextPollDelay({ status: "error", message: "offline" }, 3)).toBe(MAX_BACKOFF_MS)
    expect(nextPollDelay({ status: "error", message: "offline" }, 10)).toBe(MAX_BACKOFF_MS)
    expect(nextPollDelay({ status: "rateLimited", retryAt: 0 }, 1)).toBe(RATE_LIMIT_WAIT_MS)
    expect(nextPollDelay({ status: "rateLimited", retryAt: 0 }, 3)).toBe(MAX_BACKOFF_MS)
    expect(nextPollDelay({ status: "notAuthenticated", source: "gh" }, 1)).toBeNull()
    expect(nextPollDelay({ status: "signedOut" }, 1)).toBeNull()
    expect(nextPollDelay({ status: "noAccess", source: "gh", message: "Not Found" }, 1)).toBeNull()
    expect(nextPollDelay({ status: "notGitHub" }, 0)).toBeNull()
  })

  it("links the search of all requests of the repository of a pull request", () => {
    const repository = repositoryOfPullRequestUrl("https://github.com/acme/monorepo/pull/12")
    expect(repository).toEqual({ owner: "acme", name: "monorepo" })
    expect(reviewRequestsSearchUrl(repository!)).toBe(
      "https://github.com/acme/monorepo/pulls?q=is%3Apr%20is%3Aopen%20archived%3Afalse%20draft%3Afalse%20user-review-requested%3A%40me",
    )
    expect(repositoryOfPullRequestUrl("https://example.com/x")).toBeNull()
  })
})
