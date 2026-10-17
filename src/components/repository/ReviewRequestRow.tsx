import { memo } from "react"
import type { MouseEvent } from "react"
import { cn } from "cn"
import { ExternalLinkIcon, GitPullRequestIcon, LinkIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { ContextMenu, ContextMenuContent, ContextMenuTrigger } from "@/components/ui/context-menu"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { StatusChipDot } from "@/components/branch/StatusChip"
import { LineCounts } from "@/components/commit/CommitMetadata"
import { ContextMenuItems } from "@/components/shared/ContextMenuItems"
import { RELATIVE_TIME_TICK_MS, useNow } from "@/hooks/useNow"
import type { ReviewRequest } from "@/lib/bindings"
import { copyText } from "@/lib/clipboard"
import { formatFileCount } from "@/lib/commit-files"
import type { MenuItemGroups } from "@/lib/menu"
import type { ReturnedPullRequest } from "@/lib/outgoing-attention"
import { formatReturnedMeta } from "@/lib/outgoing-attention"
import { isMac } from "@/lib/platform"
import { openPullRequest } from "@/lib/pull-request"
import { getReviewRequestActions, reviewRequestMenu } from "@/lib/review-request-actions"
import { formatRequestMeta, formatWaiting, hasWaitedWorkingDay } from "@/lib/review-requests"
import { requestReveal } from "@/stores/commit-move"
import { usePatronusAvailable } from "@/stores/patronus"
import type { ReviewAction, SubmittedReview } from "@/stores/review-requests"
import { openRequestOnGitHub, openReviewRequest, useReviewAction, useSubmittedReview } from "@/stores/review-requests"

export interface ReviewRequestRowProps {
  request: ReviewRequest
  repositoryPath: string
  unread: boolean
  /** The row the list's focus is on: the one tab stop of the list */
  tabStop: boolean
}

/**
 * The menu of a row, the right-click and the context menu key: the actions of the pull request, as the header of its
 * review has them (Review, Approve…, Approve and Safe Push… where Patronus merges, Request Changes…, Open Pull Request,
 * Open in Window, the copy actions); a write opens the review for it
 */
function requestMenu(
  request: ReviewRequest,
  repositoryPath: string,
  submitted: SubmittedReview | undefined,
  busy: ReviewAction | undefined,
  patronusAvailable: boolean,
): MenuItemGroups {
  const actions = getReviewRequestActions(
    request,
    {
      reviewedCommit: { id: null, reason: "" },
      submitted,
      busy,
      // the dialog says whether Patronus merges into its base
      safePush: patronusAvailable ? { blocked: null } : null,
      onReview: () => openReviewRequest(repositoryPath, request.number),
      onApprove: () => openReviewRequest(repositoryPath, request.number, "approve"),
      onSafePush: () => openReviewRequest(repositoryPath, request.number, "safePush"),
      onRequestChanges: () => openReviewRequest(repositoryPath, request.number, "requestChanges"),
      onOpenPullRequest: () => openRequestOnGitHub(request),
      onOpenInWindow: () => openReviewRequest(repositoryPath, request.number, null, true),
    },
    "rowMenu",
  )
  return reviewRequestMenu(actions, "rowMenu")
}

/**
 * A pull request that waits for the user's review, one 36 px line: the unread dot, the title, the muted meta ("#123 ·
 * alice · 8 files · +120 −30", the base when it is not the default branch), a dot for failing or running checks and how
 * long it waits. A click reviews it (in the review window while it is open), ⇧-click in the review window, ⌘-click and
 * O open it on GitHub, as the button on hover does.
 */
export const ReviewRequestRow = memo(function ReviewRequestRow({ request, repositoryPath, unread, tabStop }: ReviewRequestRowProps) {
  const submitted = useSubmittedReview(request.number)
  const busy = useReviewAction(request.number)
  const patronusAvailable = usePatronusAvailable()
  const onClick = (event: MouseEvent) => {
    if (isMac ? event.metaKey : event.ctrlKey) {
      openRequestOnGitHub(request)
    } else {
      // ⇧-click: a window of its own, as a new window is on macOS
      openReviewRequest(repositoryPath, request.number, null, event.shiftKey)
    }
  }
  const files = formatFileCount(request.changedFiles)
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
        {/* the dot keeps its place on a read row: the titles line up */}
        <span aria-hidden data-testid={unread ? "review-request-unread" : undefined} className={cn("size-1.5 shrink-0 rounded-full", unread && "bg-primary")} />
        <span className="min-w-0 truncate font-medium" data-testid="review-request-title">
          {request.title}
        </span>
        <span className="hidden shrink-0 items-center gap-1 text-xs text-muted-foreground sm:flex" data-testid="review-request-meta">
          {`${formatRequestMeta(request)} · ${files} · `}
          <LineCounts additions={request.additions} deletions={request.deletions} colored />
          {!request.baseIsDefault && ` · → ${request.baseRefName}`}
        </span>
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
          <WaitingAge waitingSince={request.waitingSince} reRequested={request.reRequested} />
        </span>
      </ContextMenuTrigger>
      <ContextMenuContent>
        <ContextMenuItems groups={requestMenu(request, repositoryPath, submitted, busy, patronusAvailable)} />
      </ContextMenuContent>
    </ContextMenu>
  )
})

/**
 * "waiting 2 days", "re-requested 3 hours ago": muted, `warning` after a working day; the leaf on the shared clock, so a
 * tick re-renders only it
 */
export function WaitingAge({ waitingSince, reRequested, testId = "review-request-age" }: Pick<ReviewRequest, "waitingSince" | "reRequested"> & { testId?: string }) {
  const now = useNow(RELATIVE_TIME_TICK_MS)
  const old = hasWaitedWorkingDay(waitingSince * 1000, now)
  return (
    <span data-testid={testId} data-old={old || undefined} className={cn("text-xs whitespace-nowrap tabular-nums", old ? "text-warning" : "text-muted-foreground")}>
      {formatWaiting({ waitingSince, reRequested }, now)}
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
 * requested by alice") and its branch. A click and Enter show the branch row, where the changes are made; ⌘-click and O
 * open the pull request, as the button on hover does; the menu has Open Pull Request and Copy Pull Request Link.
 */
export const ReturnedPullRequestRow = memo(function ReturnedPullRequestRow({ item, tabStop }: ReturnedPullRequestRowProps) {
  const { branchName, pullRequest } = item
  const showBranch = () => requestReveal({ branchName, commitIds: [] })
  const menu: MenuItemGroups = [
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
              // the physical key, in any keyboard layout: the key of Open Pull Request in the row's menu
              else if (event.code === "KeyO" && !event.shiftKey) {
                event.preventDefault()
                openPullRequest(pullRequest)
              }
            }}
          />
        }
      >
        {/* the place of the unread dot of a request: the titles line up */}
        <span aria-hidden className="size-1.5 shrink-0" />
        <span className="min-w-0 truncate font-medium" data-testid="returned-pull-request-title">
          {pullRequest.title}
        </span>
        <span className="hidden shrink-0 text-xs text-muted-foreground sm:flex" data-testid="returned-pull-request-meta">
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
