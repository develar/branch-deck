import type { BranchReviewSegment, ReviewCommit } from "@/lib/bindings"
import { formatCommitCount } from "@/lib/commit"
import { formatShortHash } from "@/lib/hash"

/**
 * An item of the commit column of a range review: a scope of the range ("All Changes", "Since Your Review") or one of
 * its commits, newest first. `group` heads the first commit of a branch of a stack; `divider` is a line above a
 * commit, the boundary of the user's last review.
 */
export type RangeReviewItem
  = | { readonly kind: "scope", readonly key: string, readonly label: string, readonly detail?: string }
    | { readonly kind: "commit", readonly key: string, readonly commit: ReviewCommit, readonly group?: string, readonly divider?: string }

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

/** The column of a part of a stack: "All Changes", then the commits of each branch, the reviewed (newest) one first */
export function segmentItems(segments: readonly BranchReviewSegment[]): RangeReviewItem[] {
  const count = segments.reduce((sum, segment) => sum + segment.commits.length, 0)
  const items: RangeReviewItem[] = [{ kind: "scope", key: ALL_CHANGES, label: "All Changes", detail: formatCommitCount(count) }]
  for (const segment of [...segments].reverse()) {
    segment.commits.forEach((commit, index) => {
      items.push({ kind: "commit", key: commit.id, commit, ...(index === 0 && segments.length > 1 && { group: segment.name }) })
    })
  }
  return items
}

/** A range of one commit has nothing to choose: the review shows that commit, without the column */
export function isSingleCommit(items: readonly RangeReviewItem[]): boolean {
  return items.length === 2 && items[0]!.kind === "scope" && items[1]!.kind === "commit"
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
export function rangePosition(items: readonly RangeReviewItem[], selected: string): { commit?: { index: number, count: number }, scope?: string } {
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
