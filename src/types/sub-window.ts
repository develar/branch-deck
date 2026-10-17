import type { MergeConflictInfo, MissingCommit } from "@/lib/bindings"
import type { BranchReviewFocus } from "@/lib/branch-review"
import type { CreatePullRequestContext, CreatePullRequestInput } from "@/lib/create-pull-request-context"
import type { MissingCommitOwners } from "@/lib/merge-conflict"
import type { ToastMessage } from "@/lib/toast-message"

/** `__INIT_DATA__` of the `conflicting-files` sub-window (opened from ConflictingFileListCard) */
export interface ConflictingFilesWindowData {
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
  conflictCommitHash: string
  conflictCommitMessage: string
  conflictCommitAuthorTime: number
  conflictCommitCommitterTime: number
  /** Left out of the meta line when unknown */
  branchName?: string
  /** The branch the commit is copied onto (the baseline or the stack parent), for the title */
  onto?: string
  missingCommits: MissingCommit[]
  /** The missing commits by branch, as the main window grouped them: the window has no branch table */
  owners: MissingCommitOwners
  /** What the branch is built on, for the explanation; undefined for a conflict with the amended commit */
  builtOn?: string | null
  /** The steps of the fix the main window found when it opened the window (`fixStepsFor`) */
  fixSteps?: string[]
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
 * or "Create Pull Request…", "Safe Push…" (S), "Push"; disabled with the reason as its description
 */
export interface BranchReviewItem {
  readonly id: "open-pull-request" | "create-pull-request" | "safe-push" | "push"
  readonly label: string
  readonly kbds?: string[]
  readonly disabled?: boolean
  readonly description?: string
  /** The page the window opens itself: the pull request */
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
  /** What the "Create pull request" dialog of the window shows, while it is open; null otherwise */
  readonly createPullRequest: CreatePullRequestContext | null
}

/**
 * A request of the review window: the main window runs the action on the branch as its row would (`itemId`), or what
 * the window's "Create pull request" dialog asks (`dialog`): its context while it is open, the creation, the Safe Push
 * without review
 */
export type BranchReviewRequest = { readonly id: string; readonly repositoryPath: string; readonly branchName: string } & (
  | { readonly itemId: "safe-push" | "push" }
  | { readonly dialog: "open" | "close" | "safePushWithoutReview" }
  | { readonly dialog: "create"; readonly input: CreatePullRequestInput }
)

/**
 * How a request went, shown in the window: why the main window refused it, or the toast of what it did (it recorded
 * both in Activity); a toast about a pull request opens it
 */
export type BranchReviewFeedback =
  | { readonly kind: "error"; readonly title: string; readonly message: string }
  | { readonly kind: "toast"; readonly toast: Omit<ToastMessage, "action">; readonly pullRequestUrl: string | null }

/** `__INIT_DATA__` of the `branch-review` sub-window (opened by `openBranchReviewWindow`) */
export interface BranchReviewWindowData {
  repositoryPath: string
  branchPrefix: string
  baselineBranch: string
  /** The reviewed branch last, its stack parents before it, root first */
  chain: string[]
  /** The reviewer groups the code-owner check asks for, when it ran */
  groups: BranchReviewGroup[]
  /** Where the review starts: the part of the stack, the files, the commit, the file (the code-owner block of the Safe Push form) */
  focus: Omit<BranchReviewFocus, "groups">
  /** A new value on every open: the same branch opened again resets the window */
  openedAt: number
  /** What the main window knows of the chain at the open; it sends what changed over the link */
  state: BranchReviewWindowState
}
