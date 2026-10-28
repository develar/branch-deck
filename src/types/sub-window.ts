import type { MergeConflictInfo } from "@/lib/bindings"
import type { BranchReviewFocus } from "@/lib/branch-review"
import type { MissingCommitOwners } from "@/lib/merge-conflict"
import type { CompareFields } from "@/lib/pull-request"
import type { CreatePullRequestContext, CreatePullRequestInput } from "@/lib/pull-request-dialog"
import type { ToastMessage } from "@/lib/toast-message"

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

/** A reviewer group the code-owner check of Patronus asks for, with the paths it reviews */
export interface BranchReviewGroup {
  name: string
  /** Empty when unknown, or for a group that reviews every change */
  paths: string[]
  branchWide: boolean
}

/**
 * An action on a branch of the stack, as the main window offers it (`getBranchReviewItems`): "Open Pull Request" (O)
 * or "Create Pull Request…", "Safe Push" (S), "Push"; disabled with the reason as its description
 */
export interface BranchReviewItem {
  /** The branch's, or of the review of the whole stack the actions of the whole stack (`getWholeStackReviewItems`) */
  readonly id:
    | "open-pull-request"
    | "create-pull-request"
    | "safe-push"
    | "push-to-baseline"
    | "push"
    | "open-stack-pull-request"
    | "create-stack-pull-request"
    | "safe-push-whole-stack"
    | "push-with-children"
  readonly label: string
  readonly kbds?: string[]
  readonly disabled?: boolean
  /** Its intent runs in the session (`Availability.running`, `queued`): a spinner in place of its icon */
  readonly running?: boolean
  readonly description?: string
  /** The page the window opens itself: the pull request */
  readonly url?: string
}

/**
 * An item of the branch's menu (`getBranchContextMenuItems`) for the "⋯" of the review window: plain data, the main
 * window runs it (`menuItemId`); a submenu stays in the row's menu
 */
export interface BranchReviewMenuItem {
  readonly id: string
  readonly label: string
  readonly disabled?: boolean
  /** Its intent runs in the session: a spinner in place of its icon, a request of it joins it */
  readonly running?: boolean
  readonly description?: string
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

/** A branch of the chain a review of the whole stack came from, a segment of its stack bar that goes back to it */
export interface ReviewedBranch {
  readonly name: string
  readonly commits: number
}

/**
 * What the main window tells the review window of a branch (over its link): what it can do with the branches. The heads
 * a sync built, the baseline and the branch of each commit of a whole stack the window reads from its own session
 */
export interface BranchReviewWindowState {
  readonly repositoryPath: string
  /** The main window shows the repository: only then can it act on its branches */
  readonly selected: boolean
  /** The actions of each branch of the chain; none while the main window shows another repository */
  readonly items: Readonly<Record<string, readonly BranchReviewItem[]>>
  /** The whole menu of each branch of the chain, in its groups: the window's "⋯" offers what the row offers */
  readonly menus: Readonly<Record<string, readonly (readonly BranchReviewMenuItem[])[]>>
  /** What the "Create pull request" dialog of the window shows, while it is open; null otherwise */
  readonly createPullRequest: CreatePullRequestContext | null
  /** The whole stack of the chain's root as a scope of the stack bar; null where it would show nothing more */
  readonly wholeStack: WholeStackScope | null
  /**
   * The menu of each commit of the chain, by its id in the virtual branch, as its row's (`getReviewCommitMenu`): the
   * main window runs the item (`commitMenuItemId`); the window adds the copies, which it runs itself
   */
  readonly commitMenus: Readonly<Record<string, readonly (readonly BranchReviewMenuItem[])[]>>
  /**
   * The reviewer groups of the last code-owner check of the reviewed branch at its head, as the check answers while the
   * window is open (the "Needs approval" chips follow it); null while none answered, the groups of the open stay then
   */
  readonly groups: readonly BranchReviewGroup[] | null
}

/**
 * A request of the review window: the main window runs the action on the branch as its row would (`itemId`), or what
 * the window's "Create pull request" dialog asks (`dialog`): its context while it is open, the creation, the Safe Push
 * without review
 */
export type BranchReviewRequest = {
  readonly id: string
  readonly repositoryPath: string
  readonly branchName: string
  /** Of the whole stack of `branchName`, its root: the actions, the menu and the dialog of the whole stack */
  readonly wholeStack?: boolean
} & (
  | { readonly itemId: "safe-push" | "push-to-baseline" | "push" | "safe-push-whole-stack" | "push-with-children" }
  | { readonly menuItemId: string }
  | { readonly dialog: "open" | "close" | "safePushWithoutReview" }
  | { readonly dialog: "create"; readonly input: CreatePullRequestInput }
  | { readonly dialog: "continueOnGitHub"; readonly fields: CompareFields }
  /** An item of the menu of the commit `commitId` (its id in the virtual branch) of `branchName` */
  | { readonly commitMenuItemId: string; readonly commitId: string }
  /** The window moves to the whole stack of the chain's root, keeping the chain as the way back */
  | { readonly scope: "wholeStack" }
  /** The window moves to the review of `branchName`, a branch of the stack it shows */
  | { readonly review: "branch" }
  /** The window moves to the review of `branchName` at its commit of `commitKey` (`reviewCommitKey`), a commit of the whole stack */
  | { readonly review: "commit"; readonly commitKey: string }
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
  /** The baseline at the open, for the read the page starts at its load; then the one of the window's session */
  baselineBranch: string
  /** The reviewed branch last, its stack parents before it, root first */
  chain: string[]
  /** The reviewer groups the code-owner check asks for, when it ran */
  groups: BranchReviewGroup[]
  /** The whole stack of the root `chain` names alone: its stack branch (`openWholeStackReviewWindow`) */
  wholeStack?: boolean
  /** The review of the whole stack: the chain it came from (the main line from the island), its way back */
  from?: ReviewedBranch[]
  /** Where the review starts: the part of the stack, the files, the commit, the file ("Review Changes" of the Safe Push popover) */
  focus: Omit<BranchReviewFocus, "groups">
  /** A new value on every open: a move to a commit of the chain on screen shows it (`move`) */
  openedAt: number
  /** The open that started the review (`planReviewOpen`): the window starts over when it changes */
  startedAt: number
  /** A move to `focus.commitId`, a commit of the chain on screen: the review keeps its part (when it has the commit) and filter */
  move: boolean
  /** What the main window knows of the chain at the open; it sends what changed over the link */
  state: BranchReviewWindowState
}
