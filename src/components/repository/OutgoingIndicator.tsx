import { lazy, Suspense, useState } from "react"
import type { ReactNode } from "react"
import { cn } from "cn"
import { ArrowUpIcon, GitPullRequestIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { usePushSummary } from "@/hooks/usePushSummary"
import type { OutgoingAttention } from "@/lib/outgoing-attention"
import { formatIndicatorLabel } from "@/lib/outgoing-attention"
import type { PushSummary } from "@/lib/push-summary"
import { formatOutgoingTitle, getOutgoingBarSegments, isPlainOutgoingBar, outgoingBarFill } from "@/lib/push-summary"

const loadOutgoingPopover = () => import("@/components/repository/OutgoingPopover")
const OutgoingPopover = lazy(() => loadOutgoingPopover().then(it => ({ default: it.OutgoingPopover })))

export interface OutgoingIndicatorProps {
  /** The pull request chip of a branch row of the overview */
  renderPullRequest?: (branchName: string) => ReactNode
  /** Branches with an open pull request, in the table order: the overview lists those with nothing to push too */
  reviewBranches?: readonly string[]
  /**
   * The branches whose pull request waits for their author (changes requested, failing checks): a `destructive` dot on
   * the icon, named in the accessible name and explained by a line per branch in the overview. A failed robot is the
   * robots indicator's.
   */
  attention?: readonly OutgoingAttention[]
  /** Below the branch rows of the overview: the update state of the baseline */
  footer?: ReactNode
}

/**
 * Header button with the commits waiting to be pushed and a bar of one segment per branch: the hue says whether its push
 * drops remote commits, the solid part is the user's commits. Hover or click opens the overview. With nothing to push it
 * shows the open pull requests instead, and it is hidden when there are none either.
 */
export function OutgoingIndicator({ renderPullRequest, reviewBranches = [], attention = [], footer }: OutgoingIndicatorProps) {
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
  const Icon = hasOutgoing ? ArrowUpIcon : GitPullRequestIcon
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger
        openOnHover
        delay={300}
        render={
          <Button
            variant="ghost"
            size="sm"
            aria-label={formatIndicatorLabel(title, attention)}
            data-testid="outgoing-indicator"
            // the overview is loaded while the hover delay runs
            onPointerEnter={() => void loadOutgoingPopover()}
            onFocus={() => void loadOutgoingPopover()}
          />
        }
      >
        {/* the attention dot sits on the corner of the icon, so the bar keeps its place */}
        <span className="relative flex">
          <Icon data-icon="inline-start" />
          {attention.length > 0 && <span aria-hidden data-testid="outgoing-attention-dot" className="absolute -top-0.5 -right-0.5 size-1.5 rounded-full bg-destructive" />}
        </span>
        {hasOutgoing ? (
          <span className="flex items-center gap-2">
            <span className="tabular-nums" data-testid="outgoing-total">
              {summary.total}
            </span>
            <OutgoingBar summary={summary} />
          </span>
        ) : (
          <span className="tabular-nums" data-testid="outgoing-review-count">
            {reviewBranches.length}
          </span>
        )}
      </PopoverTrigger>
      <PopoverContent align="end" className="w-auto max-w-2xl gap-3 p-3" data-testid="outgoing-popover">
        <Suspense fallback={null}>
          <OutgoingPopover
            summary={summary}
            reviewBranches={reviewBranches}
            attention={attention}
            onClose={() => setOpen(false)}
            renderPullRequest={renderPullRequest}
            footer={footer}
          />
        </Suspense>
      </PopoverContent>
    </Popover>
  )
}

/**
 * A segment per branch (or per hue for many branches), its width proportional to the commits; the user's commits are
 * solid, the others' light. The segments of the branches are apart, the two parts of a segment touch. None where it
 * would be one solid strip (`isPlainOutgoingBar`): the count says it all.
 */
function OutgoingBar({ summary }: { summary: PushSummary }) {
  const segments = getOutgoingBarSegments(summary)
  if (isPlainOutgoingBar(segments)) {
    return null
  }
  return (
    <span aria-hidden className="flex h-1.5 w-16 gap-0.5 overflow-hidden" data-testid="outgoing-bar">
      {segments.map(segment => {
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
