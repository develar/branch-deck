import { CheckIcon, CopyIcon, GitMergeIcon, GitPullRequestIcon, LinkIcon, MessageSquareDiffIcon, UserCheckIcon } from "lucide-react"
import type { ActionId, ActionState, ReviewRequest } from "@/lib/bindings"
import { copyText } from "@/lib/clipboard"
import { formatShortHash } from "@/lib/hash"
import type { MenuItem, MenuItemGroups } from "@/lib/menu"
import { findAction, formatActionBlocker, presentAction } from "@/presenters/actions"
import type { ReviewAction } from "@/stores/review-requests"

/*
 * The actions of a pull request that waits for the user's review (docs/guidelines/ux-writing.md#actions): one list for
 * the menu of its row in Your Turn and the header of its review window, in the overview and in the review of its changes
 * alike.
 */

/**
 * Where the actions show: the row menu opens the review window for a write (approving needs the commit the user saw);
 * the window is the review
 */
export type ReviewActionSurface = "rowMenu" | "window"

/** What an open of the window asks for: a write offered at once, or the review of the changes (D on the row) */
export type ReviewIntent = "approve" | "safePush" | "requestChanges" | "review"

/** The commit the user reviewed, or why there is none yet */
export type ReviewedCommit = { readonly id: string } | { readonly id: null; readonly reason: string }

export interface ReviewRequestActionContext {
  readonly reviewedCommit: ReviewedCommit
  /**
   * The actions of the pull request as Rust decides them (`RepositoryView.actions` of `pull:<number>`,
   * `domain_policy::actions::review_actions`): an own one takes no review, an approval of its head is done, its Safe
   * Push where Patronus merges into its base; undefined while the session has none of it (another repository)
   */
  readonly actions: readonly ActionState[] | undefined
  /** What runs for it now */
  readonly busy: ReviewAction | undefined
  readonly onReview: () => void
  readonly onApprove: () => void
  readonly onSafePush: () => void
  readonly onRequestChanges: () => void
  readonly onOpenPullRequest: () => void
  /** An own pull request of the table: its branch row shows */
  readonly onShowBranch?: () => void
  /** A pull request the user added by its link leaves Reviews */
  readonly onRemoveFromReviews?: () => void
}

/** The actions by name: the header of the review places them, a menu lists them in groups (`reviewRequestMenu`) */
export interface ReviewRequestActions {
  readonly review: MenuItem
  readonly approve: MenuItem
  /** The user approved the commit the review shows: Approve is done, Safe Push… is the next step */
  readonly approved: boolean
  /** Absent where Patronus doesn't merge */
  readonly safePush: MenuItem | null
  readonly requestChanges: MenuItem
  readonly openPullRequest: MenuItem
  readonly copyLink: MenuItem
  readonly copyBranchName: MenuItem
  /** Of an own pull request of the table */
  readonly showBranch: MenuItem | null
  /** Of a pull request the user added by its link */
  readonly removeFromReviews: MenuItem | null
}

/** The commit the user approved last of the pull request, as Rust knows it: the blocker of its Approve */
export function approvedCommitOf(actions: readonly ActionState[] | undefined): string | null {
  const approve = findAction(actions, "approve")
  return approve?.availability.kind === "disabled" && approve.availability.reason.code === "approved" ? approve.availability.reason.commit : null
}

/** Why Rust refuses the action `id` of `request` other than an approval (which the surface words by its commit); null while it runs */
function refusal(actions: readonly ActionState[] | undefined, id: ActionId, request: ReviewRequest): string | null {
  const action = findAction(actions, id)
  if (action?.availability.kind !== "disabled" || action.availability.reason.code === "approved") {
    return null
  }
  return formatActionBlocker(id, action.availability.reason, { subject: `#${request.number}` })
}

/**
 * The actions of `request`. In the review window a write runs on the commit the user reviewed and is disabled with the
 * reason until there is one; the keys are the review's (A, R, M, O, D), and an approved commit shows "Approved". In
 * the row menu a write opens the review window for it ("…"), which fetches the pull request first, and shows no keys
 * but Enter and O. What Rust refuses (an own pull request, the base Patronus doesn't merge into) is disabled everywhere
 */
export function getReviewRequestActions(request: ReviewRequest, context: ReviewRequestActionContext, surface: ReviewActionSurface): ReviewRequestActions {
  const inReview = surface === "window"
  const { reviewedCommit, busy, actions } = context
  // the row knows the head GitHub shows; the review the commit the user sees
  const shownCommit = inReview ? reviewedCommit.id : request.headRefOid
  const approvedCommit = approvedCommitOf(actions)
  const approved = approvedCommit !== null && approvedCommit === shownCommit
  // the review names what the write needs; the row's item opens the review, which gets it
  const writeBlocked = !inReview ? null : reviewedCommit.id === null ? reviewedCommit.reason : busy !== undefined ? "Wait for GitHub to answer" : null
  const keys = (key: string) => (inReview ? [key] : undefined)
  const approveRefused = writeBlocked ?? refusal(actions, "approve", request)

  const approve: MenuItem = {
    id: "approve",
    label: !inReview ? "Approve…" : approved ? "Approved" : "Approve",
    icon: CheckIcon,
    kbds: keys("a"),
    disabled: approveRefused !== null || approved,
    description: approved ? `You approved ${formatShortHash(approvedCommit)}` : (approveRefused ?? undefined),
    onSelect: context.onApprove,
  }

  let safePush: MenuItem | null = null
  if (findAction(actions, "safe-push-pull-request") !== undefined) {
    const blocked = writeBlocked ?? refusal(actions, "safe-push-pull-request", request)
    safePush = {
      id: "safe-push",
      // after the approval of the commit, the Safe Push alone is left
      label: approved ? "Safe Push…" : "Approve and Safe Push…",
      icon: GitMergeIcon,
      kbds: keys("m"),
      disabled: blocked !== null,
      description: blocked ?? undefined,
      onSelect: context.onSafePush,
    }
  }

  const requestChangesRefused = writeBlocked ?? refusal(actions, "request-changes", request)
  const offered = (id: ActionId, onSelect: (() => void) | undefined): MenuItem | null =>
    onSelect === undefined ? null : presentAction(actions, id, { onSelect }, { subject: `#${request.number}` })
  return {
    review: { id: "review", label: "Review", icon: UserCheckIcon, kbds: inReview ? ["d"] : ["enter"], onSelect: context.onReview },
    approve,
    approved,
    safePush,
    requestChanges: {
      id: "request-changes",
      label: "Request Changes…",
      icon: MessageSquareDiffIcon,
      kbds: keys("r"),
      disabled: requestChangesRefused !== null,
      description: requestChangesRefused ?? undefined,
      onSelect: context.onRequestChanges,
    },
    openPullRequest: { id: "open-pull-request", label: "Open Pull Request", icon: GitPullRequestIcon, kbds: ["o"], onSelect: context.onOpenPullRequest },
    // a menu item copies without a toast
    copyLink: { id: "copy-pull-request-link", label: "Copy Pull Request Link", icon: LinkIcon, onSelect: () => void copyText(request.url) },
    copyBranchName: { id: "copy-branch-name", label: "Copy Branch Name", icon: CopyIcon, onSelect: () => void copyText(request.headRefName) },
    showBranch: offered("show-branch", context.onShowBranch),
    removeFromReviews: offered("remove-from-reviews", context.onRemoveFromReviews),
  }
}

/** The actions of a row in the groups of its menu: review it, write to it, open it, copy */
export function reviewRequestMenu(actions: ReviewRequestActions): MenuItemGroups {
  const writes = [actions.approve, ...(actions.safePush === null ? [] : [actions.safePush]), actions.requestChanges]
  const open = [actions.openPullRequest, ...(actions.showBranch === null ? [] : [actions.showBranch])]
  const groups: MenuItem[][] = [[actions.review], writes, open, [actions.copyLink, actions.copyBranchName]]
  if (actions.removeFromReviews !== null) {
    groups.push([actions.removeFromReviews])
  }
  return groups
}
