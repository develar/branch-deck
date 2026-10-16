import { CheckIcon, CopyIcon, GitMergeIcon, GitPullRequestIcon, LinkIcon, MessageSquareDiffIcon, UserCheckIcon } from "lucide-react"
import type { ReviewRequest } from "@/lib/bindings"
import { copyText } from "@/lib/clipboard"
import { formatShortHash } from "@/lib/hash"
import type { MenuItem, MenuItemGroups } from "@/lib/menu"
import type { ReviewAction, SubmittedReview } from "@/stores/review-requests"

/*
 * The actions of a pull request that waits for the user's review (docs/guidelines/ux-writing.md#actions): one list for
 * the menu of its row in Your Turn and the footer of its review dialog, in the preview and in the review of its
 * changes alike.
 */

/** Where the actions show: the row menu opens the dialog for a write (approving needs the commit the user saw) */
export type ReviewActionSurface = "rowMenu" | "dialog"

/** What the dialog opened from a row's menu offers at once */
export type ReviewIntent = "approve" | "safePush" | "requestChanges"

/** The commit the user reviewed, or why there is none yet */
export type ReviewedCommit = { readonly id: string } | { readonly id: null, readonly reason: string }

export interface ReviewRequestActionContext {
  readonly reviewedCommit: ReviewedCommit
  /** The review the user submitted on it from the app in this session */
  readonly submitted: SubmittedReview | undefined
  /** What runs for it now */
  readonly busy: ReviewAction | undefined
  /** null: Patronus doesn't merge in the repository, no item; else why it can't start now (null = it can) */
  readonly safePush: { readonly blocked: string | null } | null
  readonly onReview: () => void
  readonly onApprove: () => void
  readonly onSafePush: () => void
  readonly onRequestChanges: () => void
  readonly onOpenPullRequest: () => void
}

/** The actions by name: the footer of the dialog places them, a menu lists them in groups (`reviewRequestMenu`) */
export interface ReviewRequestActions {
  readonly review: MenuItem
  readonly approve: MenuItem
  /** Absent where Patronus doesn't merge */
  readonly safePush: MenuItem | null
  readonly requestChanges: MenuItem
  readonly openPullRequest: MenuItem
  readonly copyLink: MenuItem
  readonly copyBranchName: MenuItem
}

/** The approval of the user is on this very commit: approving it again says nothing new */
export function isApprovedCommit(submitted: SubmittedReview | undefined, commitId: string | null): boolean {
  return submitted?.event === "approve" && commitId !== null && submitted.commitId === commitId
}

/**
 * The actions of `request`. In the dialog a write runs on the commit the user reviewed and is disabled with the reason
 * until there is one; the keys are the dialog's (A, R, M, O). In the row menu a write opens the dialog for it ("…"),
 * which fetches the pull request first, and shows no keys but Enter.
 */
export function getReviewRequestActions(request: ReviewRequest, context: ReviewRequestActionContext, surface: ReviewActionSurface): ReviewRequestActions {
  const inDialog = surface === "dialog"
  const { reviewedCommit, submitted, busy } = context
  // the row knows the head GitHub shows; the dialog the commit the user sees
  const approvedCommit = inDialog ? reviewedCommit.id : request.headRefOid
  const approved = isApprovedCommit(submitted, approvedCommit)
  // the dialog names what the write needs; the row's item opens the dialog, which gets it
  const writeBlocked = !inDialog ? null : reviewedCommit.id === null ? reviewedCommit.reason : busy !== undefined ? "Wait for GitHub to answer" : null
  const keys = (key: string) => inDialog ? [key] : undefined

  const approve: MenuItem = {
    label: inDialog ? "Approve" : "Approve…",
    icon: CheckIcon,
    kbds: keys("a"),
    disabled: writeBlocked !== null || approved,
    description: approved ? `You approved ${formatShortHash(approvedCommit!)}` : writeBlocked ?? undefined,
    onSelect: context.onApprove,
  }

  let safePush: MenuItem | null = null
  if (context.safePush !== null) {
    const blocked = writeBlocked ?? context.safePush.blocked
    safePush = {
      // after the approval of the commit, the Safe Push alone is left
      label: approved ? "Safe Push…" : "Approve and Safe Push…",
      icon: GitMergeIcon,
      kbds: keys("m"),
      disabled: blocked !== null,
      description: blocked ?? undefined,
      onSelect: context.onSafePush,
    }
  }

  return {
    review: { label: "Review", icon: UserCheckIcon, kbds: inDialog ? ["d"] : ["enter"], onSelect: context.onReview },
    approve,
    safePush,
    requestChanges: {
      label: "Request Changes…",
      icon: MessageSquareDiffIcon,
      kbds: keys("r"),
      disabled: writeBlocked !== null,
      description: writeBlocked ?? undefined,
      onSelect: context.onRequestChanges,
    },
    openPullRequest: { label: "Open Pull Request", icon: GitPullRequestIcon, kbds: ["o"], onSelect: context.onOpenPullRequest },
    // a menu item copies without a toast
    copyLink: { label: "Copy Pull Request Link", icon: LinkIcon, onSelect: () => void copyText(request.url) },
    copyBranchName: { label: "Copy Branch Name", icon: CopyIcon, onSelect: () => void copyText(request.headRefName) },
  }
}

/** The actions in the groups of a menu: review it, write to it, open it, copy */
export function reviewRequestMenu(actions: ReviewRequestActions, surface: ReviewActionSurface): MenuItemGroups {
  const writes = [actions.approve, ...(actions.safePush === null ? [] : [actions.safePush]), actions.requestChanges]
  return [
    // the dialog's footer has Review as a button of its own
    surface === "rowMenu" ? [actions.review] : [],
    writes,
    [actions.openPullRequest],
    [actions.copyLink, actions.copyBranchName],
  ]
}
