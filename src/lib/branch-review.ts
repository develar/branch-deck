import type { ActionState, ReviewSegment } from "@/lib/bindings"
import type { DiffSubject } from "@/lib/commit-files"
import { formatShortHash } from "@/lib/hash"
import { sameFields } from "@/lib/record"
import type { ReviewFilter } from "@/lib/review-owners"
import { getRowPart } from "@/model/store"
import { actionBlocker } from "@/presenters/actions"
import type { ArchivedBranchesState } from "@/stores/archived-branches"
import { structureIn } from "@/stores/branch-sync"
import type { Branch, BranchSyncState } from "@/stores/branch-sync"
import { getOperationWords } from "@/stores/indicators"
import type { UnappliedBranchesState } from "@/stores/unapplied-branches"
import type { BranchReviewGroup, KeptReview } from "@/types/sub-window"

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
export function focusRange(segments: readonly ReviewSegment[], range: BranchReviewFocus["range"]): SegmentRange {
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
  readonly segments: readonly ReviewSegment[]
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

/**
 * A commit across its copies, which a review of the whole stack matches to the branch it came from (the stack branch
 * has no mapping notes): its author, author time and subject without the prefix (a copy's subject, a row's
 * `strippedSubject`), as the robot's commits are matched
 */
export function reviewCommitKey(commit: { readonly author: string; readonly authorTime: number; readonly subject: string }): string {
  return `${commit.author}\n${commit.authorTime}\n${commit.subject}`
}

/** Why a branch of a review has no actions: it is no longer a virtual branch */
export function branchGoneReason(branchName: string): string {
  return `${branchName} is no longer a branch: it landed or was renamed`
}

/**
 * Why the branch `branch` can't be reviewed now, as Rust decides its Review Changes (`domain_policy::actions`: no
 * commits, its build blocked or failed; its virtual branch must be built on its parents); null when it can. `actions`:
 * the actions of the branch a component subscribed to (`useRowPart`); the words of an operation that runs it are of the
 * operations of the session now
 */
export function reviewBlockerOf(branch: Pick<Branch, "name" | "statusText">, actions: readonly ActionState[] | undefined = getRowPart(branch.name, "actions")): string | null {
  return actionBlocker(actions, "review-changes", { ...getOperationWords(), subject: branch.name, statusText: branch.statusText }) ?? null
}

/** A contiguous part of the chain: the indexes of its first and last segment */
export interface SegmentRange {
  readonly from: number
  readonly to: number
}

/** What a range of segments diffs: the base of the first one to the head of the last one */
export function segmentRangeSubject(segments: readonly ReviewSegment[], range: SegmentRange): DiffSubject | null {
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
export function keepSegmentRange(previous: readonly ReviewSegment[], next: readonly ReviewSegment[], range: SegmentRange): SegmentRange {
  const from = next.findIndex(segment => segment.name === previous[range.from]?.name)
  const to = next.findIndex(segment => segment.name === previous[range.to]?.name)
  return from === -1 || to === -1 || from > to ? { from: next.length - 1, to: next.length - 1 } : { from, to }
}

/**
 * What the review window reads of the table of its own session (`branchSyncStore` of its window): the head of each
 * branch of the chain (a branch not there has left: it landed, it was renamed), of a whole stack the tip of its stack branch,
 * the baseline, and of a whole stack the branch of each of its commits by `reviewCommitKey` (its copies carry no
 * mapping notes)
 */
export interface ReviewedTable {
  readonly tips: Readonly<Record<string, string>>
  readonly baselineBranch: string
  readonly stackCommits: Readonly<Record<string, string>>
}

/**
 * The table of `state` as the review of `chain` reads it, once the view settled: null while a sync runs (its copies are
 * being rebuilt) and before a sync grouped the branches (a session nobody synced yet has no baseline)
 */
export function settledReviewTable(state: BranchSyncState, chain: readonly string[], wholeStack: boolean): ReviewedTable | null {
  if (state.isSyncing || state.baselineBranch === null) {
    return null
  }
  const tips: Record<string, string> = {}
  const stackCommits: Record<string, string> = {}
  if (wholeStack) {
    const root = chain[0] ?? ""
    const tip = state.stackBranches[root]?.tip
    if (tip != null) {
      tips[root] = tip
    }
    for (const name of [root, ...structureIn(state, root).descendants]) {
      for (const commit of state.branches[name]?.commits ?? []) {
        stackCommits[reviewCommitKey({ ...commit, subject: commit.strippedSubject })] = name
      }
    }
  } else {
    for (const name of chain) {
      // its newest commit's copy; a commit the sync wrote no copy of (a failed or blocked build) is its own head, so a
      // branch is gone only once it left the table
      const newest = state.branches[name]?.commits[0]
      if (newest !== undefined) {
        tips[name] = newest.hash ?? newest.originalHash
      }
    }
  }
  return { tips, baselineBranch: state.baselineBranch, stackCommits }
}

/**
 * The table of `state` as the review of the kept branch `kept` (the branch it was `branchName`) reads it, once the view
 * settled: the tip of an unapplied branch, an archived one there with no tip (no sync moves it), nothing once it is gone
 */
export function settledKeptTable(
  state: BranchSyncState,
  kept: KeptReview,
  branchName: string,
  unapplied: UnappliedBranchesState,
  archived: ArchivedBranchesState,
): ReviewedTable | null {
  if (state.isSyncing || state.baselineBranch === null) {
    return null
  }
  const tip = kept.kind === "unapplied" ? unapplied.branches[kept.name]?.tip : archived.branches[kept.name] === undefined ? undefined : ""
  return { tips: tip === undefined ? {} : { [branchName]: tip }, baselineBranch: state.baselineBranch, stackCommits: {} }
}

/** The same table for the review: a view that changed nothing it reads keeps the record, and the window renders nothing */
export function sameReviewTable(a: ReviewedTable | null, b: ReviewedTable | null): boolean {
  return a === b || (a !== null && b !== null && a.baselineBranch === b.baselineBranch && sameFields(a.tips, b.tips) && sameFields(a.stackCommits, b.stackCommits))
}

/** "Unapplied on 1a2b3c4d": where the kept branch is kept and the commit its own commits start on */
export function formatKeptOrigin(kept: KeptReview, base: string | undefined): string {
  const where = kept.kind === "unapplied" ? "Unapplied" : "Archived"
  return base === undefined ? where : `${where} on ${formatShortHash(base)}`
}

/** Why the review of a kept branch shows what it read last: it was reapplied or deleted */
export function keptGoneReason(kept: KeptReview, branchName: string): string {
  return kept.kind === "unapplied" ? `${branchName} is no longer unapplied: it was reapplied or deleted` : `The archived branch ${branchName} was deleted`
}
