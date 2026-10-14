import { lazy, Suspense, useState } from "react"
import type { ReactNode } from "react"
import { ArrowUpIcon, GitPullRequestIcon } from "lucide-react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { usePushSummary } from "@/hooks/usePushSummary"
import type { PushSummary } from "@/lib/push-summary"
import { formatOutgoingTitle, getOutgoingBarSegments, outgoingBarFill } from "@/lib/push-summary"

const loadOutgoingPopover = () => import("@/components/repository/OutgoingPopover")
const OutgoingPopover = lazy(() => loadOutgoingPopover().then(it => ({ default: it.OutgoingPopover })))

export interface OutgoingIndicatorProps {
  /** The pull request chip of a branch row of the overview */
  renderPullRequest?: (branchName: string) => ReactNode
  /** Branches with an open pull request, in the table order: the overview lists those with nothing to push too */
  reviewBranches?: readonly string[]
  /** An open pull request waits for the author (changes requested, failing checks): a `destructive` dot */
  needsAttention?: boolean
  /** Below the branch rows of the overview: the update state of the baseline */
  footer?: ReactNode
}

/**
 * Header button with the commits waiting to be pushed and a bar of one segment per branch: the hue says whether its push
 * drops remote commits, the solid part is the user's commits. Hover or click opens the overview. With nothing to push it
 * shows the open pull requests instead, and it is hidden when there are none either.
 */
export function OutgoingIndicator({ renderPullRequest, reviewBranches = [], needsAttention = false, footer }: OutgoingIndicatorProps) {
  const summary = usePushSummary()
  const [open, setOpen] = useState(false)
  const hasOutgoing = summary.branches.length > 0
  const isShown = hasOutgoing || reviewBranches.length > 0
  // everything pushed: the next outgoing commit must not reopen the popover
  if (!isShown && open) {
    setOpen(false)
  }
  if (!isShown) {
    return null
  }

  const title = formatOutgoingTitle(summary, reviewBranches.length)
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger
        openOnHover
        delay={300}
        render={(
          <Button
            variant="ghost"
            size="sm"
            aria-label={title}
            data-testid="outgoing-indicator"
            // the overview is loaded while the hover delay runs
            onPointerEnter={() => void loadOutgoingPopover()}
            onFocus={() => void loadOutgoingPopover()}
          />
        )}
      >
        {hasOutgoing
          ? (
              <>
                <ArrowUpIcon data-icon="inline-start" />
                <span className="tabular-nums" data-testid="outgoing-total">{summary.total}</span>
                <OutgoingBar summary={summary} />
              </>
            )
          : (
              <>
                <GitPullRequestIcon data-icon="inline-start" />
                <span className="tabular-nums" data-testid="outgoing-review-count">{reviewBranches.length}</span>
              </>
            )}
        {needsAttention && <span aria-hidden data-testid="outgoing-attention-dot" className="ml-1 size-1.5 shrink-0 rounded-full bg-destructive" />}
      </PopoverTrigger>
      <PopoverContent align="end" className="w-auto max-w-lg gap-3 p-3" data-testid="outgoing-popover">
        <Suspense fallback={null}>
          <OutgoingPopover summary={summary} reviewBranches={reviewBranches} onClose={() => setOpen(false)} renderPullRequest={renderPullRequest} footer={footer} />
        </Suspense>
      </PopoverContent>
    </Popover>
  )
}

/**
 * A segment per branch (or per hue for many branches), its width proportional to the commits; the user's commits are
 * solid, the others' light. The segments of the branches are apart, the two parts of a segment touch.
 */
function OutgoingBar({ summary }: { summary: PushSummary }) {
  return (
    <span aria-hidden className="ml-1 flex h-1.5 w-16 gap-0.5 overflow-hidden" data-testid="outgoing-bar">
      {getOutgoingBarSegments(summary).map((segment) => {
        const fill = outgoingBarFill(segment.hue)
        return (
          <span
            key={segment.key}
            data-hue={segment.hue}
            className={cn("flex h-full min-w-0.5 basis-0 overflow-hidden rounded-sm", fill.others)}
            style={{ flexGrow: segment.weight }}
          >
            <span className={cn("h-full", fill.mine)} style={{ width: `${(segment.mine / segment.weight) * 100}%` }} />
          </span>
        )
      })}
    </span>
  )
}
