import type { BranchReviewSegment } from "@/lib/bindings"
import type { DiffSubject } from "@/lib/commit-files"
import { formatBlockedReason } from "@/lib/stack"
import type { Branch } from "@/stores/branch-sync"
import type { BranchReviewActions } from "@/types/sub-window"

/** The event the main window sends after a sync moved the tips of the branches; a review window reads its stack again */
export const BRANCH_TIPS_CHANGED = "branch-tips-changed"

export interface BranchTipsChanged {
  repositoryPath: string
  /** The head of each virtual branch by name */
  tips: Record<string, string>
}

/**
 * The event the main window sends when what the review window can do with the branches of its stack changed (a pull
 * request, the Patronus robots, a sync or a push), deduplicated
 */
export const BRANCH_REVIEW_ACTIONS_CHANGED = "branch-review-actions-changed"

export interface BranchReviewActionsChanged {
  repositoryPath: string
  /** The actions of each branch of the chain the main window opened last; none while it shows another repository */
  branches: Record<string, BranchReviewActions>
}

/** The event the review window sends for an action on a branch: the window only asks, the main window runs it */
export const BRANCH_ACTION = "branch-action"

export interface BranchActionRequest {
  repositoryPath: string
  branchName: string
  /** "safe-push" opens the Safe Push form of the row, "push" pushes as the push button of the row does */
  action: "safe-push" | "push"
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
export type StackBarItem
  = | { readonly kind: "segment", readonly index: number }
    | { readonly kind: "fold", readonly from: number, readonly count: number }

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
    }
    else {
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
