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
 * code-owner block of the Safe Push form opens it at a group or a file.
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

/** Why a branch of a review has no actions: it is no longer a virtual branch */
export function branchGoneReason(branchName: string): string {
  return `${branchName} is no longer a branch: it landed or was renamed`
}

/** What a request of the review window does, for "Cannot …": "push auth", "create the pull request of auth" */
export function requestAction(itemId: Exclude<BranchReviewItem["id"], "open-pull-request"> | "safe-push-without-review", branchName: string): string {
  switch (itemId) {
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
 * What the review window of a branch is keyed by: another branch, or an open at a place (a part, a filter, a commit, a
 * file: the code-owner block of the Safe Push form) starts it over; a plain open of the branch on screen only brings the
 * window to the front, where the review is
 */
export function branchReviewWindowKey(data: BranchReviewWindowData): string {
  const subject = `${data.repositoryPath}\n${data.chain.join("\n")}`
  const { range, filter, commitId, path } = data.focus
  const isPlain = range === "own" && filter?.kind === "all" && commitId === null && path === null
  return isPlain ? subject : `${subject}\n${data.openedAt}`
}
