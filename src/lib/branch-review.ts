import type { BranchReviewSegment } from "@/lib/bindings"
import type { DiffSubject } from "@/lib/commit-files"
import type { ReviewFilter } from "@/lib/review-owners"
import { formatBlockedReason } from "@/lib/stack"
import type { Branch } from "@/stores/branch-sync"
import type { BranchReviewGroup, BranchReviewItem, BranchReviewWindowData } from "@/types/sub-window"

/**
 * The label of the review window of a branch: one window, reused for the next branch. Its link
 * (`@/lib/sub-window-link`) sends the heads a sync built and the actions of each branch of its chain
 * (`BranchReviewWindowState`), and carries the actions it asks for (`BranchReviewRequest`) with their refusals.
 */
export const BRANCH_REVIEW_WINDOW = "branch-review"

/**
 * Where a review of a branch starts: the part of its stack (the branch itself, all of it down to the baseline, or the
 * part between two branches by name), the files (a reviewer group, an owner), a commit of the part, a file: the
 * "Review Changes" of the Safe Push popover opens it at a group or a file.
 */
export interface BranchReviewFocus {
  readonly range?: "own" | "all" | { readonly from: string; readonly to: string }
  readonly filter?: ReviewFilter
  /** One commit of the part; all its changes without it */
  readonly commitId?: string | null
  readonly path?: string | null
  /** The groups of a code-owner check the caller holds; the remembered ones otherwise */
  readonly groups?: BranchReviewGroup[]
}

/** The part of the segments a focus names; the reviewed branch alone for a part whose ends left the chain */
export function focusRange(segments: readonly BranchReviewSegment[], range: BranchReviewFocus["range"]): SegmentRange {
  const last = segments.length - 1
  if (range === "all") {
    return { from: 0, to: last }
  }
  if (range === undefined || range === "own") {
    return { from: last, to: last }
  }
  const from = segments.findIndex(segment => segment.name === range.from)
  const to = segments.findIndex(segment => segment.name === range.to)
  return from === -1 || to === -1 || from > to ? { from: last, to: last } : { from, to }
}

/** What a review shows of its segments: the part and the commit (null for all its changes) */
export interface SegmentView {
  readonly segments: readonly BranchReviewSegment[]
  readonly range: SegmentRange
  readonly commitId: string | null
}

/**
 * The view moved to the commit of `focus` (D on a commit row of a branch of the chain on screen): the part stays when
 * it has the commit, else it is the commit's segment; a commit the segments don't have shows all changes of the part
 * `focus` names (the branch of the commit)
 */
export function moveView<T extends SegmentView>(view: T, focus: BranchReviewFocus): T {
  const index = view.segments.findIndex(segment => segment.commits.some(it => it.id === focus.commitId))
  if (index === -1) {
    return { ...view, range: focusRange(view.segments, focus.range), commitId: null }
  }
  const kept = index >= view.range.from && index <= view.range.to
  return { ...view, range: kept ? view.range : { from: index, to: index }, commitId: focus.commitId! }
}

/** Why a branch of a review has no actions: it is no longer a virtual branch */
export function branchGoneReason(branchName: string): string {
  return `${branchName} is no longer a branch: it landed or was renamed`
}

/** What a request of the review window does, for "Cannot …": "push auth", "create the pull request of auth" */
export function requestAction(
  itemId: Exclude<BranchReviewItem["id"], "open-pull-request"> | "safe-push-without-review" | "continue-on-github" | { menuLabel: string },
  branchName: string,
): string {
  if (typeof itemId === "object") {
    return `run "${itemId.menuLabel.replace(/…$/, "")}" on ${branchName}`
  }
  switch (itemId) {
    case "continue-on-github":
      return `open the pull request of ${branchName} on GitHub`
    case "push":
      return `push ${branchName}`
    case "safe-push":
    case "safe-push-without-review":
      return `start the Safe Push of ${branchName}`
    case "create-pull-request":
      return `create the pull request of ${branchName}`
  }
}

/** Why a branch can't be reviewed now; null when it can: its virtual branch must be built on its parents */
export function branchReviewBlocker(branch: Branch): string | null {
  if (branch.commitCount === 0) {
    return "No commits to review"
  }
  if (branch.status === "Blocked") {
    return formatBlockedReason(branch)
  }
  if (branch.hasError) {
    return "The branch failed to sync"
  }
  return null
}

/** A contiguous part of the chain: the indexes of its first and last segment */
export interface SegmentRange {
  readonly from: number
  readonly to: number
}

/** What a range of segments diffs: the base of the first one to the head of the last one */
export function segmentRangeSubject(segments: readonly BranchReviewSegment[], range: SegmentRange): DiffSubject | null {
  const first = segments[range.from]
  const last = segments[range.to]
  return first === undefined || last === undefined ? null : { kind: "range", base: first.base, head: last.head }
}

/**
 * The range after a click on segment `index`: the segment alone, or with Shift from the current range's anchor (the
 * reviewed end, `to`) to it, always contiguous
 */
export function selectSegment(current: SegmentRange, index: number, extend: boolean): SegmentRange {
  if (!extend) {
    return { from: index, to: index }
  }
  return index <= current.to ? { from: index, to: current.to } : { from: current.from, to: index }
}

/** More segments than this fold the middle of the stack bar */
const UNFOLDED_SEGMENTS = 6

/** An item of the stack bar: a segment, or a fold of the segments `from`..`from + count - 1` ("… 14 more") */
export type StackBarItem = { readonly kind: "segment"; readonly index: number } | { readonly kind: "fold"; readonly from: number; readonly count: number }

/**
 * The stack bar of a long stack, which would wrap over several lines: the root, the ends of the range and the last two
 * segments (the parent and the reviewed branch), each run of two or more others folded; every segment once `unfolded`
 */
export function stackBarItems(count: number, range: SegmentRange, unfolded: boolean): StackBarItem[] {
  const all = Array.from({ length: count }, (_, index): StackBarItem => ({ kind: "segment", index }))
  if (unfolded || count <= UNFOLDED_SEGMENTS) {
    return all
  }
  const kept = new Set([0, range.from, range.to, count - 2, count - 1])
  const items: StackBarItem[] = []
  let index = 0
  while (index < count) {
    if (kept.has(index)) {
      items.push(all[index]!)
      index++
      continue
    }
    let end = index
    while (end < count && !kept.has(end)) {
      end++
    }
    if (end - index === 1) {
      items.push(all[index]!)
    } else {
      items.push({ kind: "fold", from: index, count: end - index })
    }
    index = end
  }
  return items
}

/** The range of the chain by the names of its ends, after the segments were read again (a sync moved a tip) */
export function keepSegmentRange(previous: readonly BranchReviewSegment[], next: readonly BranchReviewSegment[], range: SegmentRange): SegmentRange {
  const from = next.findIndex(segment => segment.name === previous[range.from]?.name)
  const to = next.findIndex(segment => segment.name === previous[range.to]?.name)
  return from === -1 || to === -1 || from > to ? { from: next.length - 1, to: next.length - 1 } : { from, to }
}

/** The tips of the branches by name, for the review windows: the head of each virtual branch's newest commit */
export function branchTips(branches: Readonly<Record<string, Branch>>): Record<string, string> {
  const tips: Record<string, string> = {}
  for (const [name, branch] of Object.entries(branches)) {
    const tip = branch.commits[0]?.hash
    if (tip !== undefined) {
      tips[name] = tip
    }
  }
  return tips
}

/**
 * What the review window of a branch is keyed by: the repository, the chain and the open that started the review
 * (`startedAt`, see `planReviewOpen`). A move to a commit of the chain on screen and a plain open of the branch on
 * screen keep it; another branch or an open at a place start the window over.
 */
export function branchReviewWindowKey(data: BranchReviewWindowData): string {
  return `${data.repositoryPath}\n${data.chain.join("\n")}\n${data.startedAt}`
}

/** What the review window shows, as the main window opened it */
export interface ShownBranchReview {
  readonly repositoryPath: string
  readonly chain: readonly string[]
  readonly wholeStack: boolean
  /** The open that started the review: the key of the window (`branchReviewWindowKey`) */
  readonly startedAt: number
}

/** How an open goes on from the window on screen */
export interface BranchReviewOpen {
  readonly chain: readonly string[]
  readonly startedAt: number
  /**
   * A move to a commit of the chain on screen: the window keeps its review (the part when it has the commit, the
   * filter, an open dialog) and shows the commit
   */
  readonly move: boolean
}

/** An open at a commit and nothing else: D on a commit row of a branch */
export function isCommitFocus(focus: BranchReviewFocus): boolean {
  return focus.commitId != null && (focus.range === undefined || focus.range === "own") && focus.filter === undefined && focus.groups === undefined
}

/**
 * How the open of `branchName` (its `chain`, at `focus`) goes on from the window on screen: a commit of a branch of the
 * chain it shows moves there, a plain open of the branch it shows only brings it to the front; anything else (another
 * branch, a place, the whole stack on screen, another repository) starts it over
 */
export function planReviewOpen(
  shown: ShownBranchReview | null,
  request: { readonly repositoryPath: string; readonly branchName: string; readonly chain: readonly string[]; readonly focus: BranchReviewFocus },
  now: number,
): BranchReviewOpen {
  if (shown !== null && !shown.wholeStack && shown.repositoryPath === request.repositoryPath) {
    if (isCommitFocus(request.focus) && shown.chain.includes(request.branchName)) {
      return { chain: shown.chain, startedAt: shown.startedAt, move: true }
    }
    const isPlain =
      request.focus.range === undefined && request.focus.filter === undefined && request.focus.commitId == null && request.focus.path == null && request.focus.groups === undefined
    if (isPlain && shown.chain.join("\n") === request.chain.join("\n")) {
      return { chain: shown.chain, startedAt: shown.startedAt, move: false }
    }
  }
  return { chain: request.chain, startedAt: now, move: false }
}
