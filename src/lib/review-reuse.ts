import type { ReviewRequest } from "@/lib/bindings"
import type { BranchReviewFocus } from "@/lib/branch-review"
import { formatShortHash } from "@/lib/hash"
import type { ReviewSource } from "@/lib/pull-request-review"
import type { ReviewIntent } from "@/lib/review-request-actions"
import { formatWholeStack } from "@/lib/whole-stack"
import type { BranchReviewWindowData, CommitsReviewWindowData, KeptReview, PullRequestReviewWindowData, ReviewWindowData } from "@/types/sub-window"

/**
 * A review the window shows: the data of the open it started from (or of a move to a commit of it), with the start it
 * belongs to (the key of the review: a new start shows another review) and whether the last open moved it
 */
export type ShownReviewWindow = (BranchReviewWindowData | CommitsReviewWindowData | PullRequestReviewWindowData) & {
  /** The open that started the review: the window starts over when it changes, and the review it left goes to its history */
  readonly startedAt: number
  /** The last open moved the review to a commit of it (`openedAt` of the data names it): it keeps its part and filter */
  readonly move: boolean
}

/*
 * The one review window (openspec/specs/review/review-window/spec.md#one-window): what it shows, the one rule of how an open goes
 * on from it (`planReviewOpen`, which its page applies to every open it gets, `takeOpen`), the title of what it shows,
 * and its history (Back, ⌘[).
 */

/** What the review window shows: the key of its reuse */
export type ShownReview = { readonly repositoryPath: string; readonly startedAt: number } & (
  | { readonly kind: "branch"; readonly chain: readonly string[]; readonly wholeStack: boolean; readonly kept?: KeptReview }
  | { readonly kind: "commits"; readonly listLabel: string | null; readonly ids: readonly string[] }
  | { readonly kind: "pullRequest"; readonly source: ReviewSource; readonly number: number }
)

/** What an open asks the review window to show */
export type ReviewOpenRequest = { readonly repositoryPath: string } & (
  | {
      readonly kind: "branch"
      /** The branch the open is of: the reviewed one, the branch of the commit of `focus` */
      readonly branchName: string
      readonly chain: readonly string[]
      readonly wholeStack: boolean
      readonly kept?: KeptReview
      readonly focus: BranchReviewFocus
    }
  | {
      readonly kind: "commits"
      readonly listLabel: string | null
      readonly ids: readonly string[]
      /** The file to open at (Enter on a file of the preview's tree): a place */
      readonly path: string | null
    }
  | {
      readonly kind: "pullRequest"
      readonly source: ReviewSource
      readonly number: number
      /** A write of the row's menu, or "review" (D on the row): a place */
      readonly intent: ReviewIntent | null
    }
)

/** How an open goes on from the window on screen */
export interface ReviewOpen {
  /** What the window shows after it: the review on screen for a move or a plain open of it */
  readonly shown: ShownReview
  /**
   * A move to a commit of the review on screen: the window keeps its review (the part when it has the commit, the
   * filter, an open dialog) and shows the commit
   */
  readonly move: boolean
}

/** An open at a commit and nothing else: D on a commit row */
function isCommitFocus(focus: BranchReviewFocus): boolean {
  return focus.commitId != null && (focus.range === undefined || focus.range === "own") && focus.filter === undefined && focus.groups === undefined
}

/** An open of the subject alone, at no place: the menu item, D on its row */
function isPlainFocus(focus: BranchReviewFocus): boolean {
  return focus.range === undefined && focus.filter === undefined && focus.commitId == null && focus.path == null && focus.groups === undefined
}

/**
 * How an open goes on from the window on screen, one rule for every subject: an open of the subject on screen only
 * brings the window to the front; a commit of it moves there (a commit of a branch of the chain or of the kept branch, a
 * commit of the list, the pull request on screen after ↑/↓ walked away from it); anything else (another subject, a place
 * such as a file, a reviewer group or a write of a request, another repository) starts the window over, and the review
 * it showed goes to its history
 */
export function planReviewOpen(shown: ShownReview | null, request: ReviewOpenRequest, now: number): ReviewOpen {
  const restart: ReviewOpen = { shown: shownOfRequest(request, now), move: false }
  if (shown === null || shown.repositoryPath !== request.repositoryPath) {
    return restart
  }
  if (shown.kind === "pullRequest" || request.kind === "pullRequest") {
    // the request on screen, opened from the same list, without a write: shown again (↑/↓ may have walked away from it)
    const same = shown.kind === "pullRequest" && request.kind === "pullRequest" && shown.source === request.source && shown.number === request.number
    return same && request.kind === "pullRequest" && request.intent === null ? { shown, move: true } : restart
  }
  if (shown.kind === "commits" || request.kind === "commits") {
    const sameList = shown.kind === "commits" && request.kind === "commits" && shown.listLabel === request.listLabel && shown.ids.join("\n") === request.ids.join("\n")
    return sameList && request.kind === "commits" && request.path === null ? { shown, move: true } : restart
  }
  if (shown.wholeStack !== request.wholeStack || shown.kept?.name !== request.kept?.name) {
    return restart
  }
  // a kept branch and a whole stack are one part: a commit of it is in it
  const ofChain = request.wholeStack || request.kept !== undefined ? shown.chain[0] === request.chain[0] : shown.chain.includes(request.branchName)
  if (isCommitFocus(request.focus) && ofChain && !request.wholeStack) {
    return { shown, move: true }
  }
  if (isPlainFocus(request.focus) && shown.chain.join("\n") === request.chain.join("\n")) {
    return { shown, move: false }
  }
  return restart
}

/** What the window shows after an open of `request` starts it over at `now` */
function shownOfRequest(request: ReviewOpenRequest, now: number): ShownReview {
  const { repositoryPath } = request
  switch (request.kind) {
    case "commits":
      return { kind: "commits", repositoryPath, listLabel: request.listLabel, ids: request.ids, startedAt: now }
    case "pullRequest":
      return { kind: "pullRequest", repositoryPath, source: request.source, number: request.number, startedAt: now }
    case "branch":
      return { kind: "branch", repositoryPath, chain: request.chain, wholeStack: request.wholeStack, ...(request.kept !== undefined && { kept: request.kept }), startedAt: now }
  }
}

/** What the window shows of `data`, the key `planReviewOpen` keeps or starts over */
export function shownReviewOf(data: ShownReviewWindow): ShownReview {
  const { repositoryPath, startedAt } = data
  switch (data.kind) {
    case "commits":
      return { kind: "commits", repositoryPath, listLabel: data.listLabel, ids: data.commits.map(it => it.id), startedAt }
    case "pullRequest":
      return { kind: "pullRequest", repositoryPath, source: data.source, number: data.number, startedAt }
    case "branch":
      return { kind: "branch", repositoryPath, chain: data.chain, wholeStack: data.wholeStack === true, ...(data.kept !== undefined && { kept: data.kept }), startedAt }
  }
}

/**
 * How the window names what it shows: "login", "the whole air stack", of a list "a1b2c3d Sort the start-up report";
 * the reviewed branch of a move is the branch of its commit
 */
export function reviewSubjectName(data: ReviewWindowData): string {
  if (data.kind === "pullRequest") {
    return pullRequestName(data.request)
  }
  if (data.kind === "commits") {
    const commit = data.commits[data.index]
    return commit === undefined ? (data.listLabel ?? "commits") : `${formatShortHash(commit.id)} ${commit.subject}`
  }
  if (data.wholeStack === true) {
    return formatWholeStack(data.chain[0] ?? "")
  }
  const range = data.focus.range
  return typeof range === "object" ? range.to : (data.chain.at(-1) ?? "")
}

/** "#101 Sort the start-up report": how the window names a pull request */
export function pullRequestName(request: Pick<ReviewRequest, "number" | "title">): string {
  return `#${request.number} ${request.title}`
}

/**
 * "Review login", "Review login · 1a2b3c4d" at a commit, "Review a1b2c3d Sort the start-up report", "Review #101 Sort the
 * start-up report": the native title
 */
export function reviewWindowTitle(data: ReviewWindowData): string {
  const name = `Review ${reviewSubjectName(data)}`
  return data.kind === "branch" && data.focus.commitId != null ? `${name} · ${formatShortHash(data.focus.commitId)}` : name
}

/**
 * Where the review on screen is, for its restore: of a branch its part, filter, commit and file, of a list its commit
 * and file
 */
export type ReviewPlace =
  | { readonly kind: "branch"; readonly focus: BranchReviewWindowData["focus"] }
  | { readonly kind: "commits"; readonly index: number; readonly path: string | null }
  /** The request of the list the window walked to (↑/↓) */
  | { readonly kind: "pullRequest"; readonly request: ReviewRequest; readonly index: number }

/** What the open `data` asks the window to show */
function openRequestOf(data: ReviewWindowData): ReviewOpenRequest {
  const { repositoryPath } = data
  switch (data.kind) {
    case "commits":
      return { kind: "commits", repositoryPath, listLabel: data.listLabel, ids: data.commits.map(it => it.id), path: data.path }
    case "pullRequest":
      return { kind: "pullRequest", repositoryPath, source: data.source, number: data.number, intent: data.intent }
    case "branch":
      return {
        kind: "branch",
        repositoryPath,
        branchName: data.branchName,
        chain: data.chain,
        wholeStack: data.wholeStack === true,
        ...(data.kept !== undefined && { kept: data.kept }),
        focus: data.asked,
      }
  }
}

/**
 * How the open `data` goes on from the review on screen (`planReviewOpen`): what the window shows after it, and whether
 * it started over (the review it showed goes to its history). A move keeps the review on screen and shows the commit of
 * the open: a branch falls back to the part of the commit's branch when the part on screen doesn't have it, a list
 * selects it, a pull request shows itself again; a plain open of what it shows keeps it as it is.
 */
export function takeOpen(shown: ShownReviewWindow | null, data: ReviewWindowData, now: number): { readonly shown: ShownReviewWindow; readonly restarted: boolean } {
  const open = planReviewOpen(shown === null ? null : shownReviewOf(shown), openRequestOf(data), now)
  if (shown === null || open.shown.startedAt !== shown.startedAt) {
    return { shown: { ...data, startedAt: open.shown.startedAt, move: false }, restarted: shown !== null }
  }
  if (!open.move) {
    return { shown, restarted: false }
  }
  if (shown.kind === "commits" && data.kind === "commits") {
    return { shown: { ...shown, index: data.index, path: data.path, openedAt: data.openedAt, move: true }, restarted: false }
  }
  if (shown.kind === "pullRequest" && data.kind === "pullRequest") {
    return { shown: { ...shown, number: data.number, request: data.request, index: data.index, openedAt: data.openedAt, move: true }, restarted: false }
  }
  if (shown.kind === "branch" && data.kind === "branch") {
    const focus = { range: { from: data.branchName, to: data.branchName }, filter: shown.focus.filter, commitId: data.focus.commitId, path: data.focus.path }
    return { shown: { ...shown, focus, openedAt: data.openedAt, move: true }, restarted: false }
  }
  return { shown: { ...data, startedAt: now, move: false }, restarted: true }
}

/** The reviews the window showed before, the latest last: at most this many */
export const REVIEW_HISTORY_LIMIT = 20

/** The history after the window started over: the review it left last, the oldest dropped past the limit */
export function pushReviewHistory(history: readonly ShownReviewWindow[], left: ShownReviewWindow): ShownReviewWindow[] {
  return [...history, left].slice(-REVIEW_HISTORY_LIMIT)
}

/**
 * The review `data` of the history restored at `place` (where it was when the window left it): a start over of it at
 * `now`, which reads it again
 */
export function restoredReview(data: ShownReviewWindow, place: ReviewPlace | undefined, now: number): ShownReviewWindow {
  const open = { openedAt: now, startedAt: now, move: false }
  switch (data.kind) {
    case "commits":
      return { ...data, ...open, ...(place?.kind === "commits" && { index: place.index, path: place.path }) }
    case "pullRequest":
      // the request it showed, without the write the open asked for: that was offered once
      return { ...data, ...open, intent: null, ...(place?.kind === "pullRequest" && { number: place.request.number, request: place.request, index: place.index }) }
    case "branch":
      return { ...data, ...open, ...(place?.kind === "branch" && { focus: place.focus }) }
  }
}
