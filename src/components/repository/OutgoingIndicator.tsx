import { lazy, Suspense, useState } from "react"
import type { ReactNode } from "react"
import { ArrowUpIcon } from "lucide-react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { usePushSummary } from "@/hooks/usePushSummary"
import type { PushSummary } from "@/lib/push-summary"
import { formatPushSummaryTitle, getOutgoingBarSegments, outgoingStateBarClass } from "@/lib/push-summary"

const loadOutgoingPopover = () => import("@/components/repository/OutgoingPopover")
const OutgoingPopover = lazy(() => loadOutgoingPopover().then(it => ({ default: it.OutgoingPopover })))

export interface OutgoingIndicatorProps {
  /** The pull request chip of a branch row of the overview */
  renderPullRequest?: (branchName: string) => ReactNode
  /** Below the branch rows of the overview: the update state of the baseline */
  footer?: ReactNode
}

/**
 * Header button with the commits waiting to be pushed and a bar of one segment per branch, coloured by what its push
 * does. Hover or click opens the overview. Hidden when nothing is outgoing.
 */
export function OutgoingIndicator({ renderPullRequest, footer }: OutgoingIndicatorProps) {
  const summary = usePushSummary()
  const [open, setOpen] = useState(false)
  const hasOutgoing = summary.branches.length > 0
  // everything pushed: the next outgoing commit must not reopen the popover
  if (!hasOutgoing && open) {
    setOpen(false)
  }
  if (!hasOutgoing) {
    return null
  }

  const title = formatPushSummaryTitle(summary)
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
        <ArrowUpIcon data-icon="inline-start" />
        <span className="tabular-nums" data-testid="outgoing-total">{summary.total}</span>
        <OutgoingBar summary={summary} />
      </PopoverTrigger>
      <PopoverContent align="end" className="w-auto max-w-lg gap-3 p-3" data-testid="outgoing-popover">
        <Suspense fallback={null}>
          <OutgoingPopover summary={summary} onClose={() => setOpen(false)} renderPullRequest={renderPullRequest} footer={footer} />
        </Suspense>
      </PopoverContent>
    </Popover>
  )
}

/** A segment per branch (or per state for many branches), its width proportional to the commits. */
function OutgoingBar({ summary }: { summary: PushSummary }) {
  return (
    <span aria-hidden className="ml-1 flex h-1.5 w-16 gap-0.5 overflow-hidden" data-testid="outgoing-bar">
      {getOutgoingBarSegments(summary).map(segment => (
        <span
          key={segment.key}
          data-state={segment.state}
          className={cn("h-full min-w-0.5 basis-0 rounded-sm", outgoingStateBarClass[segment.state])}
          style={{ flexGrow: segment.weight }}
        />
      ))}
    </span>
  )
}
