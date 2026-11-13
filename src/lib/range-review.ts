import type { ReviewSegment, Interdiff, RangeReadingView, RangeScope, ReviewCommit, ReviewPlan, ReviewRequest } from "@/lib/bindings"
import { formatCommitCount } from "@/lib/commit"
import { formatShortHash } from "@/lib/hash"
import { describeUnproven, formatCompactReading, formatReading } from "@/presenters/range-reading"
import { formatPairState } from "@/presenters/review-plan"

/**
 * An item of the commit column of a range review: a scope of the range ("All Changes", "Since Your Review") or one of
 * its commits, newest first. `group` heads the first commit of a branch of a stack; `divider` is a line above a
 * commit, the boundary of the user's last review.
 */
export type RangeReviewItem =
  | {
      readonly kind: "scope"
      readonly key: string
      readonly label: string
      readonly detail?: string
      /** The ids of the commits it diffs when they are not all those of the column: "Since Your Review" of a pull request */
      readonly commits?: ReadonlySet<string>
      /** What the layer of a stack leaves to read, once read */
      readonly reading?: ScopeReading
    }
  | {
      readonly kind: "commit"
      readonly key: string
      readonly commit: ReviewCommit
      readonly group?: string
      readonly divider?: string
      /** "Your review": the commit of the user's last review, which `divider` heads in the column */
      readonly review?: string
      /** The branch a commit of the whole stack came from: its commits are in the order of the history, not by branch */
      readonly branch?: string
      /** What it is since the user's review of a local branch: "New", "Rewritten" */
      readonly since?: string
    }

/**
 * What a layer of a stack leaves to read as its scope in the column says it: a compact chip ("Nothing to read", "4 to
 * read"), its hover the whole verdict and the files not proven; a layer with `nothing` to read is muted
 */
export interface ScopeReading {
  readonly text: string
  readonly hint: string
  readonly nothing: boolean
}

/** The key of "All Changes": the whole range */
export const ALL_CHANGES = "all"

/** The key of "Since Your Review": what changed after the commit of the user's last review */
export const SINCE_REVIEW = "since-review"

/**
 * The column of a pull request: "All Changes", "Since Your Review" while there is something after the user's last
 * review (`reviewedCommit`, a commit of the range below its head), then the commits newest first (the range lists them
 * oldest first), the reviewed one under "Your review · a1b2c3d"; "Since Your Review" names the commits after it
 */
export function pullRequestItems(commits: readonly ReviewCommit[], reviewedCommit: string | null): RangeReviewItem[] {
  const items: RangeReviewItem[] = [{ kind: "scope", key: ALL_CHANGES, label: "All Changes", detail: formatCommitCount(commits.length) }]
  const since = reviewedCommit === null ? -1 : commits.findIndex(commit => commit.id === reviewedCommit)
  if (since !== -1 && since < commits.length - 1) {
    const after = commits.slice(since + 1)
    items.push({ kind: "scope", key: SINCE_REVIEW, label: "Since Your Review", detail: formatCommitCount(after.length), commits: new Set(after.map(commit => commit.id)) })
  }
  for (let index = commits.length - 1; index >= 0; index--) {
    const commit = commits[index]!
    items.push({ kind: "commit", key: commit.id, commit, ...(index === since && { divider: `Your review · ${formatShortHash(commit.id)}`, review: "Your review" }) })
  }
  return items
}

/** What the scope of a layer says it leaves to read */
function scopeReading(view: RangeReadingView): ScopeReading {
  const unproven = describeUnproven(view)
  return {
    text: formatCompactReading(view.summary),
    hint: [formatReading(view.summary), unproven].filter(it => it !== null).join(". "),
    nothing: view.summary.verdict.kind === "nothingToRead",
  }
}

/** The key of the scope of a layer of a stack (Review Stack): its own changes */
export function layerScopeKey(number: number): string {
  return `layer:${number}`
}

/** A layer of a stack as its column shows it: its pull request and, once fetched, its own range */
export interface StackColumnLayer {
  readonly request: Pick<ReviewRequest, "number" | "title">
  /** Its commits, oldest first; null while the stack is fetched */
  readonly commits: readonly ReviewCommit[] | null
  /** Not built on the head of the layer below (`ReviewStackLayer.restacked`) */
  readonly restacked: boolean
}

/**
 * The column of a stack of pull requests (Review Stack): "All Changes" of the whole stack (`total` commits), a scope per
 * layer in the order of the card, the bottom first ("#202 Drop workTreeRoot", what it leaves to read once `readings`
 * has it, by its place, its commits, "not rebased" for one not on the head below), then the commits of the layers
 * newest first, each layer under its heading; a commit two layers list (a layer not rebased has the older commits of
 * the one below) once
 */
export function stackItems(layers: readonly StackColumnLayer[], total: number, readings: readonly (RangeReadingView | null)[] | null = null): RangeReviewItem[] {
  const items: RangeReviewItem[] = [{ kind: "scope", key: ALL_CHANGES, label: "All Changes", detail: formatCommitCount(total) }]
  for (const [index, layer] of layers.entries()) {
    const count = layer.commits === null ? null : formatCommitCount(layer.commits.length)
    const detail = layer.restacked ? [count, "not rebased"].filter(it => it !== null).join(" · ") : count
    const reading = readings?.[index] ?? null
    items.push({
      kind: "scope",
      key: layerScopeKey(layer.request.number),
      label: `#${layer.request.number} ${layer.request.title}`,
      ...(detail !== null && { detail }),
      ...(layer.commits !== null && { commits: new Set(layer.commits.map(commit => commit.id)) }),
      ...(reading !== null && { reading: scopeReading(reading) }),
    })
  }
  const listed = new Set<string>()
  for (const layer of [...layers].reverse()) {
    let first = true
    for (const commit of [...(layer.commits ?? [])].reverse()) {
      if (listed.has(commit.id)) {
        continue
      }
      listed.add(commit.id)
      items.push({ kind: "commit", key: commit.id, commit, ...(first && { group: `#${layer.request.number} ${layer.request.title}` }) })
      first = false
    }
  }
  return items
}

/**
 * The layer of the item `key` of the column of a stack: its scope, a commit of it (of the highest layer that has it, as
 * the column lists it); null for All Changes
 */
export function layerOfItem(layers: readonly StackColumnLayer[], key: string): number | null {
  const layer = layers.find(it => layerScopeKey(it.request.number) === key) ?? layers.findLast(it => it.commits?.some(commit => commit.id === key) === true)
  return layer?.request.number ?? null
}

/**
 * The commits the tree of the scope `scopeKey` lists in its steps (openspec/specs/branch/review-tour/spec.md, "In the Review
 * Window"): only those the scope diffs (`commits` of a scope that is not all of them), each with its branch, which
 * the column says by the heading of a branch (`group`) and the tree, which orders them by step, on each row
 */
export function storyCommits(items: readonly RangeReviewItem[], scopeKey: string): Extract<RangeReviewItem, { kind: "commit" }>[] {
  const scope = items.find(item => item.key === scopeKey)
  const inScope = scope?.kind === "scope" ? scope.commits : undefined
  let group: string | undefined
  return items.flatMap(item => {
    if (item.kind !== "commit") {
      return []
    }
    group = item.group ?? group
    if (inScope !== undefined && !inScope.has(item.commit.id)) {
      return []
    }
    const branch = item.branch ?? group
    return [branch === undefined ? item : { ...item, branch }]
  })
}

/**
 * What an empty scope of a range says while it has commits: they change files one by one, but undo each other (a fixup
 * that reverts its commit), so the range from its base to its head changes none
 */
export function formatRangeWithoutChanges(commitCount: number): string {
  return commitCount === 1 ? "Its commit changes no file" : `The ${commitCount} commits cancel each other out: together they change no file`
}

/** The shown item: the chosen one while the range has it (a scope that is gone, a commit that left), else all changes */
export function shownKey(items: readonly RangeReviewItem[], selected: string): string {
  return items.some(item => item.key === selected) ? selected : ALL_CHANGES
}

/**
 * What changed in a local branch since the user marked it reviewed (openspec/specs/branch/review-plan/spec.md), as the column
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

/**
 * The reviewed commit the plan of the commit `id` shown alone reads its message with (`ReviewSubject` `reviewed`): of a
 * commit whose message Rust says changed since the review (`PairedCommit.messageChange`), so its story marks how; null
 * otherwise. The plan names a commit as the range does, in full or short: they match by prefix
 */
export function reviewedCommitOf(interdiff: Interdiff | null, id: string): string | null {
  const paired = interdiff?.commits.find(commit => commit.id !== null && (id.startsWith(commit.id) || commit.id.startsWith(id)))
  return paired?.messageChange == null ? null : paired.reviewedId
}

/** "Since Your Review" of the interdiff of the plan: null for a branch never marked, and when nothing was added, rewritten or reworded */
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
export function segmentItems(segments: readonly ReviewSegment[], since: LocalSinceReview | null = null, onlySince = false): RangeReviewItem[] {
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
export function keepCommit(previous: readonly ReviewSegment[], next: readonly ReviewSegment[], commitId: string | null): string | null {
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
