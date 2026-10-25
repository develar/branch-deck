import { describe, expect, it } from "vite-plus/test"
import {
  describeNewRequests,
  describeRequestActivity,
  formatOldestWaiting,
  formatRequestMeta,
  formatWaiting,
  hasWaitedWorkingDay,
  mergeReviewRequests,
  pickVisibleRequests,
  repositoryOfPullRequestUrl,
  reviewRequestKey,
  reviewRequestsSearchUrl,
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
    const ago = (ms: number) => (now - ms) / 1000
    expect(formatWaiting({ waitingSince: ago(30_000), reRequested: false, backToYou: null }, now)).toBe("waiting 1 minute")
    expect(formatWaiting({ waitingSince: ago(5 * 60_000), reRequested: false, backToYou: null }, now)).toBe("waiting 5 minutes")
    expect(formatWaiting({ waitingSince: ago(3 * 3_600_000), reRequested: false, backToYou: null }, now)).toBe("waiting 3 hours")
    expect(formatWaiting({ waitingSince: ago(50 * 3_600_000), reRequested: false, backToYou: null }, now)).toBe("waiting 2 days")
    expect(formatWaiting({ waitingSince: ago(3 * 3_600_000), reRequested: true, backToYou: null }, now)).toBe("re-requested 3 hours ago")
    // the user answered with a comment, the other side moved since: what it did, also after an earlier review
    const replied = { at: ago(10 * 60_000), repliedBy: "alice" }
    expect(formatWaiting({ waitingSince: replied.at, reRequested: true, backToYou: replied }, now)).toBe("alice replied 10 minutes ago")
    expect(formatWaiting({ waitingSince: replied.at, reRequested: false, backToYou: { ...replied, repliedBy: null } }, now)).toBe("new commits 10 minutes ago")
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
  it("keeps the objects and the list of an answer that changed nothing", () => {
    const first = mergeReviewRequests([], [request(1), request(2)])
    expect(first.map(it => it.number)).toEqual([1, 2])
    expect(mergeReviewRequests(first, [request(1), request(2)])).toBe(first)

    const changed = mergeReviewRequests(first, [request(1), request(2, { title: "Renamed" })])
    expect(changed).not.toBe(first)
    expect(changed[0]).toBe(first[0])
    expect(changed[1]!.title).toBe("Renamed")
  })

  it("shows the oldest three and every unread request, an unread one in the place of a read one", () => {
    const sorted = [1, 2, 3, 4, 5].map(it => request(it))
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
  it("keeps the key of a request back to the user after their answer", () => {
    const back = request(2, { requestedAt: 1_002, waitingSince: 9_000, backToYou: { at: 9_000, repliedBy: "alice" } })
    expect(reviewRequestKey(back)).toBe(reviewRequestKey(request(2)))
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

describe("links", () => {
  it("links the search of all requests of the repository of a pull request", () => {
    const repository = repositoryOfPullRequestUrl("https://github.com/acme/monorepo/pull/12")
    expect(repository).toEqual({ owner: "acme", name: "monorepo" })
    expect(reviewRequestsSearchUrl(repository!)).toBe(
      "https://github.com/acme/monorepo/pulls?q=is%3Apr%20is%3Aopen%20archived%3Afalse%20draft%3Afalse%20user-review-requested%3A%40me",
    )
    expect(repositoryOfPullRequestUrl("https://example.com/x")).toBeNull()
  })
})
