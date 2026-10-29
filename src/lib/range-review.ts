import type { BranchReviewSegment, Interdiff, RangeScope, ReviewCommit, ReviewPlan } from "@/lib/bindings"
import { formatCommitCount } from "@/lib/commit"
import { formatShortHash } from "@/lib/hash"
import { formatPairState } from "@/presenters/review-plan"

/**
 * An item of the commit column of a range review: a scope of the range ("All Changes", "Since Your Review") or one of
 * its commits, newest first. `group` heads the first commit of a branch of a stack; `divider` is a line above a
 * commit, the boundary of the user's last review.
 */
export type RangeReviewItem =
  | { readonly kind: "scope"; readonly key: string; readonly label: string; readonly detail?: string }
  | {
      readonly kind: "commit"
      readonly key: string
      readonly commit: ReviewCommit
      readonly group?: string
      readonly divider?: string
      /** The branch a commit of the whole stack came from: its commits are in the order of the history, not by branch */
      readonly branch?: string
      /** What it is since the user's review of a local branch: "New", "Rewritten" */
      readonly since?: string
    }

/** The key of "All Changes": the whole range */
export const ALL_CHANGES = "all"

/** The key of "Since Your Review": what changed after the commit of the user's last review */
export const SINCE_REVIEW = "since-review"

/**
 * The column of a pull request: "All Changes", "Since Your Review" while there is something after the user's last
 * review (`reviewedCommit`, a commit of the range below its head), then the commits newest first (the range lists them
 * oldest first), the reviewed one under "Your review · a1b2c3d"
 */
export function pullRequestItems(commits: readonly ReviewCommit[], reviewedCommit: string | null): RangeReviewItem[] {
  const items: RangeReviewItem[] = [{ kind: "scope", key: ALL_CHANGES, label: "All Changes", detail: formatCommitCount(commits.length) }]
  const since = reviewedCommit === null ? -1 : commits.findIndex(commit => commit.id === reviewedCommit)
  if (since !== -1 && since < commits.length - 1) {
    items.push({ kind: "scope", key: SINCE_REVIEW, label: "Since Your Review", detail: formatCommitCount(commits.length - 1 - since) })
  }
  for (let index = commits.length - 1; index >= 0; index--) {
    const commit = commits[index]!
    items.push({ kind: "commit", key: commit.id, commit, ...(index === since && { divider: `Your review · ${formatShortHash(commit.id)}` }) })
  }
  return items
}

/** The shown item: the chosen one while the range has it (a scope that is gone, a commit that left), else all changes */
export function shownKey(items: readonly RangeReviewItem[], selected: string): string {
  return items.some(item => item.key === selected) ? selected : ALL_CHANGES
}

/**
 * What changed in a local branch since the user marked it reviewed (spec/branch/review-plan.spec.md), as the column
 * shows it: the commits added or rewritten, by id, newest first, and their plain range when they are the commits on top
 */
export interface LocalSinceReview {
  /** The words of each added or rewritten commit ("New", "Rewritten"), by its id */
  readonly changed: ReadonlyMap<string, string>
  /** The ids of `changed`, newest first, as the column lists them */
  readonly newestFirst: readonly string[]
  /** `<parent of the first added>..tip` when only commits were added on top; null when they are not one range */
  readonly scope: RangeScope | null
}

/**
 * The interdiff of `plan` as the shown part may use it: only when the part is one branch (`ownBranch`; the reviewed
 * pair is on the base of that branch alone, so a part of several branches would count the lower ones as new), and only
 * of a plan of the head on screen (the plan before a sync, kept while the next is read, names commits that may be gone)
 */
export function shownInterdiff(plan: ReviewPlan | null, ownBranch: boolean, head: string | null): Interdiff | null {
  return ownBranch && plan !== null && plan.head === head ? plan.interdiff : null
}

/** "Since Your Review" of the interdiff of the plan: null for a branch never marked, and when nothing was added or rewritten */
export function localSinceReview(interdiff: Interdiff | null): LocalSinceReview | null {
  if (interdiff === null) {
    return null
  }
  const changed = new Map<string, string>()
  for (const commit of interdiff.commits) {
    const words = formatPairState(commit.state)
    if (commit.id !== null && words !== null) {
      changed.set(commit.id, words)
    }
  }
  if (changed.size === 0) {
    return null
  }
  // the interdiff lists the commits of the branch oldest first
  return { changed, newestFirst: [...changed.keys()].reverse(), scope: interdiff.scope }
}

/**
 * The column of a part of a stack: "All Changes", then the commits of each branch, the reviewed (newest) one first.
 * With `since` (a local branch marked reviewed, something changed since), "Since Your Review" after "All Changes", each
 * changed commit says what it is, and `onlySince` lists only them: the added and the rewritten ones, adjacent or not
 */
export function segmentItems(segments: readonly BranchReviewSegment[], since: LocalSinceReview | null = null, onlySince = false): RangeReviewItem[] {
  const count = segments.reduce((sum, segment) => sum + segment.commits.length, 0)
  const items: RangeReviewItem[] = [{ kind: "scope", key: ALL_CHANGES, label: "All Changes", detail: formatCommitCount(count) }]
  if (since !== null) {
    items.push({ kind: "scope", key: SINCE_REVIEW, label: "Since Your Review", detail: formatCommitCount(since.changed.size) })
  }
  for (const segment of [...segments].reverse()) {
    const commits = since !== null && onlySince ? segment.commits.filter(commit => since.changed.has(commit.id)) : segment.commits
    commits.forEach((commit, index) => {
      const words = since?.changed.get(commit.id)
      items.push({
        kind: "commit",
        key: commit.id,
        commit,
        ...(index === 0 && segments.length > 1 && { group: segment.name }),
        ...(words !== undefined && { since: words }),
      })
    })
  }
  return items
}

/** A range of one commit (or a list of one) has nothing to choose: the review shows that commit, without the column */
export function isSingleCommit(items: readonly RangeReviewItem[]): boolean {
  return items.filter(item => item.kind === "commit").length === 1 && items.filter(item => item.kind === "scope").length <= 1
}

/** The column of a list of commits (the commit review window): the commits as the list shows them, no scope */
export function commitItems(commits: readonly ReviewCommit[]): RangeReviewItem[] {
  return commits.map(commit => ({ kind: "commit", key: commit.id, commit }))
}

/**
 * The commit shown after the segments were read again: the same commit, else the commit a sync rebuilt in its place (a
 * new id: the same branch, subject and author time, the nth of such twins); null for all changes
 */
export function keepCommit(previous: readonly BranchReviewSegment[], next: readonly BranchReviewSegment[], commitId: string | null): string | null {
  if (commitId === null || next.some(segment => segment.commits.some(commit => commit.id === commitId))) {
    return commitId
  }
  for (const segment of previous) {
    const commit = segment.commits.find(it => it.id === commitId)
    if (commit === undefined) {
      continue
    }
    const isTwin = (it: ReviewCommit) => it.subject === commit.subject && it.authorTime === commit.authorTime
    const nth = segment.commits.filter(isTwin).indexOf(commit)
    return next.find(it => it.name === segment.name)?.commits.filter(isTwin)[nth]?.id ?? null
  }
  return null
}

/**
 * The place of the selected item for the footer: the commit among the commits, or the label of the scope; nothing for
 * a range of one commit, which has no column
 */
export function rangePosition(items: readonly RangeReviewItem[], selected: string): { commit?: { index: number; count: number }; scope?: string } {
  if (isSingleCommit(items) || items.length === 0) {
    return {}
  }
  const commits = items.filter(item => item.kind === "commit")
  const index = commits.findIndex(item => item.key === selected)
  if (index !== -1) {
    return { commit: { index, count: commits.length } }
  }
  const scope = items.find(item => item.key === selected)
  return { scope: scope?.kind === "scope" ? scope.label : "All Changes" }
}
