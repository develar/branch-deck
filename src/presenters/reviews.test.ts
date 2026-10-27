import { describe, expect, it } from "vite-plus/test"
import type { ReviewDecision, ReviewEntry, Reviewer } from "@/lib/bindings"
import { searchWords, matchesAllWords } from "@/lib/text-search"
import {
  formatReviewHint,
  formatReviewMeta,
  formatReviewsNoMatch,
  pickVisibleEntries,
  returnedTexts,
  reviewEntryTexts,
  reviewMark,
  reviewRequestTexts,
  reviewSectionSearchUrl,
  reviewsSearchUrl,
  shownPullRequestOf,
  typedReference,
} from "@/presenters/reviews"
import { openPullRequest } from "@/test-support/pull-request"
import { reviewRequest } from "@/test-support/review-request"

const REPOSITORY = { owner: "acme", name: "monorepo" }

const TEAM: ReviewEntry = { request: reviewRequest(201, { title: "Add OAuth login", author: "carol" }), involvement: { kind: "team", teams: ["ide-platform"] } }
const OWN: ReviewEntry = {
  request: reviewRequest(203, { title: "Document the CLI", author: "user-name", headRefName: "user-name/virtual/feature-docs", baseRefName: "release-261" }),
  involvement: { kind: "authored", reviewers: [{ login: "dave", state: "approved", requested: false }], isDraft: false, branch: "feature-docs" },
}

function matches(texts: readonly string[], text: string): boolean {
  return matchesAllWords(texts, searchWords(text))
}

describe("the filter of Reviews", () => {
  it("matches a row by every word of what it shows and its branches, ignoring case, never by its URL", () => {
    const team = reviewEntryTexts(TEAM)
    expect(matches(team, "oauth")).toBe(true)
    expect(matches(team, "OAuth LOGIN")).toBe(true)
    expect(matches(team, "#201")).toBe(true)
    expect(matches(team, "carol")).toBe(true)
    // the meta of how the user takes part: the team that asks
    expect(matches(team, "ide-platform")).toBe(true)
    expect(matches(team, "alice/feature-201")).toBe(true)
    // every word, not any
    expect(matches(team, "oauth schema")).toBe(false)
    // the words of the URL are in every row
    expect(matches(team, "github")).toBe(false)
    expect(matches(team, "acme")).toBe(false)

    const own = reviewEntryTexts(OWN)
    // the reviewers of an own pull request, and the base it goes into
    expect(matches(own, "approved by dave")).toBe(true)
    expect(matches(own, "release-261")).toBe(true)
    expect(matches(own, "feature-docs")).toBe(true)
  })

  it("matches a request by name by who asked, and a returned pull request by who handed it back and its branch", () => {
    expect(matches(reviewRequestTexts(reviewRequest(101, { title: "Rework the session store" })), "requested by bob")).toBe(true)
    expect(matches(reviewRequestTexts(reviewRequest(101)), "#101 alice")).toBe(true)
    const returned = {
      branchName: "feature-ui",
      pullRequest: openPullRequest("feature-ui", { number: 812, reviewers: [{ login: "erin", state: "changesRequested", requested: false }] }),
    }
    expect(matches(returnedTexts(returned), "erin")).toBe(true)
    expect(matches(returnedTexts(returned), "#812 feature-ui")).toBe(true)
  })

  it("names a pull request by its number or a link, and words by nothing", () => {
    expect(typedReference(" #305 ")).toEqual({ kind: "number", number: 305 })
    expect(typedReference("305")).toEqual({ kind: "number", number: 305 })
    expect(typedReference("https://github.com/acme/monorepo/pull/305/files")).toEqual({ kind: "link", text: "https://github.com/acme/monorepo/pull/305/files" })
    expect(typedReference("acme/monorepo#305")).toEqual({ kind: "link", text: "acme/monorepo#305" })
    expect(typedReference("oauth")).toBeNull()
    expect(typedReference("oauth login")).toBeNull()
    expect(typedReference("feature / docs")).toBeNull()
    expect(typedReference("")).toBeNull()
  })

  it("finds the shown row a number or a pasted link names, never another number that starts the same", () => {
    const rows = [201, 202, 2010].map(number => ({ number, url: `https://github.com/acme/monorepo/pull/${number}` }))
    expect(shownPullRequestOf({ kind: "number", number: 201 }, rows)?.number).toBe(201)
    expect(shownPullRequestOf({ kind: "number", number: 20 }, rows)).toBeUndefined()
    expect(shownPullRequestOf({ kind: "link", text: "https://github.com/acme/monorepo/pull/201" }, rows)?.number).toBe(201)
    expect(shownPullRequestOf({ kind: "link", text: "https://github.com/acme/monorepo/pull/201/files" }, rows)?.number).toBe(201)
    expect(shownPullRequestOf({ kind: "link", text: "https://github.com/acme/monorepo/pull/201#discussion_r1" }, rows)?.number).toBe(201)
    expect(shownPullRequestOf({ kind: "link", text: "https://github.com/acme/monorepo/pull/2010" }, rows)?.number).toBe(2010)
    expect(shownPullRequestOf({ kind: "link", text: "https://github.com/acme/monorepo/pull/20" }, rows)).toBeUndefined()
    expect(shownPullRequestOf({ kind: "link", text: "acme/monorepo#201" }, rows)).toBeUndefined()
  })

  it("searches GitHub with the typed words where it returned only a part, and says what found nothing", () => {
    expect(reviewSectionSearchUrl("team", REPOSITORY, " oauth ")).toBe(
      `https://github.com/acme/monorepo/pulls?q=${encodeURIComponent("is:pr is:open archived:false draft:false review-requested:@me oauth")}`,
    )
    expect(reviewSectionSearchUrl("team", REPOSITORY)).toBe(
      `https://github.com/acme/monorepo/pulls?q=${encodeURIComponent("is:pr is:open archived:false draft:false review-requested:@me")}`,
    )
    expect(reviewSectionSearchUrl("linked", REPOSITORY, "oauth")).toBeNull()
    expect(reviewsSearchUrl(REPOSITORY, "oauth")).toBe(`https://github.com/acme/monorepo/pulls?q=${encodeURIComponent("is:pr is:open involves:@me oauth")}`)
    expect(formatReviewsNoMatch(" migrate ")).toBe("No pull requests match “migrate”")
  })
})

describe("a row of Reviews", () => {
  const reviewed = (state: "approved" | "changesRequested" | "commented" | null): ReviewEntry => ({
    request: reviewRequest(202, { author: "carol", reviewedState: state, reviewedCommit: state === null ? null : "6330a0a2ffffffffffffffffffffffffffffffff" }),
    involvement: { kind: "reviewed", state, repliedAt: state === null ? 1_000 : null, newCommits: false },
  })

  it("says in its meta only what its mark doesn't, the rest in the hint", () => {
    expect(formatReviewMeta(TEAM)).toBe("#201 · carol · requested from ide-platform")
    expect(formatReviewMeta(reviewed("approved"))).toBe("#202 · carol")
    expect(formatReviewHint(reviewed("approved"))).toBe("you approved 6330a0a2")
    expect(formatReviewMeta(reviewed(null))).toBe("#202 · carol · you replied")
    // an own pull request: where it stands in short, whoever approved in the hint
    const own = (reviewers: Reviewer[], reviewDecision: ReviewDecision): ReviewEntry => ({
      request: reviewRequest(1250, { reviewDecision }),
      involvement: { kind: "authored", reviewers, isDraft: false, branch: null },
    })
    const reviewers: Reviewer[] = [
      { login: "dave", state: "approved", requested: false },
      { login: "erin", state: "requested", requested: true },
    ]
    expect(formatReviewMeta(own(reviewers, "reviewRequired"))).toBe("#1250 · waiting for erin")
    expect(formatReviewHint(own(reviewers, "reviewRequired"))).toBe("approved by dave · waiting for erin")
    // an approval is the mark of the row, not a word of its meta
    expect(formatReviewMeta(own([], "approved"))).toBe("#1250")
    expect(reviewMark(own([], "approved"), false)).toEqual({ kind: "approved", label: "Approved" })
    expect(formatReviewMeta(own([], "reviewRequired"))).toBe("#1250 · review required")
  })

  it("marks the user's move alone, and the user's review muted", () => {
    expect(reviewMark(reviewed("approved"), true)).toEqual({ kind: "move", label: "Your move: new commits since your review" })
    expect(reviewMark(reviewed("approved"), false)).toEqual({ kind: "approved", label: "You approved 6330a0a2" })
    expect(reviewMark(reviewed("changesRequested"), false).kind).toBe("changesRequested")
    expect(reviewMark(reviewed(null), false)).toEqual({ kind: "commented", label: "You replied" })
    expect(reviewMark(TEAM, false).kind).toBe("none")
  })

  it("shows the user's moves and three more before Show N More", () => {
    const entries = [1, 2, 3, 4, 5, 6].map(number => ({ ...TEAM, request: reviewRequest(number) }))
    expect(pickVisibleEntries(entries, new Set([5])).map(entry => entry.request.number)).toEqual([1, 2, 3, 5])
    expect(pickVisibleEntries(entries, new Set()).map(entry => entry.request.number)).toEqual([1, 2, 3])
    // all of a short section show: the same list
    const three = entries.slice(0, 3)
    expect(pickVisibleEntries(three, new Set())).toBe(three)
  })
})
