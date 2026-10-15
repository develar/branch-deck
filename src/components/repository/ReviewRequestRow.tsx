import { memo } from "react"
import type { MouseEvent } from "react"
import { CopyIcon, ExternalLinkIcon, LinkIcon, UserCheckIcon } from "lucide-react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { ContextMenu, ContextMenuContent, ContextMenuTrigger } from "@/components/ui/context-menu"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { StatusChipDot } from "@/components/branch/StatusChip"
import { LineCounts } from "@/components/commit/CommitMetadata"
import { ContextMenuItems } from "@/components/shared/ContextMenuItems"
import { RELATIVE_TIME_TICK_MS, useNow } from "@/hooks/useNow"
import type { ReviewRequest } from "@/lib/bindings"
import { copyText } from "@/lib/clipboard"
import type { MenuItemGroups } from "@/lib/menu"
import { isMac } from "@/lib/platform"
import { formatRequestMeta, formatWaiting, hasWaitedWorkingDay } from "@/lib/review-requests"
import { openRequestOnGitHub, openReviewRequest } from "@/stores/review-requests"

export interface ReviewRequestRowProps {
  request: ReviewRequest
  repositoryPath: string
  unread: boolean
  /** The row the list's focus is on: the one tab stop of the list */
  tabStop: boolean
}

/** The menu of a row, the right-click and the context menu key: Review, Open on GitHub, Copy Link, Copy Branch Name */
function requestMenu(request: ReviewRequest, repositoryPath: string): MenuItemGroups {
  return [
    [
      { label: "Review", icon: UserCheckIcon, onSelect: () => openReviewRequest(repositoryPath, request.number) },
      { label: "Open on GitHub", icon: ExternalLinkIcon, onSelect: () => openRequestOnGitHub(request) },
    ],
    [
      // a menu item copies without a toast
      { label: "Copy Link", icon: LinkIcon, onSelect: () => void copyText(request.url) },
      { label: "Copy Branch Name", icon: CopyIcon, onSelect: () => void copyText(request.headRefName) },
    ],
  ]
}

/**
 * A pull request that waits for the user's review, one 36 px line: the unread dot, the title, the muted meta ("#123 ·
 * alice · 8 files · +120 −30", the base when it is not the default branch), a dot for failing or running checks and how
 * long it waits. A click reviews it, ⌘-click opens it on GitHub, as the button on hover does.
 */
export const ReviewRequestRow = memo(function ReviewRequestRow({ request, repositoryPath, unread, tabStop }: ReviewRequestRowProps) {
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
                  aria-label={`Open #${request.number} on GitHub`}
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
            <TooltipContent>{`Open #${request.number} on GitHub`}</TooltipContent>
          </Tooltip>
          <WaitingAge waitingSince={request.waitingSince} reRequested={request.reRequested} />
        </span>
      </ContextMenuTrigger>
      <ContextMenuContent>
        <ContextMenuItems groups={requestMenu(request, repositoryPath)} />
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
