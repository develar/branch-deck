import type { MergeConflictInfo, MissingCommit } from "@/lib/bindings"

/** `__INIT_DATA__` of the `conflicting-files` sub-window (opened from ConflictingFileListCard) */
export interface ConflictingFilesWindowData {
  conflict: MergeConflictInfo
  branchName: string
}

/** `__INIT_DATA__` of the `missing-commits` sub-window (opened from MissingCommitsDetails) */
export interface MissingCommitsWindowData {
  conflictCommitHash: string
  conflictCommitMessage: string
  conflictCommitAuthorTime: number
  conflictCommitCommitterTime: number
  branchName: string
  missingCommits: MissingCommit[]
  mergeBase?: {
    hash: string
    /** `ConflictAnalysis.mergeBaseSubject`; optional, the Vue opener didn't set it */
    subject?: string
    message: string
    author: string
    time: number
  }
  divergenceSummary: {
    commitsAheadInSource: number
    commitsAheadInTarget: number
  }
}
