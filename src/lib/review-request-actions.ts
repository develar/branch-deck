import { CheckCheckIcon, CheckIcon, CopyIcon, GitMergeIcon, GitPullRequestIcon, LinkIcon, MessageSquareDiffIcon, UserCheckIcon } from "lucide-react"
import type { ActionId, ActionState, ReviewRequest, ReviewStackApproval } from "@/lib/bindings"
import { copyText } from "@/lib/clipboard"
import { formatShortHash } from "@/lib/hash"
import type { MenuItem, MenuItemGroups } from "@/lib/menu"
import { formatApproveStack } from "@/lib/review-requests"
import { joinNames } from "@/lib/text"
import { findAction, formatActionBlocker, presentAction } from "@/presenters/actions"
import type { OperationWords } from "@/presenters/operations"
import type { ReviewAction } from "@/stores/review-requests"

/*
 * The actions of a pull request that waits for the user's review (docs/contributing/ux-writing.md#actions): one list for
 * the menu of its row in Your Turn and the header of its review window, in the overview and in the review of its changes
 * alike.
 */

/**
 * Where the actions show: the row menu opens the review window for a write (approving needs the commit the user saw);
 * the window is the review
 */
export type ReviewActionSurface = "rowMenu" | "window"

/**
 * What an open of the window asks for: a write offered at once, or the review of the changes (D on the row); of a stack
 * Approve N Pull Requests, its button focused once the stack is fetched
 */
export type ReviewIntent = "approve" | "safePush" | "requestChanges" | "review" | "approveStack"

/** What an open of Review Stack asks for: the changes at once (D on the header of the stack), Approve N Pull Requests */
export type StackReviewIntent = Extract<ReviewIntent, "review" | "approveStack">

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
  /**
   * The operations of the session as the window's view has them and the busy states this window gave its own: the
   * words of an action of it that runs or waits (`useOperationWords` of `actions`)
   */
  readonly words: OperationWords
  /** What runs for it now */
  readonly busy: ReviewAction | undefined
  /**
   * What an approval counts for in the review window (`presentApprovalScope`): "Counts for Design · You opened 3 of its 5
   * files"; none without groups, and in the row menu
   */
  readonly approvalScope?: string | null
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
  /**
   * Absent where Rust offers no approval of one pull request: All Changes of a stack, whose actions are those of the stack
   * (`review-stack:<top>`, Approve N Pull Requests alone)
   */
  readonly approve: MenuItem | null
  /** The user approved the commit the review shows: Approve is done, Safe Push… is the next step */
  readonly approved: boolean
  /** What the approval counts for, in the description of Approve and the confirmation of Approve and Safe Push; never a blocker */
  readonly approvalScope: string | null
  /** Absent where Patronus doesn't merge */
  readonly safePush: MenuItem | null
  /** Absent as Approve is */
  readonly requestChanges: MenuItem | null
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
function refusal(actions: readonly ActionState[] | undefined, id: ActionId, request: ReviewRequest, words: OperationWords): string | null {
  const action = findAction(actions, id)
  if (action?.availability.kind !== "disabled" || action.availability.reason.code === "approved") {
    return null
  }
  return formatActionBlocker(id, action.availability.reason, { ...words, subject: `#${request.number}` })
}

/**
 * The actions of `request`. In the review window a write runs on the commit the user reviewed and is disabled with the
 * reason until there is one; the keys are the review's (A, R, M, O, D), and an approved commit shows "Approved". In
 * the row menu a write opens the review window for it ("…"), which fetches the pull request first, and shows no keys
 * but Enter and O. What Rust refuses (an own pull request, the base Patronus doesn't merge into) is disabled everywhere
 */
export function getReviewRequestActions(request: ReviewRequest, context: ReviewRequestActionContext, surface: ReviewActionSurface): ReviewRequestActions {
  const inReview = surface === "window"
  const { reviewedCommit, busy, actions, words } = context
  // the row knows the head GitHub shows; the review the commit the user sees
  const shownCommit = inReview ? reviewedCommit.id : request.headRefOid
  const approvedCommit = approvedCommitOf(actions)
  const approved = approvedCommit !== null && approvedCommit === shownCommit
  // the review names what the write needs; the row's item opens the review, which gets it
  const writeBlocked = !inReview ? null : reviewedCommit.id === null ? reviewedCommit.reason : busy !== undefined ? "Wait for GitHub to answer" : null
  const keys = (key: string) => (inReview ? [key] : undefined)
  const approveRefused = writeBlocked ?? refusal(actions, "approve", request, words)
  const approvalScope = inReview && !approved ? (context.approvalScope ?? null) : null
  // the reason it waits first, then what it counts for: information only
  const approveDescription = [approveRefused, approvalScope].filter(it => it !== null).join(". ")

  // the decisions of one pull request are offered where Rust offers them: not on All Changes of a stack; while the
  // session has no actions of the pull request (another repository) they are
  const offers = (id: ActionId) => actions === undefined || findAction(actions, id) !== undefined
  const approve: MenuItem | null = !offers("approve")
    ? null
    : {
        id: "approve",
        label: !inReview ? "Approve…" : approved ? "Approved" : "Approve",
        icon: CheckIcon,
        kbds: keys("a"),
        disabled: approveRefused !== null || approved,
        description: approved ? `You approved ${formatShortHash(approvedCommit)}` : approveDescription === "" ? undefined : approveDescription,
        onSelect: context.onApprove,
      }

  let safePush: MenuItem | null = null
  if (findAction(actions, "safe-push-pull-request") !== undefined) {
    const blocked = writeBlocked ?? refusal(actions, "safe-push-pull-request", request, words)
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

  const requestChangesRefused = writeBlocked ?? refusal(actions, "request-changes", request, words)
  const offered = (id: ActionId, onSelect: (() => void) | undefined): MenuItem | null =>
    onSelect === undefined ? null : presentAction(actions, id, { onSelect }, { ...words, subject: `#${request.number}` })
  return {
    review: { id: "review", label: "Review", icon: UserCheckIcon, kbds: inReview ? ["d"] : ["enter"], onSelect: context.onReview },
    approve,
    approved,
    approvalScope,
    safePush,
    requestChanges: !offers("request-changes")
      ? null
      : {
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
  const writes = [actions.approve, actions.safePush, actions.requestChanges].filter(it => it !== null)
  const open = [actions.openPullRequest, ...(actions.showBranch === null ? [] : [actions.showBranch])]
  const groups: MenuItem[][] = [[actions.review], writes, open, [actions.copyLink, actions.copyBranchName]]
  if (actions.removeFromReviews !== null) {
    groups.push([actions.removeFromReviews])
  }
  return groups
}

/** What Approve N Pull Requests of a stack in its review window reads (Review Stack) */
export interface StackApprovalContext {
  /**
   * As Rust decides it over the heads the fetch of the window brought (`RepositoryView.reviewStacks`,
   * `review-stack:<top>`, `domain_policy::stack_approval`); undefined until the session has the fetch of the stack
   */
  readonly approval: ReviewStackApproval | undefined
  /** Whether it can run, the action of the stack itself as Rust decides it (`review-stack:<top>` of `RepositoryView.actions`) */
  readonly action: ActionState | undefined
  /** The layers the window shows: the count before Rust says which it approves */
  readonly layerCount: number
  /** Why the window has no heads to approve on yet (the stack is fetched, its fetch failed, a head could not be fetched); null once it has */
  readonly waiting: string | null
  /** Approve N Pull Requests of the stack, or a review of one of its layers, runs */
  readonly busy: boolean
  /** The operations of the session as the window has them (`useOperationWords`) */
  readonly words: OperationWords
  /** What the approvals count for where Patronus merges (`presentStackApprovalLanding`); null without Patronus */
  readonly landing: string | null
  readonly onApprove: () => void
}

/**
 * Approve N Pull Requests of the stack (the button beside Approve, its item in the window's menu, ⇧A): the layers Rust
 * approves on the heads the window shows, disabled with the reason while the window has none, while a write runs, or as
 * Rust refuses it (every layer approved on its head); the layers approved already are named, they are skipped
 */
export function getStackApprovalItem(context: StackApprovalContext): MenuItem {
  const { approval } = context
  const approves = approval?.approves.length ?? 0
  const reason = context.action?.availability.kind === "disabled" ? context.action.availability.reason : null
  const refused =
    context.waiting ??
    (context.busy
      ? "Wait for GitHub to answer"
      : approval === undefined || context.action === undefined
        ? "Wait until the stack is fetched: an approval is on the commits you saw"
        : reason === null
          ? null
          : formatActionBlocker("approve-stack", reason, { ...context.words, subject: "the stack" }))
  const skipped = approval?.approved.map(number => `#${number}`) ?? []
  const skips = skipped.length === 0 ? null : `Skips ${joinNames(skipped)}: you already approved ${skipped.length === 1 ? "its commit" : "their commits"}`
  // the reason it waits, else what it skips and what Patronus counts the approvals for: information only
  const said = [skips, context.landing].filter(it => it !== null).join(". ")
  return {
    id: "approve-stack",
    label: formatApproveStack(approves === 0 ? context.layerCount : approves),
    icon: CheckCheckIcon,
    kbds: ["shift", "a"],
    disabled: refused !== null,
    description: refused ?? (said === "" ? undefined : said),
    onSelect: context.onApprove,
  }
}
