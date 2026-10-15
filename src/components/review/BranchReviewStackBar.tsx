import { Fragment } from "react"
import type { MouseEvent } from "react"
import { ChevronRightIcon } from "lucide-react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import type { BranchReviewSegment } from "@/lib/bindings"
import { selectSegment } from "@/lib/branch-review"
import type { SegmentRange } from "@/lib/branch-review"
import { formatCommitCount } from "@/lib/commit"

export interface BranchReviewStackBarProps {
  baselineBranch: string
  /** Root first, the reviewed branch last */
  segments: readonly BranchReviewSegment[]
  range: SegmentRange
  onRangeChange: (range: SegmentRange) => void
}

/**
 * The stack of the reviewed branch from the baseline: a segment per branch with its commits. A click shows one branch
 * on top of its parent, Shift+click extends the range to the clicked branch, "Whole Stack" shows everything from the
 * baseline (what Patronus checks).
 */
export function BranchReviewStackBar({ baselineBranch, segments, range, onRangeChange }: BranchReviewStackBarProps) {
  const isWhole = range.from === 0 && range.to === segments.length - 1
  const select = (index: number) => (event: MouseEvent) => onRangeChange(selectSegment(range, index, event.shiftKey))
  const commitCount = segments.reduce((sum, segment) => sum + segment.commits.length, 0)
  return (
    <div className="flex min-w-0 flex-wrap items-center gap-1.5" data-testid="branch-review-stack">
      <span className="text-muted-foreground">{baselineBranch}</span>
      {segments.map((segment, index) => {
        const selected = index >= range.from && index <= range.to
        return (
          <Fragment key={segment.name}>
            <ChevronRightIcon aria-hidden className="size-3.5 shrink-0 text-muted-foreground" />
            <Button
              size="xs"
              variant="outline"
              aria-pressed={selected}
              data-testid="branch-review-segment"
              data-name={segment.name}
              className={cn("max-w-64", selected && "border-primary/50 bg-primary/10 text-primary hover:bg-primary/15 hover:text-primary")}
              onClick={segments.length > 1 ? select(index) : undefined}
            >
              <span className="truncate">{segment.name}</span>
              <span className="text-muted-foreground tabular-nums">{segment.commits.length}</span>
            </Button>
          </Fragment>
        )
      })}
      {segments.length > 1 && (
        <Tooltip>
          <TooltipTrigger
            render={(
              <Button
                size="xs"
                variant="ghost"
                aria-pressed={isWhole}
                className={cn("ml-1", isWhole && "text-primary hover:text-primary")}
                onClick={() => onRangeChange({ from: 0, to: segments.length - 1 })}
              />
            )}
          >
            Whole Stack
          </TooltipTrigger>
          <TooltipContent>{`From ${baselineBranch}: ${formatCommitCount(commitCount)}, what Patronus checks · Shift+click a branch to extend`}</TooltipContent>
        </Tooltip>
      )}
    </div>
  )
}
