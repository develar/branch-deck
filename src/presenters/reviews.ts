/**
 * The words of Reviews (openspec/specs/repository/reviews/spec.md): the title of each section, the meta and the age of a pull
 * request by how the user takes part in it, the GitHub search of each section, and who asks in the review window. Rust
 * decides the sections and their order (`domain_policy::reviews`); this module only words them.
 */
import type { ApprovalLanding, ReviewEntry, ReviewInvolvement, ReviewRequest, ReviewSectionKind, ReviewerState } from "@/lib/bindings"
import { formatShortHash } from "@/lib/hash"
import type { ReturnedPullRequest } from "@/lib/outgoing-attention"
import { formatReturnedMeta } from "@/lib/outgoing-attention"
import { VISIBLE_ROW_CAP, formatDuration, formatRequestMeta, pickVisibleRequests, reviewRequestKey, withSearchText } from "@/lib/review-requests"
import { capitalize, joinNames } from "@/lib/text"
import { formatRelativeTime } from "@/lib/time"

/** The title of each section after the requests by name */
export const REVIEW_SECTION_TITLES: Readonly<Record<ReviewSectionKind, string>> = {
  linked: "Added by Link",
  team: "Requested from Your Teams",
  reviewed: "Reviewed by You",
  authored: "Your Pull Requests",
}

/** The title of the last section: the closed pull requests the user takes part in whose titles have the words of the filter */
export const CLOSED_SECTION_TITLE = "Closed and Merged"

/** "merged", "closed": how a closed pull request ended; null of an open one */
function closedWord(request: ReviewRequest): "merged" | "closed" | null {
  switch (request.state) {
    case "merged":
      return "merged"
    case "closed":
      return "closed"
    default:
      return null
  }
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
  const changes = changesRequestedBy(involvement)
  const waiting = waitingFor(involvement)
  const parts = [
    ...(involvement.isDraft ? ["draft"] : []),
    ...(changes.length > 0 ? [`changes requested by ${changes.join(", ")}`] : []),
    ...(approved.length > 0 ? [`approved by ${approved.join(", ")}`] : []),
    ...(waiting.length > 0 ? [`waiting for ${waiting.join(", ")}`] : []),
  ]
  return parts.length === 0 ? null : parts.join(" · ")
}

function changesRequestedBy(involvement: Extract<ReviewInvolvement, { kind: "authored" }>): string[] {
  return involvement.reviewers.filter(reviewer => reviewer.state === "changesRequested").map(reviewer => reviewer.login)
}

function waitingFor(involvement: Extract<ReviewInvolvement, { kind: "authored" }>): string[] {
  return involvement.reviewers.filter(reviewer => reviewer.requested && reviewer.state !== "approved").map(reviewer => reviewer.login)
}

/**
 * Where an own pull request stands in short, as GitHub decides it: "changes requested by alice", "waiting for bob",
 * "review required"; an approval is its mark (`reviewMark`), whoever approved is in the hint (`formatReviewHint`)
 */
function authoredWords(request: ReviewRequest, involvement: Extract<ReviewInvolvement, { kind: "authored" }>): string | null {
  const changes = changesRequestedBy(involvement)
  const waiting = waitingFor(involvement)
  const parts = [
    ...(involvement.isDraft ? ["draft"] : []),
    ...(changes.length > 0 ? [`changes requested by ${changes.join(", ")}`] : request.reviewDecision === "changesRequested" ? ["changes requested"] : []),
    ...(waiting.length > 0 ? [`waiting for ${waiting.join(", ")}`] : []),
  ]
  if (parts.length === 0 && request.reviewDecision === "reviewRequired" && involvement.reviewers.every(reviewer => reviewer.state !== "approved")) {
    return "review required"
  }
  return parts.length === 0 ? null : parts.join(" · ")
}

/**
 * The muted meta of a row of Reviews in short, its mark telling the user's review (`reviewMark`): "#123 · carol" and,
 * where no mark tells it, how the user takes part: "requested from ide-platform", "you replied"; of an own pull
 * request where it stands ("approved · waiting for bob"); what it leaves out is in the hint
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
      break
    case "reviewed":
      if (involvement.state === null) {
        parts.push("you replied")
      }
      break
    case "authored": {
      const words = authoredWords(request, involvement)
      if (words !== null) {
        parts.push(words)
      }
      break
    }
  }
  return parts.join(" · ")
}

/** What the meta of a row of Reviews leaves out, in its tooltip: "you approved a1b2c3d", the reviewers by name */
export function formatReviewHint(entry: ReviewEntry): string {
  const { request, involvement } = entry
  switch (involvement.kind) {
    case "team":
      return ""
    case "linked":
      return yourReview(request.reviewedState, request.reviewedCommit) ?? "not reviewed"
    case "reviewed":
      return involvement.state === null ? "" : (yourReview(involvement.state, request.reviewedCommit) ?? "")
    case "authored":
      return reviewersWords(involvement) ?? ""
  }
}

/** The mark at the start of a row of Reviews: the user's move, else how a closed one ended, else the user's review of it; none for a team request */
export type ReviewMark = { readonly kind: "move" | "merged" | "closed" | "approved" | "changesRequested" | "commented" | "none"; readonly label: string }

/**
 * The mark of a row of Reviews: a filled dot for the user's move (Rust decides it: `ReviewSection.moves`), which only
 * it stands out by; a closed one how it ended, merged or closed; of the rest the user's review muted: approved, changes
 * requested, commented or replied
 */
export function reviewMark(entry: ReviewEntry, move: boolean): ReviewMark {
  const { request, involvement } = entry
  const ended = closedWord(request)
  if (ended !== null) {
    return { kind: ended, label: capitalize(ended) }
  }
  if (move) {
    return { kind: "move", label: moveWords(entry) }
  }
  const state = involvement.kind === "reviewed" ? involvement.state : involvement.kind === "linked" ? request.reviewedState : null
  if (involvement.kind === "reviewed" && state === null) {
    return { kind: "commented", label: "You replied" }
  }
  switch (state) {
    case "approved":
      return { kind: "approved", label: capitalize(yourReview(state, request.reviewedCommit)!) }
    case "changesRequested":
      return { kind: "changesRequested", label: "You requested changes" }
    case "commented":
      return { kind: "commented", label: "You commented" }
    default:
      return involvement.kind === "authored" && request.reviewDecision === "approved" ? { kind: "approved", label: "Approved" } : { kind: "none", label: "" }
  }
}

/** Why a row is the user's move */
function moveWords({ request, involvement }: ReviewEntry): string {
  switch (involvement.kind) {
    case "linked":
      return request.reviewedState === null ? "Your move: not reviewed yet" : "Your move: new commits since your review"
    case "reviewed":
      return "Your move: new commits since your review"
    case "authored":
      return "Your move: changes requested"
    case "team":
      return "Your move"
  }
}

/**
 * The rows a section of Reviews shows before "Show N More": the user's moves (`moves`, by number) and three more, in
 * its order, as Your Turn shows its unread requests and the oldest (`pickVisibleRequests`); the same list when all show
 */
export function pickVisibleEntries(entries: readonly ReviewEntry[], moves: ReadonlySet<number>): readonly ReviewEntry[] {
  const requests = entries.map(entry => entry.request)
  const keys = new Set(entries.filter(entry => moves.has(entry.request.number)).map(entry => reviewRequestKey(entry.request)))
  // the moves come on top of the three rows, not in their place
  const visible = pickVisibleRequests(requests, keys, VISIBLE_ROW_CAP + keys.size)
  if (visible.length === entries.length) {
    return entries
  }
  const shown = new Set(visible.map(request => request.number))
  return entries.filter(entry => shown.has(entry.request.number))
}

/** What the review window of a merged or closed pull request says of it: "This pull request is merged"; null of an open one */
export function formatClosedNote(request: ReviewRequest): string | null {
  const ended = closedWord(request)
  return ended === null ? null : `This pull request is ${ended}`
}

/**
 * What the age at the end of a row says: "merged 2 days ago" of a closed one, "waiting 2 days" of a team request, "new
 * commits since your review", "updated 3 hours ago"
 */
export function formatReviewAge(entry: ReviewEntry, now: number): string {
  const { request, involvement } = entry
  const ended = closedWord(request)
  if (ended !== null) {
    return `${ended} ${formatRelativeTime((request.closedAt ?? request.updatedAt) * 1000, now)}`
  }
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

/**
 * The GitHub search of a section in the repository, with the words of the filter (`text`) when the filter cut what
 * GitHub returned; null for the links, which no search lists
 */
export function reviewSectionSearchUrl(kind: ReviewSectionKind, repository: { owner: string; name: string }, text = ""): string | null {
  return kind === "linked" ? null : pullsUrl(repository, withSearchText(SECTION_SEARCHES[kind], text))
}

/** Every open pull request of the repository the user takes part in, on GitHub: "Open on GitHub" of the view; with the words of the filter */
export function reviewsSearchUrl(repository: { owner: string; name: string }, text = ""): string {
  return pullsUrl(repository, withSearchText("is:pr is:open involves:@me", text))
}

/**
 * Every pull request of the repository the user takes part in, open or closed, with the words of the filter: "Search on
 * GitHub" of a filter that found nothing
 */
export function allReviewsSearchUrl(repository: { owner: string; name: string }, text: string): string {
  return pullsUrl(repository, withSearchText("is:pr involves:@me", text))
}

/** The closed pull requests the user takes part in whose titles have the words of the filter, on GitHub: the rest of a search that found more than it returned */
export function closedReviewsSearchUrl(repository: { owner: string; name: string }, text: string): string {
  return pullsUrl(repository, withSearchText("is:pr is:closed involves:@me in:title", text))
}

function pullsUrl(repository: { owner: string; name: string }, query: string): string {
  return `https://github.com/${repository.owner}/${repository.name}/pulls?q=${encodeURIComponent(query)}`
}

/**
 * The texts the filter of Reviews searches in a row of a section: what the row shows (its title, "#201", the author,
 * the meta and its hint of how the user takes part: the teams, the reviewers, "you approved"), its branches and its labels; never
 * the URL, whose words every row has
 */
export function reviewEntryTexts(entry: ReviewEntry): string[] {
  const { request } = entry
  return [
    request.title,
    `#${request.number}`,
    request.author ?? "",
    request.headRefName,
    request.baseRefName,
    formatReviewMeta(entry),
    formatReviewHint(entry),
    ...(request.labels ?? []),
  ]
}

/** The texts the filter of Reviews searches in a request of Your Turn: its title, "#101 · alice · requested by dave", its branches and its labels */
export function reviewRequestTexts(request: ReviewRequest): string[] {
  return [request.title, formatRequestMeta(request), request.headRefName, request.baseRefName, ...(request.labels ?? [])]
}

/** The texts the filter of Reviews searches in an own pull request a reviewer handed back: its title, its meta, its branch */
export function returnedTexts(item: ReturnedPullRequest): string[] {
  return [item.pullRequest.title, formatReturnedMeta(item.pullRequest), item.branchName, item.pullRequest.headRefName]
}

/**
 * What the text of the field of Reviews names when it names a pull request: its number (`#123`, `123`), or a link that
 * Rust reads (a URL, `owner/name#123`); null for words, which only filter
 */
export type TypedReference = { readonly kind: "number"; readonly number: number } | { readonly kind: "link"; readonly text: string }

export function typedReference(text: string): TypedReference | null {
  const value = text.trim()
  const number = /^#?(\d+)$/.exec(value)
  if (number !== null) {
    return { kind: "number", number: Number(number[1]) }
  }
  return value !== "" && !/\s/.test(value) && value.includes("/") ? { kind: "link", text: value } : null
}

/**
 * The shown row the reference names: the one of its number, or the one whose URL a pasted link starts with (`/files`
 * and an anchor after it); undefined when no shown row is it, and Rust is asked for the link
 */
export function shownPullRequestOf<T extends { readonly number: number; readonly url: string }>(reference: TypedReference, rows: readonly T[]): T | undefined {
  if (reference.kind === "number") {
    return rows.find(row => row.number === reference.number)
  }
  return rows.find(row => reference.text === row.url || (reference.text.startsWith(row.url) && "/#?".includes(reference.text.charAt(row.url.length))))
}

/** The line of Reviews the filter left empty: `No pull requests match “oauth”` */
export function formatReviewsNoMatch(text: string): string {
  return `No pull requests match “${text.trim()}”`
}

/**
 * Who asks in the review window of a pull request of Reviews: "Added by link", "ide-platform asks for a review", "You
 * reviewed carol's pull request"; of a closed one how it ended first: "Merged: You reviewed carol's pull request"
 */
export function formatAskedBy(request: ReviewRequest, involvement: ReviewInvolvement): string {
  const ended = closedWord(request)
  const asked = askedBy(request, involvement)
  return ended === null ? asked : `${capitalize(ended)}: ${asked}`
}

function askedBy(request: ReviewRequest, involvement: ReviewInvolvement): string {
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

/** A reviewer group of the pull request as Approve words it, with its files among the changes of the shown head */
export interface ApprovalScopeGroup {
  readonly name: string
  /** The user can approve for the group (`ReviewerGroupView.yours`); null while the sign-in names no login */
  readonly yours: boolean | null
  /** The group has its approvals (`ReviewerGroupView.approved`) */
  readonly approved: boolean
  /** Its files among the changes of the pull request; null when Patronus doesn't tell them */
  readonly files: ReadonlySet<string> | null
}

/**
 * What an approval counts for, in the description of Approve and in the confirmation of Approve and Safe Push
 * (openspec/specs/repository/review-requests/spec.md, "Approve"): "Counts for Design · You opened 3 of its 5 files", the user's
 * groups that still need an approval and how many of their files the window showed for this head; "Counts for no group
 * that still needs an approval" when the user is in none; null without groups, and while the sign-in names no login
 (nobody knows whose they are). Information only: it never blocks
 */
export function presentApprovalScope(groups: readonly ApprovalScopeGroup[], opened: ReadonlySet<string>): string | null {
  if (groups.length === 0 || groups.some(group => group.yours === null)) {
    return null
  }
  const yours = groups.filter(group => group.yours === true && !group.approved)
  if (yours.length === 0) {
    return "Counts for no group that still needs an approval"
  }
  const counts = `Counts for ${joinNames(yours.map(group => group.name))}`
  const known = yours.flatMap(group => (group.files === null ? [] : [group.files]))
  if (known.length < yours.length) {
    return counts
  }
  const files = new Set(known.flatMap(paths => [...paths]))
  if (files.size === 0) {
    return counts
  }
  const seen = [...files].filter(path => opened.has(path)).length
  return `${counts} · You opened ${seen} of ${yours.length === 1 ? "its" : "their"} ${files.size} ${files.size === 1 ? "file" : "files"}`
}

/**
 * What an approval counts for in landing where Patronus merges, as Rust decides it of the base of the pull request
 * (`RepositoryView.approvalLanding`; openspec/specs/repository/review-requests/spec.md, "Review window: Approve of a layer
 * says when Patronus counts it"): Patronus counts the approvals of the pull request a Safe Push starts from, into its
 * base. One into another base than the target "Counts once it goes into master: …"; the layer of a stack into the
 * target "Counts for the Safe Push of #301 alone, not of the layers above it"; nothing else, nor without Patronus
 */
export function presentApprovalLanding(landing: ApprovalLanding | undefined, number: number, inStack: boolean): string | null {
  switch (landing?.kind) {
    case "onceIntoTarget":
      return `Counts once it goes into ${landing.target}: Patronus counts the approvals of the pull request a Safe Push starts from`
    case "ownSafePush":
      return inStack ? `Counts for the Safe Push of #${number} alone, not of the layers above it` : null
    case "shown":
      return "Shown: in master already; your review gates no Safe Push, its approval closes the review"
    case undefined:
      return null
  }
}

/** What Approve says it counts for: in landing (`presentApprovalLanding`), then for the groups (`presentApprovalScope`) */
export function joinApprovalScope(landing: string | null, groups: string | null): string | null {
  const parts = [landing, groups].filter(it => it !== null)
  return parts.length === 0 ? null : parts.join(". ")
}

/** A layer of a stack Approve N Pull Requests approves, with what Rust says its approval counts for where Patronus merges */
export interface StackLayerLanding {
  readonly number: number
  /** `RepositoryView.approvalLanding` of `pull:<number>`; undefined without Patronus */
  readonly landing: ApprovalLanding | undefined
}

/**
 * What Approve N Pull Requests counts for where Patronus merges (openspec/specs/repository/review-requests/spec.md, "Review
 * window: Approve N Pull Requests approves every layer the window shows"): "Patronus counts #301 for its own Safe Push,
 * #302 and #303 once each goes into master"; nothing while no layer waits to go into the target, nor without Patronus
 */
export function presentStackApprovalLanding(layers: readonly StackLayerLanding[]): string | null {
  const own = layers.filter(({ landing }) => landing?.kind === "ownSafePush").map(({ number }) => `#${number}`)
  const later = layers.flatMap(({ number, landing }) => (landing?.kind === "onceIntoTarget" ? [{ number, target: landing.target }] : []))
  if (later.length === 0) {
    return null
  }
  const once = `${joinNames(later.map(({ number }) => `#${number}`))} once ${later.length === 1 ? "it goes" : "each goes"} into ${later[0]!.target}`
  return own.length === 0 ? `Patronus counts ${once}` : `Patronus counts ${joinNames(own)} for its own Safe Push, ${once}`
}
