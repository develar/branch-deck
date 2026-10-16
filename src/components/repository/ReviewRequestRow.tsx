import { memo } from "react"
import type { MouseEvent } from "react"
import { ExternalLinkIcon } from "lucide-react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { ContextMenu, ContextMenuContent, ContextMenuTrigger } from "@/components/ui/context-menu"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { StatusChipDot } from "@/components/branch/StatusChip"
import { LineCounts } from "@/components/commit/CommitMetadata"
import { ContextMenuItems } from "@/components/shared/ContextMenuItems"
import { RELATIVE_TIME_TICK_MS, useNow } from "@/hooks/useNow"
import type { ReviewRequest } from "@/lib/bindings"
import type { MenuItemGroups } from "@/lib/menu"
import { isMac } from "@/lib/platform"
import { getReviewRequestActions, reviewRequestMenu } from "@/lib/review-request-actions"
import { formatRequestMeta, formatWaiting, hasWaitedWorkingDay } from "@/lib/review-requests"
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
 * The menu of a row, the right-click and the context menu key: the actions of the pull request, as the footer of its
 * review dialog has them (Review, Approve…, Request Changes…, Open Pull Request, the copy actions); a write opens the
 * dialog for it
 */
function requestMenu(request: ReviewRequest, repositoryPath: string, submitted: SubmittedReview | undefined, busy: ReviewAction | undefined): MenuItemGroups {
  const actions = getReviewRequestActions(request, {
    reviewedCommit: { id: null, reason: "" },
    submitted,
    busy,
    safePush: null,
    onReview: () => openReviewRequest(repositoryPath, request.number),
    onApprove: () => openReviewRequest(repositoryPath, request.number, "approve"),
    onSafePush: () => openReviewRequest(repositoryPath, request.number, "safePush"),
    onRequestChanges: () => openReviewRequest(repositoryPath, request.number, "requestChanges"),
    onOpenPullRequest: () => openRequestOnGitHub(request),
  }, "rowMenu")
  return reviewRequestMenu(actions, "rowMenu")
}

/**
 * A pull request that waits for the user's review, one 36 px line: the unread dot, the title, the muted meta ("#123 ·
 * alice · 8 files · +120 −30", the base when it is not the default branch), a dot for failing or running checks and how
 * long it waits. A click reviews it, ⌘-click and O open it on GitHub, as the button on hover does.
 */
export const ReviewRequestRow = memo(function ReviewRequestRow({ request, repositoryPath, unread, tabStop }: ReviewRequestRowProps) {
  const submitted = useSubmittedReview(request.number)
  const busy = useReviewAction(request.number)
  const onClick = (event: MouseEvent) => {
    if (isMac ? event.metaKey : event.ctrlKey) {
      openRequestOnGitHub(request)
    }
    else {
      openReviewRequest(repositoryPath, request.number)
    }
  }
  const files = request.changedFiles === 1 ? "1 file" : `${request.changedFiles} files`
  return (
    <ContextMenu>
      <ContextMenuTrigger
        render={(
          <li
            data-testid="review-request-row"
            data-review-number={request.number}
            data-unread={unread || undefined}
            tabIndex={tabStop ? 0 : -1}
            aria-label={`Review #${request.number} ${request.title}`}
            className="group flex h-9 animate-in cursor-pointer items-center gap-2 px-4 text-sm duration-150 outline-none fade-in-0 hover:bg-muted focus-visible:bg-muted focus-visible:ring-3 focus-visible:ring-ring/30 focus-visible:ring-inset sm:px-6"
            onClick={onClick}
          />
        )}
      >
        {/* the dot keeps its place on a read row: the titles line up */}
        <span aria-hidden data-testid={unread ? "review-request-unread" : undefined} className={cn("size-1.5 shrink-0 rounded-full", unread && "bg-primary")} />
        <span className="min-w-0 truncate font-medium" data-testid="review-request-title">{request.title}</span>
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
              render={(
                <Button
                  size="icon-xs"
                  variant="ghost"
                  aria-label={`Open pull request #${request.number}`}
                  data-testid="review-request-open"
                  className="text-muted-foreground opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
                  tabIndex={-1}
                  onClick={(event) => {
                    event.stopPropagation()
                    openRequestOnGitHub(request)
                  }}
                />
              )}
            >
              <ExternalLinkIcon />
            </TooltipTrigger>
            <TooltipContent>Open Pull Request</TooltipContent>
          </Tooltip>
          <WaitingAge waitingSince={request.waitingSince} reRequested={request.reRequested} />
        </span>
      </ContextMenuTrigger>
      <ContextMenuContent>
        <ContextMenuItems groups={requestMenu(request, repositoryPath, submitted, busy)} />
      </ContextMenuContent>
    </ContextMenu>
  )
})

/**
 * "waiting 2 days", "re-requested 3 hours ago": muted, `warning` after a working day; the leaf on the shared clock, so a
 * tick re-renders only it
 */
export function WaitingAge({ waitingSince, reRequested }: Pick<ReviewRequest, "waitingSince" | "reRequested">) {
  const now = useNow(RELATIVE_TIME_TICK_MS)
  const old = hasWaitedWorkingDay(waitingSince * 1000, now)
  return (
    <span data-testid="review-request-age" data-old={old || undefined} className={cn("text-xs whitespace-nowrap tabular-nums", old ? "text-warning" : "text-muted-foreground")}>
      {formatWaiting({ waitingSince, reRequested }, now)}
    </span>
  )
}
