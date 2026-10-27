import { memo } from "react"
import type { MouseEvent, ReactNode } from "react"
import { cn } from "cn"
import { ExternalLinkIcon, FileDiffIcon, GitPullRequestIcon, LinkIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { ContextMenu, ContextMenuContent, ContextMenuTrigger } from "@/components/ui/context-menu"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { LandingNodeHover } from "@/components/branch/LandingNodeHover"
import { LaneNodeDot } from "@/components/branch/StackLanes"
import { StatusChipDot } from "@/components/branch/StatusChip"
import { LineCounts } from "@/components/commit/CommitMetadata"
import { ContextMenuItems } from "@/components/shared/ContextMenuItems"
import { useLandingNode } from "@/hooks/useLandingNode"
import { RELATIVE_TIME_TICK_MS, useNow } from "@/hooks/useNow"
import type { ActionState, ReviewRequest } from "@/lib/bindings"
import { reviewBlockerOf } from "@/lib/branch-review"
import { openBranchReviewWindow } from "@/lib/branch-review-window"
import { copyText } from "@/lib/clipboard"
import { findIssueKey, stripLeadingIssueKey } from "@/lib/commit"
import { formatFileCount } from "@/lib/commit-files"
import type { MenuItemGroups } from "@/lib/menu"
import type { ReturnedPullRequest } from "@/lib/outgoing-attention"
import { formatReturnedMeta } from "@/lib/outgoing-attention"
import { isMac } from "@/lib/platform"
import { openPullRequest } from "@/lib/pull-request"
import type { ReviewSource } from "@/lib/pull-request-review"
import { getReviewRequestActions, reviewRequestMenu } from "@/lib/review-request-actions"
import { formatRequestMeta, formatWaiting, hasWaitedWorkingDay } from "@/lib/review-requests"
import { useRowPart } from "@/model/store"
import { useBranchSync } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"
import type { ReviewAction } from "@/stores/review-requests"
import { openRequestOnGitHub, openReviewRequest, useReviewAction } from "@/stores/review-requests"

export interface ReviewRequestRowProps {
  request: ReviewRequest
  repositoryPath: string
  unread: boolean
  /** The row the list's focus is on: the one tab stop of the list */
  tabStop: boolean
  /** Where the row is: the review window walks the rows of the same list */
  source?: ReviewSource
  /** The meta after the title, by how the user takes part (Reviews); the request's ("#101 · carol · requested by dave") otherwise */
  meta?: string
  /** What the meta leaves out, in its tooltip (Reviews: "you approved 6330a0a2", the reviewers by name) */
  metaHint?: string
  /**
   * The mark at the start (Reviews: the node of an own branch, the user's review, the dot of the user's move); the
   * unread dot otherwise
   */
  lead?: ReactNode
  /** Nothing of it is the user's move (Reviews): its size goes into the tooltip of the meta */
  quiet?: boolean
  /** What the age at the end says (Reviews); how long the request waits otherwise */
  age?: ReactNode
  /** An own pull request of the table: its branch row shows */
  onShowBranch?: () => void
  /** A pull request the user added by its link leaves Reviews */
  onRemoveFromReviews?: () => void
}

/** The actions of the pull request `number` as Rust decides them */
function usePullRequestActions(number: number): readonly ActionState[] | undefined {
  return useRowPart(`pull:${number}`, "actions")
}

/**
 * The menu of a row, the right-click and the context menu key: the actions of the pull request, as the header of its
 * review has them (Review, Approve…, Approve and Safe Push… where Patronus merges, Request Changes…, Open Pull Request,
 * the copy actions; Show Branch of an own one, Remove from Reviews of a link); a write opens the review window for it
 */
function requestMenu(
  request: ReviewRequest,
  repositoryPath: string,
  source: ReviewSource,
  actions: readonly ActionState[] | undefined,
  busy: ReviewAction | undefined,
  extra: Pick<ReviewRequestRowProps, "onShowBranch" | "onRemoveFromReviews">,
): MenuItemGroups {
  const items = getReviewRequestActions(
    request,
    {
      reviewedCommit: { id: null, reason: "" },
      actions,
      busy,
      onReview: () => openReviewRequest(repositoryPath, request.number, undefined, source),
      onApprove: () => openReviewRequest(repositoryPath, request.number, "approve", source),
      onSafePush: () => openReviewRequest(repositoryPath, request.number, "safePush", source),
      onRequestChanges: () => openReviewRequest(repositoryPath, request.number, "requestChanges", source),
      onOpenPullRequest: () => openRequestOnGitHub(request),
      ...extra,
    },
    "rowMenu",
  )
  return reviewRequestMenu(items)
}

/**
 * A pull request that waits for the user's review, one 36 px line: the unread dot, the title (its issue key muted), the
 * muted meta ("#123 · alice · 8 files · +120 −30", the base when it is not the default branch), which gives way before
 * the title does, a dot for failing or running checks and how long it waits. A click reviews it in the review window;
 * ⌘-click and O open it on GitHub, as the button on hover does. Reviews shows its pull requests in the same row, with
 * the mark, the meta and the age of how the user takes part, and the size of a row that is not the user's move in the
 * tooltip of its meta.
 */
export const ReviewRequestRow = memo(function ReviewRequestRow({
  request,
  repositoryPath,
  unread,
  tabStop,
  source = "yourTurn",
  meta,
  metaHint,
  lead,
  quiet = false,
  age,
  onShowBranch,
  onRemoveFromReviews,
}: ReviewRequestRowProps) {
  const actions = usePullRequestActions(request.number)
  const busy = useReviewAction(request.number)
  const onClick = (event: MouseEvent) => {
    if (isMac ? event.metaKey : event.ctrlKey) {
      openRequestOnGitHub(request)
    } else if (onShowBranch === undefined) {
      openReviewRequest(repositoryPath, request.number, undefined, source)
    } else {
      onShowBranch()
    }
  }
  const files = formatFileCount(request.changedFiles)
  const size = `${files} · +${request.additions} −${request.deletions}`
  const hint = [metaHint, quiet ? size : undefined].filter(it => it !== undefined && it !== "").join(" · ")
  const metaLine = (
    // the title is what the row is: the meta gives way first, the title only once the meta is gone
    <span className="hidden min-w-0 shrink-[1000] truncate text-xs text-muted-foreground sm:inline" data-testid="review-request-meta">
      {meta ?? formatRequestMeta(request)}
      {!quiet && (
        <>
          {` · ${files} · `}
          <LineCounts additions={request.additions} deletions={request.deletions} colored={false} />
        </>
      )}
      {!request.baseIsDefault && ` · → ${request.baseRefName}`}
    </span>
  )
  return (
    <ContextMenu>
      <ContextMenuTrigger
        render={
          <li
            data-testid="review-request-row"
            data-review-number={request.number}
            data-unread={unread || undefined}
            tabIndex={tabStop ? 0 : -1}
            aria-label={`Review #${request.number} ${request.title}`}
            className="group flex h-9 animate-in cursor-pointer items-center gap-2 px-4 text-sm duration-150 fade-in-0 outline-none hover:bg-muted focus-visible:bg-muted focus-visible:ring-3 focus-visible:ring-ring/30 focus-visible:ring-inset sm:px-6"
            onClick={onClick}
          />
        }
      >
        {/* the mark keeps its place on a row without one: the titles line up */}
        <span className="flex size-4 shrink-0 items-center justify-center">{lead ?? <UnreadDot unread={unread} />}</span>
        <span className="min-w-0 truncate font-medium" data-testid="review-request-title">
          <PullRequestTitle title={request.title} />
        </span>
        {hint === "" ? (
          metaLine
        ) : (
          <Tooltip>
            <TooltipTrigger render={metaLine} />
            <TooltipContent>{hint}</TooltipContent>
          </Tooltip>
        )}
        {request.checksState === "failing" && <StatusChipDot color="error" testId="review-request-checks-failing" />}
        {request.checksState === "pending" && <StatusChipDot color="warning" testId="review-request-checks-pending" />}
        <span className="ml-auto flex shrink-0 items-center gap-2">
          <Tooltip>
            <TooltipTrigger
              render={
                <Button
                  size="icon-xs"
                  variant="ghost"
                  aria-label={`Open pull request #${request.number}`}
                  data-testid="review-request-open"
                  className="text-muted-foreground opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
                  tabIndex={-1}
                  onClick={event => {
                    event.stopPropagation()
                    openRequestOnGitHub(request)
                  }}
                />
              }
            >
              <ExternalLinkIcon />
            </TooltipTrigger>
            <TooltipContent>Open Pull Request</TooltipContent>
          </Tooltip>
          {age ?? <WaitingAge waitingSince={request.waitingSince} reRequested={request.reRequested} backToYou={request.backToYou} />}
        </span>
      </ContextMenuTrigger>
      <ContextMenuContent>
        <ContextMenuItems groups={requestMenu(request, repositoryPath, source, actions, busy, { onShowBranch, onRemoveFromReviews })} />
      </ContextMenuContent>
    </ContextMenu>
  )
})

/**
 * The node of a branch of the table, as its row has it, its hover the row's: where the branch is on its way into
 * master; nothing while the view has no node of it
 */
export function BranchNodeMark({ branch }: { branch: string }) {
  const node = useLandingNode(branch)
  if (node === null) {
    return null
  }
  return (
    <LandingNodeHover node={node} subject={{ kind: "branch", name: branch }} trigger={<span className="flex" data-testid="review-entry-node" data-kind={node.kind} />}>
      <LaneNodeDot kind={node.kind} />
    </LandingNodeHover>
  )
}

/** The dot of an unread request; its place kept on a read one */
function UnreadDot({ unread }: { unread: boolean }) {
  return <span aria-hidden data-testid={unread ? "review-request-unread" : undefined} className={cn("size-1.5 rounded-full", unread && "bg-primary")} />
}

/** The title with the issue key it starts with muted, as a branch row has its key: the words are what the eye reads */
function PullRequestTitle({ title }: { title: string }) {
  const key = findIssueKey(title)
  const rest = key === null ? title : stripLeadingIssueKey(title, key)
  if (key === null || rest === title) {
    return title
  }
  return (
    <>
      <span className="font-normal text-muted-foreground">{key}</span> {rest}
    </>
  )
}

/**
 * "waiting 2 days", "alice replied 10 minutes ago", "re-requested 3 hours ago": muted, `warning` after a working day;
 * the leaf on the shared clock, so a tick re-renders only it
 */
export function WaitingAge({
  waitingSince,
  reRequested,
  backToYou,
  testId = "review-request-age",
}: Pick<ReviewRequest, "waitingSince" | "reRequested" | "backToYou"> & { testId?: string }) {
  const now = useNow(RELATIVE_TIME_TICK_MS)
  const old = hasWaitedWorkingDay(waitingSince * 1000, now)
  return (
    <span data-testid={testId} data-old={old || undefined} className={cn("text-xs whitespace-nowrap tabular-nums", old ? "text-warning" : "text-muted-foreground")}>
      {formatWaiting({ waitingSince, reRequested, backToYou }, now)}
    </span>
  )
}

export interface ReturnedPullRequestRowProps {
  item: ReturnedPullRequest
  /** The row the list's focus is on: the one tab stop of the list */
  tabStop: boolean
}

/**
 * An own pull request a reviewer handed back, one 36 px line as a request: the title, the muted meta ("#812 · changes
 * requested by alice") and its branch. A click and Enter show the branch row, where the changes are made; D reviews the
 * branch in its window; ⌘-click and O open the pull request, as the button on hover does; the menu has Review Changes,
 * Open Pull Request and Copy Pull Request Link.
 */
export const ReturnedPullRequestRow = memo(function ReturnedPullRequestRow({ item, tabStop }: ReturnedPullRequestRowProps) {
  const { branchName, pullRequest } = item
  const showBranch = () => requestReveal({ branchName, commitIds: [] })
  // the review of its branch, as D on the branch row opens it
  const branch = useBranchSync(state => state.branches[branchName])
  const actions = useRowPart(branchName, "actions")
  const reviewBlocker = branch === undefined ? `${branchName} is not in the table` : reviewBlockerOf(branch, actions)
  const reviewBranch = () => {
    if (reviewBlocker === null) {
      void openBranchReviewWindow(branchName)
    }
  }
  const menu: MenuItemGroups = [
    [
      {
        id: "review-changes",
        label: "Review Changes",
        icon: FileDiffIcon,
        kbds: ["d"],
        disabled: reviewBlocker !== null,
        ...(reviewBlocker !== null && { description: reviewBlocker }),
        onSelect: reviewBranch,
      },
    ],
    [
      { id: "open-pull-request", label: "Open Pull Request", icon: GitPullRequestIcon, kbds: ["o"], onSelect: () => openPullRequest(pullRequest) },
      { id: "copy-pull-request-link", label: "Copy Pull Request Link", icon: LinkIcon, onSelect: () => void copyText(pullRequest.url) },
    ],
  ]
  return (
    <ContextMenu>
      <ContextMenuTrigger
        render={
          <li
            data-testid="returned-pull-request-row"
            data-returned-branch={branchName}
            tabIndex={tabStop ? 0 : -1}
            aria-label={`Show ${branchName}: #${pullRequest.number} ${pullRequest.title}, changes requested`}
            className="group flex h-9 cursor-pointer items-center gap-2 px-4 text-sm outline-none hover:bg-muted focus-visible:bg-muted focus-visible:ring-3 focus-visible:ring-ring/30 focus-visible:ring-inset sm:px-6"
            onClick={event => {
              if (isMac ? event.metaKey : event.ctrlKey) {
                openPullRequest(pullRequest)
              } else {
                showBranch()
              }
            }}
            onKeyDown={event => {
              if (event.target !== event.currentTarget || event.altKey || event.metaKey || event.ctrlKey) {
                return
              }
              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault()
                showBranch()
              }
              // the physical key, in any keyboard layout: the keys of Open Pull Request and Review Changes in the row's menu
              else if (event.code === "KeyO" && !event.shiftKey) {
                event.preventDefault()
                openPullRequest(pullRequest)
              } else if (event.code === "KeyD" && !event.shiftKey) {
                event.preventDefault()
                reviewBranch()
              }
            }}
          />
        }
      >
        {/* in the place of the unread dot of a request: the titles line up */}
        <span className="flex size-4 shrink-0 items-center justify-center">
          <BranchNodeMark branch={branchName} />
        </span>
        <span className="min-w-0 truncate font-medium" data-testid="returned-pull-request-title">
          <PullRequestTitle title={pullRequest.title} />
        </span>
        <span className="hidden min-w-0 shrink-[1000] truncate text-xs text-muted-foreground sm:inline" data-testid="returned-pull-request-meta">
          {formatReturnedMeta(pullRequest)}
        </span>
        <span className="ml-auto flex shrink-0 items-center gap-2">
          <Tooltip>
            <TooltipTrigger
              render={
                <Button
                  size="icon-xs"
                  variant="ghost"
                  aria-label={`Open pull request #${pullRequest.number}`}
                  className="text-muted-foreground opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
                  tabIndex={-1}
                  onClick={event => {
                    event.stopPropagation()
                    openPullRequest(pullRequest)
                  }}
                />
              }
            >
              <ExternalLinkIcon />
            </TooltipTrigger>
            <TooltipContent>Open Pull Request</TooltipContent>
          </Tooltip>
          <span className="text-xs whitespace-nowrap text-muted-foreground" data-testid="returned-pull-request-branch">
            {branchName}
          </span>
        </span>
      </ContextMenuTrigger>
      <ContextMenuContent>
        <ContextMenuItems groups={menu} />
      </ContextMenuContent>
    </ContextMenu>
  )
})
