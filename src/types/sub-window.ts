import type { MergeConflictInfo, MissingCommit } from "@/lib/bindings"
import type { MergeBase } from "@/lib/merge-conflict"

/** `__INIT_DATA__` of the `conflicting-files` sub-window (opened from ConflictingFileListCard) */
export interface ConflictingFilesWindowData {
  conflict: MergeConflictInfo
  /** Left out of the meta line when unknown */
  branchName?: string
  /** The branch the commit is copied onto (the baseline or the stack parent), for the title */
  onto?: string
}

/** `__INIT_DATA__` of the `missing-commits` sub-window (opened from MissingCommitsDetails) */
export interface MissingCommitsWindowData {
  conflictCommitHash: string
  conflictCommitMessage: string
  conflictCommitAuthorTime: number
  conflictCommitCommitterTime: number
  /** Left out of the meta line when unknown */
  branchName?: string
  /** The branch the commit is copied onto (the baseline or the stack parent), for the title */
  onto?: string
  missingCommits: MissingCommit[]
  mergeBase?: MergeBase
  divergenceSummary: {
    commitsAheadInSource: number
    commitsAheadInTarget: number
  }
}

/** A reviewer group the code-owner check of Patronus asks for, with the paths it reviews */
export interface BranchReviewGroup {
  name: string
  /** Empty when unknown, or for a group that reviews every change */
  paths: string[]
  branchWide: boolean
}

/** What the review window offers for a branch of its stack, as the main window knows it (`getBranchReviewActions`) */
export interface BranchReviewActions {
  /** The pull request of the branch ("Open Pull Request", O); null without one */
  pullRequest: { number: number, url: string } | null
  /**
   * "Safe Push…" (S), where the branch preview offers it: Patronus is on the repository and the branch has work; null
   * otherwise. `reason` says why it can't start now, null when it can.
   */
  safePush: { reason: string | null } | null
  /** "Push", for a branch with work; `reason` says why it can't push now, null when it can */
  push: { reason: string | null } | null
}

/** `__INIT_DATA__` of the `branch-review` sub-window (opened by `openBranchReview`) */
export interface BranchReviewWindowData {
  repositoryPath: string
  branchPrefix: string
  baselineBranch: string
  /** The reviewed branch last, its stack parents before it, root first */
  chain: string[]
  /** The part of the chain shown first: the branch itself, or all of it down to the baseline (what Patronus checks) */
  range: "own" | "all"
  /** The reviewer groups the code-owner check asks for, when it ran */
  groups: BranchReviewGroup[]
  /** The group whose files are shown first; null for all files */
  group: string | null
  /** The file shown first */
  path: string | null
  /** A new value on every open: the same branch opened again resets the window */
  openedAt: number
  /** The actions of each branch of the chain at the open; the main window sends changes (`branch-review-actions-changed`) */
  actions: Record<string, BranchReviewActions>
}
