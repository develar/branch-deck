/**
 * The words of Reviews (spec/repository/reviews.spec.md): the title of each section, the meta and the age of a pull
 * request by how the user takes part in it, the GitHub search of each section, and who asks in the review window. Rust
 * decides the sections and their order (`domain_policy::reviews`); this module only words them.
 */
import type { ReviewEntry, ReviewInvolvement, ReviewRequest, ReviewSectionKind, ReviewerState } from "@/lib/bindings"
import { formatShortHash } from "@/lib/hash"
import { formatDuration } from "@/lib/review-requests"
import { formatRelativeTime } from "@/lib/time"

/** The title of each section after the requests by name */
export const REVIEW_SECTION_TITLES: Readonly<Record<ReviewSectionKind, string>> = {
  linked: "Added by Link",
  team: "Requested from Your Teams",
  reviewed: "Reviewed by You",
  authored: "Your Pull Requests",
}

/** "you approved a1b2c3d", "you requested changes", "you commented"; null without a review of the user */
function yourReview(state: ReviewerState | null, commit: string | null): string | null {
  switch (state) {
    case "approved":
      return commit === null ? "you approved" : `you approved ${formatShortHash(commit)}`
    case "changesRequested":
      return "you requested changes"
    case "commented":
      return "you commented"
    default:
      return null
  }
}

/** "approved by alice · waiting for bob, ide-platform": the reviewers of an own pull request by their latest review */
function reviewersWords(involvement: Extract<ReviewInvolvement, { kind: "authored" }>): string | null {
  const approved = involvement.reviewers.filter(reviewer => reviewer.state === "approved").map(reviewer => reviewer.login)
  const changes = involvement.reviewers.filter(reviewer => reviewer.state === "changesRequested").map(reviewer => reviewer.login)
  const waiting = involvement.reviewers.filter(reviewer => reviewer.requested && reviewer.state !== "approved").map(reviewer => reviewer.login)
  const parts = [
    ...(involvement.isDraft ? ["draft"] : []),
    ...(changes.length > 0 ? [`changes requested by ${changes.join(", ")}`] : []),
    ...(approved.length > 0 ? [`approved by ${approved.join(", ")}`] : []),
    ...(waiting.length > 0 ? [`waiting for ${waiting.join(", ")}`] : []),
  ]
  return parts.length === 0 ? null : parts.join(" · ")
}

/**
 * The muted meta of a row of Reviews: "#123 · carol" and how the user takes part: "requested from ide-platform", "you
 * approved a1b2c3d", "not reviewed"; of an own pull request its reviewers
 */
export function formatReviewMeta(entry: ReviewEntry): string {
  const { request, involvement } = entry
  const parts = [`#${request.number}`]
  if (involvement.kind !== "authored" && request.author !== null) {
    parts.push(request.author)
  }
  switch (involvement.kind) {
    case "team":
      parts.push(involvement.teams.length === 0 ? "requested from a team of yours" : `requested from ${involvement.teams.join(", ")}`)
      break
    case "linked":
      parts.push(yourReview(request.reviewedState, request.reviewedCommit) ?? "not reviewed")
      break
    case "reviewed":
      parts.push(involvement.state === null ? "you replied" : (yourReview(involvement.state, request.reviewedCommit) ?? "you reviewed"))
      break
    case "authored": {
      const reviewers = reviewersWords(involvement)
      if (reviewers !== null) {
        parts.push(reviewers)
      }
      break
    }
  }
  return parts.join(" · ")
}

/** What the age at the end of a row says: "waiting 2 days" of a team request, "new commits since your review", "updated 3 hours ago" */
export function formatReviewAge(entry: ReviewEntry, now: number): string {
  const { request, involvement } = entry
  switch (involvement.kind) {
    case "team":
      return `waiting ${formatDuration(request.waitingSince * 1000, now)}`
    case "reviewed":
      if (involvement.newCommits) {
        return "new commits since your review"
      }
      if (involvement.repliedAt !== null) {
        return `you replied ${formatRelativeTime(involvement.repliedAt * 1000, now)}`
      }
      return `updated ${formatRelativeTime(request.updatedAt * 1000, now)}`
    default:
      return `updated ${formatRelativeTime(request.updatedAt * 1000, now)}`
  }
}

/** The searches of the sections as Rust asks GitHub (`remote_facts::reviews::reviews_searches`), for "Open All on GitHub" */
const SECTION_SEARCHES: Readonly<Record<Exclude<ReviewSectionKind, "linked">, string>> = {
  team: "is:pr is:open archived:false draft:false review-requested:@me",
  reviewed: "is:pr is:open archived:false draft:false reviewed-by:@me -author:@me",
  authored: "is:pr is:open archived:false author:@me",
}

/** The GitHub search of a section in the repository; null for the links, which no search lists */
export function reviewSectionSearchUrl(kind: ReviewSectionKind, repository: { owner: string; name: string }): string | null {
  return kind === "linked" ? null : pullsUrl(repository, SECTION_SEARCHES[kind])
}

/** Every open pull request of the repository the user takes part in, on GitHub: "Open on GitHub" of the view */
export function reviewsSearchUrl(repository: { owner: string; name: string }): string {
  return pullsUrl(repository, "is:pr is:open involves:@me")
}

function pullsUrl(repository: { owner: string; name: string }, query: string): string {
  return `https://github.com/${repository.owner}/${repository.name}/pulls?q=${encodeURIComponent(query)}`
}

/** Who asks in the review window of a pull request of Reviews: "Added by link", "ide-platform asks for a review", "You reviewed carol's pull request" */
export function formatAskedBy(request: ReviewRequest, involvement: ReviewInvolvement): string {
  const of = request.author === null ? "the pull request" : `${request.author}'s pull request`
  switch (involvement.kind) {
    case "linked":
      return `Added by link: ${of}`
    case "team":
      return `${involvement.teams.length === 0 ? "A team of yours" : involvement.teams.join(", ")} asked for a review of ${of}`
    case "reviewed":
      return involvement.state === null ? `You replied to ${of}` : `You reviewed ${of}`
    case "authored":
      return "Your pull request"
  }
}
