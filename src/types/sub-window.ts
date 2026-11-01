import type { MergeConflictInfo, ReviewCommit } from "@/lib/bindings"
import type { BranchReviewFocus } from "@/lib/branch-review"
import type { MissingCommitOwners } from "@/lib/merge-conflict"
import type { KeptKind } from "@/lib/review-target"

/** `__INIT_DATA__` of the `conflicting-files` sub-window (opened from ConflictingFileListCard) */
export interface ConflictingFilesWindowData {
  /** The repository of the conflict: the window reads the contents of the conflict from it (`getDiffRepositoryPath`) */
  repositoryPath: string
  /** Without its contents: the window reads the merges of its conflicting files (`get_file_merges`) */
  conflict: MergeConflictInfo
  /** Left out of the meta line when unknown */
  branchName?: string
  /** The branch the commit is copied onto (the baseline or the stack parent), for the title */
  onto?: string
  /** The steps of the fix the main window found when it opened the window (`fixStepsFor`) */
  fixSteps?: string[]
}

/** `__INIT_DATA__` of the `missing-commits` sub-window (opened from MissingCommitsDetails) */
export interface MissingCommitsWindowData {
  /** The repository of the conflict: the diffs of the window and the review of a commit of it read it (`getDiffRepositoryPath`) */
  repositoryPath: string
  /** Without its contents: the window reads the diffs of the missing commits (`get_file_diffs`) */
  conflict: MergeConflictInfo
  /** Left out of the meta line when unknown */
  branchName?: string
  /** The branch the commit is copied onto (the baseline or the stack parent), for the title */
  onto?: string
  /** The missing commits by branch, as the main window grouped them: the window has no branch table */
  owners: MissingCommitOwners
  /** What the branch is built on, for the explanation; undefined for a conflict with the amended commit */
  builtOn?: string | null
  /** The steps of the fix the main window found when it opened the window (`fixStepsFor`) */
  fixSteps?: string[]
}

/**
 * A reviewer group the code-owner check of Patronus asks for, with the paths it reviews; of a pull request to review
 * also whether the user can approve for it and whether it has its approvals (`ReviewerGroupView`)
 */
export interface BranchReviewGroup {
  name: string
  /** Empty when unknown, or for a group that reviews every change */
  paths: string[]
  branchWide: boolean
  /** The user can approve for the group (`ReviewerGroupView.yours`); null while the sign-in names no login */
  yours?: boolean | null
  approved?: boolean
}

/**
 * The whole stack of the chain's root, the scope after "From Root" in the stack bar ("Whole Stack"): offered while the
 * stack has branches outside the chain; disabled with the reason while its review can't open
 */
export interface WholeStackScope {
  readonly root: string
  readonly disabled?: boolean
  /** What it holds and lands, or why it can't open */
  readonly description: string
}

/**
 * A branch kept outside HEAD the review shows (`ReviewTarget::Kept`): its full name and where it is kept; the chain of
 * the review is the branch it was, one part of its own commits
 */
export interface KeptReview {
  readonly kind: KeptKind
  /** `{prefix}/unapplied/<name>`, `{prefix}/archived/<date>/<name>` */
  readonly name: string
}

/** A branch of the chain a review of the whole stack came from, a segment of its stack bar that goes back to it */
export interface ReviewedBranch {
  readonly name: string
  readonly commits: number
}

/** What every open of the review window carries: the repository, the open */
interface ReviewWindowOpen {
  repositoryPath: string
  /** A new value on every open: the page decides how it goes on from the review on screen (`planReviewOpen`) */
  openedAt: number
}

/**
 * `__INIT_DATA__` of the review window (`REVIEW_WINDOW`, opened by `@/lib/review-window`): a branch with its stack, the
 * whole stack of a root or a kept branch (`kind: "branch"`), or a list of commits (`kind: "commits"`)
 */
export type ReviewWindowData = BranchReviewWindowData | CommitsReviewWindowData

/** The review of a branch: its chain, the whole stack of its root, or a kept branch (`openBranchReviewWindow`, …) */
export interface BranchReviewWindowData extends ReviewWindowOpen {
  kind: "branch"
  /** The baseline at the open, for the read the page starts at its load; then the one of the window's session */
  baselineBranch: string
  /** The reviewed branch last, its stack parents before it, root first */
  chain: string[]
  /** The reviewer groups the code-owner check asks for, when it ran */
  groups: BranchReviewGroup[]
  /** The whole stack of the root `chain` names alone: its stack branch (`openWholeStackReviewWindow`) */
  wholeStack?: boolean
  /** The kept branch `chain` names by the branch it was (`openKeptReviewWindow`) */
  kept?: KeptReview
  /** The review of the whole stack: the chain it came from (the main line from the island), its way back */
  from?: ReviewedBranch[]
  /** The branch the open is of: the reviewed one, the branch of the commit of `focus` */
  branchName: string
  /** What the open asked: a plain open, a commit, a place (a part, a filter, a file, the groups of a code-owner check) */
  asked: BranchReviewFocus
  /** Where the review starts: the part of the stack, the files, the commit, the file ("Review Changes" of the Safe Push popover) */
  focus: Omit<BranchReviewFocus, "groups">
}

/**
 * The review of a commit of a list (`openCommitReviewWindow`): the commits of the list above its files, ⌥↑/⌥↓ step
 * through them. Commits don't change, so the list the opener passed stays true and nothing else is read.
 */
export interface CommitsReviewWindowData extends ReviewWindowOpen {
  kind: "commits"
  /** The list the commit was reviewed from, as it was */
  commits: ReviewCommit[]
  /** The name of that list, in the title bar: the branch, or "Unassigned"; null for a list without one */
  listLabel: string | null
  index: number
  /** The file to show (Enter on a file of the preview's tree); null for the first one */
  path: string | null
}
