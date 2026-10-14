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
