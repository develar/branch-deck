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

/**
 * An action on a branch of the stack, as the main window offers it (`getBranchReviewItems`): "Open Pull Request" (O)
 * or "Create Pull Request", "Safe Push…" (S), "Push"; disabled with the reason as its description
 */
export interface BranchReviewItem {
  readonly id: "open-pull-request" | "create-pull-request" | "safe-push" | "push"
  readonly label: string
  readonly kbds?: string[]
  readonly disabled?: boolean
  readonly description?: string
  /** The page the window opens itself: the pull request, the form of a new one */
  readonly url?: string
}

/** What the main window tells the review window of a branch (over its link) */
export interface BranchReviewWindowState {
  readonly repositoryPath: string
  /** The main window shows the repository: only then can it act on its branches */
  readonly selected: boolean
  /** The head of each branch of the chain the last sync built; a branch not there has left (it landed, it was renamed) */
  readonly tips: Readonly<Record<string, string>>
  /** The actions of each branch of the chain; none while the main window shows another repository */
  readonly items: Readonly<Record<string, readonly BranchReviewItem[]>>
}

/** A request of the review window: the main window runs the action on the branch as its row would */
export interface BranchReviewRequest {
  readonly id: string
  readonly repositoryPath: string
  readonly branchName: string
  readonly itemId: "safe-push" | "push"
}

/** How a request went, shown in the window: why the main window refused it (it recorded it in Activity) */
export interface BranchReviewFeedback {
  readonly kind: "error"
  readonly title: string
  readonly message: string
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
  /** What the main window knows of the chain at the open; it sends what changed over the link */
  state: BranchReviewWindowState
}
